use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use serde::Deserialize;
use sqlx::{postgres::PgListener, PgPool};
use thiserror::Error;
use uuid::Uuid;

use crate::search_sync::{
    index_records, SearchSyncConfig, SearchSyncError, SearchSyncRecordKey, SearchSyncReport,
};
use opentk_search::SearchIndexClient;

pub const SEARCH_CDC_CHANNEL: &str = "sync_entity_change";
pub const DEFAULT_SEARCH_CDC_FLUSH_WINDOW: Duration = Duration::from_secs(1);
pub const DEFAULT_SEARCH_CDC_MAX_UNIQUE_RECORDS: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchCdcNotification {
    source_category: String,
    source_id: Uuid,
    latest_skiptoken: i64,
    deleted: bool,
}

impl SearchCdcNotification {
    #[must_use]
    pub fn new(
        source_category: String,
        source_id: Uuid,
        latest_skiptoken: i64,
        deleted: bool,
    ) -> Self {
        Self {
            source_category,
            source_id,
            latest_skiptoken,
            deleted,
        }
    }

    /// Parse one `PostgreSQL` NOTIFY payload into a typed search CDC change.
    ///
    /// # Errors
    ///
    /// Returns [`SearchCdcError`] when the payload is not the JSON object emitted
    /// by the `sync_entity` trigger.
    pub fn from_payload(payload: &str) -> Result<Self, SearchCdcError> {
        let payload: NotificationPayload = serde_json::from_str(payload)?;
        Ok(Self::new(
            payload.source_category,
            payload.source_id,
            payload.latest_skiptoken,
            payload.deleted,
        ))
    }

    #[must_use]
    pub fn source_category(&self) -> &str {
        &self.source_category
    }

    #[must_use]
    pub fn source_id(&self) -> Uuid {
        self.source_id
    }

    #[must_use]
    pub fn latest_skiptoken(&self) -> i64 {
        self.latest_skiptoken
    }

    #[must_use]
    pub fn deleted(&self) -> bool {
        self.deleted
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchCdcBatchConfig {
    pub flush_window: Duration,
    pub max_unique_records: usize,
}

impl Default for SearchCdcBatchConfig {
    fn default() -> Self {
        Self {
            flush_window: DEFAULT_SEARCH_CDC_FLUSH_WINDOW,
            max_unique_records: DEFAULT_SEARCH_CDC_MAX_UNIQUE_RECORDS,
        }
    }
}

#[derive(Debug)]
pub struct SearchCdcBatcher {
    config: SearchCdcBatchConfig,
    first_buffered_at: Option<Instant>,
    notifications: BTreeMap<SearchCdcRecordKey, SearchCdcNotification>,
}

impl SearchCdcBatcher {
    #[must_use]
    pub fn new(config: SearchCdcBatchConfig) -> Self {
        Self {
            config,
            first_buffered_at: None,
            notifications: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, notification: SearchCdcNotification, now: Instant) {
        self.first_buffered_at.get_or_insert(now);
        let key = SearchCdcRecordKey::from(&notification);
        self.notifications
            .entry(key)
            .and_modify(|existing| {
                if notification.latest_skiptoken > existing.latest_skiptoken {
                    *existing = notification.clone();
                }
            })
            .or_insert(notification);
    }

    #[must_use]
    pub fn should_flush(&self, now: Instant) -> bool {
        if self.notifications.is_empty() {
            return false;
        }
        self.notifications.len() >= self.config.max_unique_records
            || self
                .first_buffered_at
                .is_some_and(|first| now.duration_since(first) >= self.config.flush_window)
    }

    pub fn drain(&mut self) -> Vec<SearchCdcNotification> {
        self.first_buffered_at = None;
        std::mem::take(&mut self.notifications)
            .into_values()
            .collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }
}

#[derive(Debug)]
pub struct SearchCdcListener<'a, C> {
    pg_listener: Option<PgListener>,
    pool: PgPool,
    client: &'a C,
    search_config: SearchSyncConfig,
    batcher: SearchCdcBatcher,
}

impl<'a, C> SearchCdcListener<'a, C>
where
    C: SearchIndexClient + Sync,
{
    /// Open a dedicated `PostgreSQL` listener connection.
    ///
    /// # Errors
    ///
    /// Returns [`SearchCdcError`] when the dedicated listener connection cannot
    /// be opened or the CDC channel cannot be registered.
    pub async fn connect(
        database_url: &str,
        pool: PgPool,
        client: &'a C,
        search_config: SearchSyncConfig,
        batch_config: SearchCdcBatchConfig,
    ) -> Result<Self, SearchCdcError> {
        let mut pg_listener = PgListener::connect(database_url).await?;
        pg_listener.listen(SEARCH_CDC_CHANNEL).await?;
        Ok(Self {
            pg_listener: Some(pg_listener),
            pool,
            client,
            search_config,
            batcher: SearchCdcBatcher::new(batch_config),
        })
    }

    /// Receive one `PostgreSQL` notification and flush if the batch is ready.
    ///
    /// # Errors
    ///
    /// Returns [`SearchCdcError`] for listener, parse, or indexing failures.
    pub async fn receive_once(
        &mut self,
        now: Instant,
    ) -> Result<Option<SearchSyncReport>, SearchCdcError> {
        let notification = self
            .pg_listener
            .as_mut()
            .ok_or(SearchCdcError::ListenerNotConnected)?
            .recv()
            .await?;
        let notification = SearchCdcNotification::from_payload(notification.payload())?;
        self.batcher.push(notification, now);
        if self.batcher.should_flush(now) {
            Ok(Some(self.flush().await?))
        } else {
            Ok(None)
        }
    }

    /// Flush buffered CDC notifications through targeted search indexing.
    ///
    /// # Errors
    ///
    /// Returns [`SearchCdcError`] when targeted search indexing fails.
    pub async fn flush(&mut self) -> Result<SearchSyncReport, SearchCdcError> {
        let records = self
            .batcher
            .drain()
            .into_iter()
            .map(|notification| {
                SearchSyncRecordKey::new(
                    notification.source_category,
                    notification.source_id,
                    notification.latest_skiptoken,
                )
            })
            .collect::<Vec<_>>();
        Ok(index_records(&self.pool, self.client, &self.search_config, &records).await?)
    }
}

#[derive(Debug, Error)]
pub enum SearchCdcError {
    #[error("search CDC notification payload is invalid")]
    InvalidPayload(#[from] serde_json::Error),
    #[error("search CDC listener database operation failed")]
    Sql(#[from] sqlx::Error),
    #[error("search CDC listener is not connected")]
    ListenerNotConnected,
    #[error("search CDC targeted sync failed")]
    Sync(#[from] SearchSyncError),
}

#[derive(Deserialize)]
struct NotificationPayload {
    source_category: String,
    source_id: Uuid,
    latest_skiptoken: i64,
    deleted: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SearchCdcRecordKey {
    source_category: String,
    source_id: Uuid,
}

impl From<&SearchCdcNotification> for SearchCdcRecordKey {
    fn from(notification: &SearchCdcNotification) -> Self {
        Self {
            source_category: notification.source_category.clone(),
            source_id: notification.source_id,
        }
    }
}
