use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt::Display,
    sync::MutexGuard,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{postgres::PgListener, PgPool};
use thiserror::Error;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::search_sync::{
    index_records, SearchSyncConfig, SearchSyncError, SearchSyncRecordKey, SearchSyncReport,
};
use opentk_search::SearchIndexClient;

pub const SEARCH_CDC_CHANNEL: &str = "sync_entity_change";
pub const DEFAULT_SEARCH_CDC_FLUSH_WINDOW: Duration = Duration::from_secs(1);
pub const DEFAULT_SEARCH_CDC_MAX_UNIQUE_RECORDS: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchCdcRuntimeState {
    Starting,
    Running,
    Degraded,
    Stopped,
}

impl SearchCdcRuntimeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Degraded => "degraded",
            Self::Stopped => "stopped",
        }
    }

    #[must_use]
    pub const fn is_search_usable(self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchCdcBatchReport {
    pub indexed: u64,
    pub deleted: u64,
    pub failed: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchCdcStatus {
    pub state: SearchCdcRuntimeState,
    pub last_notification_at: Option<DateTime<Utc>>,
    pub pending_count: usize,
    pub last_batch: Option<SearchCdcBatchReport>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SearchCdcRuntimeStatus {
    inner: Arc<Mutex<SearchCdcStatus>>,
}

impl Default for SearchCdcRuntimeStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchCdcRuntimeStatus {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SearchCdcStatus {
                state: SearchCdcRuntimeState::Starting,
                last_notification_at: None,
                pending_count: 0,
                last_batch: None,
                last_error: None,
            })),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> SearchCdcStatus {
        self.lock_status().clone()
    }

    pub fn mark_running(&self) {
        let mut status = self.lock_status();
        status.state = SearchCdcRuntimeState::Running;
        status.last_error = None;
    }

    pub fn record_notification(&self, pending_count: usize) {
        let mut status = self.lock_status();
        status.last_notification_at = Some(Utc::now());
        status.pending_count = pending_count;
    }

    pub fn record_batch(&self, report: SearchCdcBatchReport) {
        let mut status = self.lock_status();
        status.state = SearchCdcRuntimeState::Running;
        status.pending_count = 0;
        status.last_batch = Some(report);
        status.last_error = None;
    }

    pub fn record_error(&self, error: &impl Display) {
        let mut status = self.lock_status();
        status.state = SearchCdcRuntimeState::Degraded;
        status.last_error = Some(error.to_string());
    }

    pub fn mark_stopped(&self) {
        let mut status = self.lock_status();
        status.state = SearchCdcRuntimeState::Stopped;
    }

    fn lock_status(&self) -> MutexGuard<'_, SearchCdcStatus> {
        match self.inner.lock() {
            Ok(status) => status,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchCdcNotification {
    schema: Option<String>,
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
            schema: None,
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
        Ok(Self {
            schema: payload.schema,
            source_category: payload.source_category,
            source_id: payload.source_id,
            latest_skiptoken: payload.latest_skiptoken,
            deleted: payload.deleted,
        })
    }

    #[must_use]
    pub fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
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

    #[must_use]
    pub fn len(&self) -> usize {
        self.notifications.len()
    }
}

#[derive(Debug)]
pub struct SearchCdcListener<'a, C> {
    pg_listener: Option<PgListener>,
    pool: PgPool,
    schema_name: String,
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
        let schema_name: String = sqlx::query_scalar("SELECT current_schema()")
            .fetch_one(&pool)
            .await?;
        Ok(Self {
            pg_listener: Some(pg_listener),
            pool,
            schema_name,
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
    pub async fn receive_once(&mut self) -> Result<Option<SearchSyncReport>, SearchCdcError> {
        let notification = loop {
            let notification = self
                .pg_listener
                .as_mut()
                .ok_or(SearchCdcError::ListenerNotConnected)?
                .recv()
                .await?;
            let notification = SearchCdcNotification::from_payload(notification.payload())?;
            if notification
                .schema()
                .is_none_or(|schema| schema == self.schema_name)
            {
                break notification;
            }
        };
        let now = Instant::now();
        self.batcher.push(notification, now);
        if self.batcher.should_flush(now) {
            Ok(Some(self.flush().await?))
        } else {
            Ok(None)
        }
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.batcher.len()
    }

    #[must_use]
    pub fn flush_window(&self) -> Duration {
        self.batcher.config.flush_window
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

/// Run the search CDC listener until shutdown, recording runtime status as it goes.
///
/// # Errors
///
/// Returns [`SearchCdcError`] when the listener cannot connect or subscribe
/// during startup. Per-notification and per-batch runtime errors are recorded
/// in `status` and do not stop the daemon.
pub async fn run_search_cdc_listener<C>(
    database_url: String,
    pool: PgPool,
    client: Arc<C>,
    search_config: SearchSyncConfig,
    batch_config: SearchCdcBatchConfig,
    mut shutdown: broadcast::Receiver<()>,
    status: SearchCdcRuntimeStatus,
) -> Result<(), SearchCdcError>
where
    C: SearchIndexClient + Sync + Send + 'static,
{
    let mut listener = SearchCdcListener::connect(
        &database_url,
        pool,
        client.as_ref(),
        search_config,
        batch_config,
    )
    .await?;
    status.mark_running();

    loop {
        tokio::select! {
            biased;
            shutdown_result = shutdown.recv() => {
                match shutdown_result {
                    Ok(()) | Err(broadcast::error::RecvError::Closed) => {
                        flush_pending(&mut listener, &status).await;
                        status.mark_stopped();
                        return Ok(());
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        status.record_error(&format!("search CDC shutdown receiver lagged by {skipped} messages"));
                        flush_pending(&mut listener, &status).await;
                        status.mark_stopped();
                        return Ok(());
                    }
                }
            }
            result = listener.receive_once() => {
                status.record_notification(listener.pending_count());
                match result {
                    Ok(Some(report)) => {
                        status.record_batch(batch_report(&report, Duration::ZERO));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::error!(%error, "search CDC listener runtime error");
                        status.record_error(&error_message(&error));
                    }
                }
            }
        }
    }
}

async fn flush_pending<C>(listener: &mut SearchCdcListener<'_, C>, status: &SearchCdcRuntimeStatus)
where
    C: SearchIndexClient + Sync,
{
    if listener.pending_count() == 0 {
        return;
    }
    let started = Instant::now();
    match listener.flush().await {
        Ok(report) => status.record_batch(batch_report(&report, started.elapsed())),
        Err(error) => {
            tracing::error!(%error, "search CDC listener failed to flush pending batch");
            status.record_error(&error_message(&error));
        }
    }
}

fn batch_report(report: &SearchSyncReport, duration: Duration) -> SearchCdcBatchReport {
    SearchCdcBatchReport {
        indexed: report.indexed,
        deleted: report.deleted,
        failed: report.failed,
        duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
    }
}

fn error_message(error: &SearchCdcError) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        message.push_str(": ");
        message.push_str(&error.to_string());
        source = error.source();
    }
    message
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
    schema: Option<String>,
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
