use std::{future::Future, pin::Pin, time::Duration};

use chrono::{DateTime, Utc};
use reqwest::Url;
use thiserror::Error;
use tokio::{task::JoinSet, time::sleep};
use uuid::Uuid;

use crate::{
    payload::{parse_entry_payload, ParsedEntity, PayloadParseError},
    syncfeed::{SyncFeedClient, SyncFeedClientError, SyncFeedCursor},
};

pub type SyncStoreFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, SyncStoreError>> + Send + 'a>>;

#[derive(Clone, Debug)]
pub struct CompleteSyncRunner<S> {
    pub client: SyncFeedClient,
    pub store: S,
    pub config: CompleteSyncConfig,
}

#[derive(Clone, Debug)]
pub struct CompleteSyncConfig {
    pub categories: Vec<String>,
    pub mode: SyncRunMode,
    pub poll_interval: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncRunMode {
    UntilCaughtUp,
    Continuous,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategorySyncReport {
    pub category: String,
    pub pages_written: u64,
    pub entities_seen: u64,
    pub caught_up: bool,
    pub latest_skiptoken: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteSyncReport {
    pub categories: Vec<CategorySyncReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredCategoryCursor {
    pub category: String,
    pub latest_skiptoken: i64,
    pub next_url: Url,
    pub caught_up: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSyncPage {
    pub category: String,
    pub latest_skiptoken: i64,
    pub next_url: Url,
    pub atom_updated_at: DateTime<Utc>,
    pub entities: Vec<ParsedEntity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncStoreWriteOutcome {
    pub entities_seen: usize,
    pub entities_written: usize,
    pub entities_deleted: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryStatus {
    pub category: String,
    pub latest_skiptoken: Option<i64>,
    pub state: CategorySyncState,
    pub lag: Option<Duration>,
    pub last_fetch_at: Option<DateTime<Utc>>,
    pub last_error: Option<DurableSyncError>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CategorySyncState {
    NotStarted,
    Running,
    CaughtUp,
    Error,
}

impl CategorySyncState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Running => "running",
            Self::CaughtUp => "caught_up",
            Self::Error => "error",
        }
    }

    /// Parse durable state text.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown state values.
    pub fn parse(value: &str) -> Result<Self, SyncStoreError> {
        match value {
            "not_started" => Ok(Self::NotStarted),
            "running" => Ok(Self::Running),
            "caught_up" => Ok(Self::CaughtUp),
            "error" => Ok(Self::Error),
            _ => Err(SyncStoreError {
                message: format!("unknown sync category state {value:?}"),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableSyncError {
    pub phase: SyncPhase,
    pub category: String,
    pub skiptoken: Option<i64>,
    pub entity_id: Option<Uuid>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncPhase {
    Fetch,
    Parse,
    Write,
    MarkCaughtUp,
}

impl SyncPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Parse => "parse",
            Self::Write => "write",
            Self::MarkCaughtUp => "mark_caught_up",
        }
    }

    /// Parse durable phase text.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown phase values.
    pub fn parse(value: &str) -> Result<Self, SyncStoreError> {
        match value {
            "fetch" => Ok(Self::Fetch),
            "parse" => Ok(Self::Parse),
            "write" => Ok(Self::Write),
            "mark_caught_up" => Ok(Self::MarkCaughtUp),
            _ => Err(SyncStoreError {
                message: format!("unknown sync error phase {value:?}"),
            }),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message}")]
pub struct SyncStoreError {
    pub message: String,
}

#[derive(Debug, Error)]
pub enum CompleteSyncError {
    #[error("SyncFeed fetch failed during {phase:?} for {category}: {source}")]
    Fetch {
        phase: SyncPhase,
        category: String,
        #[source]
        source: Box<SyncFeedClientError>,
    },
    #[error("payload parse failed for {category}: {source}")]
    Parse {
        category: String,
        #[source]
        source: Box<PayloadParseError>,
    },
    #[error("sync store failed during {phase:?} for {category}: {source}")]
    Store {
        phase: SyncPhase,
        category: String,
        #[source]
        source: SyncStoreError,
    },
    #[error("SyncFeed page for {category} at {request_url} did not include a next cursor")]
    MissingNext {
        category: String,
        request_url: String,
    },
    #[error("SyncFeed empty page for {category} at {request_url} did not include resume")]
    MissingResume {
        category: String,
        request_url: String,
    },
    #[error("category worker failed to join: {message}")]
    WorkerJoin { message: String },
}

pub trait SyncStore: Clone + Send + Sync + 'static {
    fn load_category_cursor<'a>(
        &'a self,
        category: &'a str,
    ) -> SyncStoreFuture<'a, Option<StoredCategoryCursor>>;

    fn write_page(&self, page: PreparedSyncPage) -> SyncStoreFuture<'_, SyncStoreWriteOutcome>;

    fn mark_caught_up<'a>(
        &'a self,
        category: &'a str,
        resume_url: Url,
        observed_at: DateTime<Utc>,
    ) -> SyncStoreFuture<'a, ()>;

    fn record_error(&self, error: DurableSyncError) -> SyncStoreFuture<'_, ()>;

    fn status<'a>(&'a self, categories: &'a [String]) -> SyncStoreFuture<'a, Vec<CategoryStatus>>;
}

impl<S: SyncStore> CompleteSyncRunner<S> {
    /// Run configured categories until each reaches `resume`.
    ///
    /// # Errors
    ///
    /// Returns fetch, parse, storage, cursor, or worker join errors. The runner
    /// records durable errors before returning whenever the storage boundary is
    /// available.
    pub async fn run_once(&self) -> Result<CompleteSyncReport, CompleteSyncError> {
        let mut workers = JoinSet::new();
        for category in &self.config.categories {
            let client = self.client.clone();
            let store = self.store.clone();
            let category = category.clone();
            let mode = self.config.mode;
            let poll_interval = self.config.poll_interval;
            workers.spawn(async move {
                run_category(client, store, category, mode, poll_interval).await
            });
        }

        let mut categories = Vec::new();
        while let Some(result) = workers.join_next().await {
            categories.push(result.map_err(|source| CompleteSyncError::WorkerJoin {
                message: source.to_string(),
            })??);
        }
        categories.sort_by(|left, right| left.category.cmp(&right.category));
        Ok(CompleteSyncReport { categories })
    }

    /// Poll forever with the same cursor semantics as `run_once`.
    ///
    /// # Errors
    ///
    /// Returns the first error from a polling cycle.
    pub async fn run_forever(&self) -> Result<(), CompleteSyncError> {
        loop {
            self.run_once().await?;
            sleep(self.config.poll_interval).await;
        }
    }
}

async fn run_category<S: SyncStore>(
    client: SyncFeedClient,
    store: S,
    category: String,
    mode: SyncRunMode,
    poll_interval: Duration,
) -> Result<CategorySyncReport, CompleteSyncError> {
    let stored = store
        .load_category_cursor(&category)
        .await
        .map_err(|source| CompleteSyncError::Store {
            phase: SyncPhase::Fetch,
            category: category.clone(),
            source,
        })?;
    if matches!(mode, SyncRunMode::UntilCaughtUp)
        && stored.as_ref().is_some_and(|cursor| cursor.caught_up)
    {
        return Ok(CategorySyncReport {
            category: category.clone(),
            pages_written: 0,
            entities_seen: 0,
            caught_up: true,
            latest_skiptoken: stored.map(|cursor| cursor.latest_skiptoken),
        });
    }

    let mut cursor = start_cursor(&client, &category, stored)?;
    let mut report = CategorySyncReport {
        category: category.clone(),
        pages_written: 0,
        entities_seen: 0,
        caught_up: false,
        latest_skiptoken: None,
    };

    loop {
        let current_skiptoken = skiptoken_from_url(&cursor.url);
        let page = match client.fetch_page(cursor.clone()).await {
            Ok(page) => page,
            Err(source) => {
                record_error(
                    &store,
                    DurableSyncError {
                        phase: SyncPhase::Fetch,
                        category: category.clone(),
                        skiptoken: current_skiptoken,
                        entity_id: None,
                        message: source.to_string(),
                    },
                )
                .await?;
                return Err(CompleteSyncError::Fetch {
                    phase: SyncPhase::Fetch,
                    category,
                    source: Box::new(source),
                });
            }
        };

        if page.entries.is_empty() {
            cursor = handle_empty_page(&store, &category, page, mode, poll_interval, &mut report)
                .await?;
            if report.caught_up && matches!(mode, SyncRunMode::UntilCaughtUp) {
                return Ok(report);
            }
            continue;
        }

        let next = next_cursor(&category, &page)?;
        let latest_skiptoken = latest_skiptoken(&category, &page, &next)?;
        let entities = parse_entities(&store, &category, &page, latest_skiptoken).await?;
        let outcome =
            write_prepared_page(&store, &category, latest_skiptoken, &next, entities).await?;
        report.pages_written += 1;
        report.entities_seen += u64::try_from(outcome.entities_seen).unwrap_or(u64::MAX);
        report.latest_skiptoken = Some(latest_skiptoken);
        cursor = next;
    }
}

fn start_cursor(
    client: &SyncFeedClient,
    category: &str,
    stored: Option<StoredCategoryCursor>,
) -> Result<SyncFeedCursor, CompleteSyncError> {
    if let Some(stored) = stored {
        SyncFeedCursor::from_url(
            client.base_url(),
            category.to_owned(),
            stored.next_url,
            client.content_mode(),
        )
        .map_err(|source| CompleteSyncError::Fetch {
            phase: SyncPhase::Fetch,
            category: category.to_owned(),
            source: Box::new(source),
        })
    } else {
        Ok(SyncFeedCursor::first_page(
            client.base_url(),
            category.to_owned(),
            client.content_mode(),
        ))
    }
}

async fn handle_empty_page<S: SyncStore>(
    store: &S,
    category: &str,
    page: crate::syncfeed::SyncFeedPage,
    mode: SyncRunMode,
    poll_interval: Duration,
    report: &mut CategorySyncReport,
) -> Result<SyncFeedCursor, CompleteSyncError> {
    let resume = page
        .resume
        .ok_or_else(|| CompleteSyncError::MissingResume {
            category: category.to_owned(),
            request_url: page.request_url.to_string(),
        })?;
    if let Err(source) = store
        .mark_caught_up(category, resume.url.clone(), Utc::now())
        .await
    {
        record_error(
            store,
            DurableSyncError {
                phase: SyncPhase::MarkCaughtUp,
                category: category.to_owned(),
                skiptoken: skiptoken_from_url(&resume.url),
                entity_id: None,
                message: source.to_string(),
            },
        )
        .await?;
        return Err(CompleteSyncError::Store {
            phase: SyncPhase::MarkCaughtUp,
            category: category.to_owned(),
            source,
        });
    }
    report.caught_up = true;
    report.latest_skiptoken = skiptoken_from_url(&resume.url);
    if matches!(mode, SyncRunMode::Continuous) {
        sleep(poll_interval).await;
    }
    Ok(resume)
}

fn next_cursor(
    category: &str,
    page: &crate::syncfeed::SyncFeedPage,
) -> Result<SyncFeedCursor, CompleteSyncError> {
    page.next_request
        .clone()
        .ok_or_else(|| CompleteSyncError::MissingNext {
            category: category.to_owned(),
            request_url: page.request_url.to_string(),
        })
}

fn latest_skiptoken(
    category: &str,
    page: &crate::syncfeed::SyncFeedPage,
    next: &SyncFeedCursor,
) -> Result<i64, CompleteSyncError> {
    skiptoken_from_url(&next.url).ok_or_else(|| CompleteSyncError::MissingNext {
        category: category.to_owned(),
        request_url: page.request_url.to_string(),
    })
}

async fn parse_entities<S: SyncStore>(
    store: &S,
    category: &str,
    page: &crate::syncfeed::SyncFeedPage,
    latest_skiptoken: i64,
) -> Result<Vec<ParsedEntity>, CompleteSyncError> {
    let mut entities = Vec::with_capacity(page.entries.len());
    for entry in &page.entries {
        match parse_entry_payload(entry) {
            Ok(entity) => entities.push(entity),
            Err(source) => {
                record_error(
                    store,
                    DurableSyncError {
                        phase: SyncPhase::Parse,
                        category: category.to_owned(),
                        skiptoken: Some(latest_skiptoken),
                        entity_id: None,
                        message: source.to_string(),
                    },
                )
                .await?;
                return Err(CompleteSyncError::Parse {
                    category: category.to_owned(),
                    source: Box::new(source),
                });
            }
        }
    }
    Ok(entities)
}

async fn write_prepared_page<S: SyncStore>(
    store: &S,
    category: &str,
    latest_skiptoken: i64,
    next: &SyncFeedCursor,
    entities: Vec<ParsedEntity>,
) -> Result<SyncStoreWriteOutcome, CompleteSyncError> {
    match store
        .write_page(PreparedSyncPage {
            category: category.to_owned(),
            latest_skiptoken,
            next_url: next.url.clone(),
            atom_updated_at: Utc::now(),
            entities,
        })
        .await
    {
        Ok(outcome) => Ok(outcome),
        Err(source) => {
            record_error(
                store,
                DurableSyncError {
                    phase: SyncPhase::Write,
                    category: category.to_owned(),
                    skiptoken: Some(latest_skiptoken),
                    entity_id: None,
                    message: source.to_string(),
                },
            )
            .await?;
            Err(CompleteSyncError::Store {
                phase: SyncPhase::Write,
                category: category.to_owned(),
                source,
            })
        }
    }
}

async fn record_error<S: SyncStore>(
    store: &S,
    error: DurableSyncError,
) -> Result<(), CompleteSyncError> {
    let category = error.category.clone();
    store
        .record_error(error)
        .await
        .map_err(|source| CompleteSyncError::Store {
            phase: SyncPhase::Write,
            category,
            source,
        })
}

#[must_use]
pub fn skiptoken_from_url(url: &Url) -> Option<i64> {
    url.query_pairs()
        .find(|(key, _)| key == "skiptoken")
        .and_then(|(_, value)| value.parse().ok())
}
