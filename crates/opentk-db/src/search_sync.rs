use chrono::{DateTime, Duration, Utc};
use opentk_core::official_schema;
use opentk_search::{
    meilisearch_schema, SearchCountClient, SearchFilter, SearchIndexClient, SearchIndexError,
    SearchIndexOperation, SearchIndexSchema, SearchMappingError,
};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use thiserror::Error;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, warn};
use uuid::Uuid;

use crate::read_model::{EntityChange, ReadModelError};
use crate::search_projection::{project_category_page, project_record_keys};

const DEFAULT_INDEX_NAME: &str = "opentk_entities";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSyncConfig {
    pub index_name: String,
    pub categories: Vec<String>,
    pub batch_size: i64,
    pub max_payload_bytes: usize,
    pub retry_limit: i32,
}

impl Default for SearchSyncConfig {
    fn default() -> Self {
        Self {
            index_name: DEFAULT_INDEX_NAME.to_owned(),
            categories: official_schema::entity_types()
                .iter()
                .map(|entity| entity.category.to_owned())
                .collect(),
            batch_size: 100,
            max_payload_bytes: 80_000_000,
            retry_limit: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchSyncMode {
    FullReindex,
    Incremental,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSyncReport {
    pub mode: SearchSyncMode,
    pub indexed: u64,
    pub deleted: u64,
    pub failed: u64,
    pub latest_cursors: Vec<SearchIndexCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchIndexCursor {
    pub index_name: String,
    pub source_category: String,
    pub latest_skiptoken: i64,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub state: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchIndexFailure {
    pub id: i64,
    pub index_name: String,
    pub source_category: String,
    pub source_id: Uuid,
    pub latest_skiptoken: i64,
    pub operation: String,
    pub attempt_count: i32,
    pub next_retry_at: DateTime<Utc>,
    pub error: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchCompletenessConfig {
    pub min_ratio_basis_points: u32,
    pub min_missing_documents: i64,
    pub grace_period: Duration,
}

impl Default for SearchCompletenessConfig {
    fn default() -> Self {
        Self {
            min_ratio_basis_points: 9_800,
            min_missing_documents: 100,
            grace_period: Duration::minutes(15),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchIndexCompleteness {
    pub index_name: String,
    pub source_category: String,
    pub postgres_count: i64,
    pub search_count: u64,
    pub missing_count: i64,
    pub ratio_basis_points: u32,
    pub state: SearchIndexCompletenessState,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchIndexCompletenessState {
    Ok,
    WarmingUp,
    Degraded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSyncRecordKey {
    pub(crate) source_category: String,
    pub(crate) source_id: Uuid,
    pub(crate) latest_skiptoken: i64,
}

impl SearchSyncRecordKey {
    #[must_use]
    pub fn new(source_category: String, source_id: Uuid, latest_skiptoken: i64) -> Self {
        Self {
            source_category,
            source_id,
            latest_skiptoken,
        }
    }
}

#[derive(Debug, Error)]
pub enum SearchSyncError {
    #[error("search sync batch_size must be greater than zero")]
    InvalidBatchSize,
    #[error("search sync retry_limit must be greater than zero")]
    InvalidRetryLimit,
    #[error("search sync max_payload_bytes must be greater than zero")]
    InvalidMaxPayloadBytes,
    #[error("unknown category {0}")]
    UnknownCategory(String),
    #[error("database read/write failed")]
    Sql(#[from] sqlx::Error),
    #[error("read model failed")]
    ReadModel(#[from] ReadModelError),
    #[error("search index failed")]
    Index(#[from] SearchIndexError),
    #[error("search mapping failed")]
    Mapping(#[from] SearchMappingError),
}

/// Rebuild the configured search index from the current `PostgreSQL` state.
///
/// # Errors
///
/// Returns [`SearchSyncError`] for invalid configuration, `PostgreSQL` failures,
/// source-record mapping failures, or search engine failures. Failed records are
/// also persisted in `search_index_failure` before returning.
pub async fn full_reindex<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchSyncConfig,
) -> Result<SearchSyncReport, SearchSyncError>
where
    C: SearchIndexClient + Sync,
{
    validate_config(config)?;
    let schema = schema_for_config(config);
    client.reset_index(&schema).await?;
    reset_cursors(pool, config).await?;
    run_indexing(pool, client, config, SearchSyncMode::FullReindex).await
}

/// Apply changes after the durable per-category search cursor.
///
/// # Errors
///
/// Returns [`SearchSyncError`] for invalid configuration, `PostgreSQL` failures,
/// source-record mapping failures, or search engine failures. Failed records are
/// also persisted in `search_index_failure` before returning.
pub async fn incremental_index<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchSyncConfig,
) -> Result<SearchSyncReport, SearchSyncError>
where
    C: SearchIndexClient + Sync,
{
    validate_config(config)?;
    run_indexing(pool, client, config, SearchSyncMode::Incremental).await
}

/// Apply search operations for explicit source records without advancing durable cursors.
///
/// This is the targeted CDC path for records delivered by `LISTEN`/`NOTIFY`.
/// Cursor-based indexing remains responsible for durable catch-up.
///
/// # Errors
///
/// Returns [`SearchSyncError`] for invalid configuration, unknown categories,
/// missing source records, `PostgreSQL` failures, source-record mapping
/// failures, or search engine failures.
pub async fn index_records<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchSyncConfig,
    records: &[SearchSyncRecordKey],
) -> Result<SearchSyncReport, SearchSyncError>
where
    C: SearchIndexClient + Sync,
{
    validate_config(config)?;
    info!(
        record_count = records.len(),
        "search CDC targeted changes materialization started"
    );
    let projection = project_record_keys(pool, records).await?;
    info!(
        record_count = records.len(),
        change_count = projection.changes.len(),
        "search CDC targeted changes materialized"
    );
    if projection.changes.is_empty() {
        return Ok(SearchSyncReport {
            mode: SearchSyncMode::Incremental,
            indexed: 0,
            deleted: 0,
            failed: 0,
            latest_cursors: list_cursors(pool).await?,
        });
    }

    let operations = projection.operations;

    let upsert_count = operations
        .iter()
        .filter(|operation| matches!(operation, SearchIndexOperation::Upsert(_)))
        .count();
    let delete_count = operations
        .iter()
        .filter(|operation| matches!(operation, SearchIndexOperation::Delete(_)))
        .count();
    info!(
        operation_count = operations.len(),
        upsert_count, delete_count, "Meilisearch batch apply started"
    );
    if let Err(error) = client
        .apply_batch_with_payload_limit(&operations, config.max_payload_bytes)
        .await
    {
        record_batch_failure(pool, config, &projection.changes, &error.to_string()).await?;
        return Err(error.into());
    }
    info!(
        operation_count = operations.len(),
        upsert_count, delete_count, "Meilisearch batch apply finished"
    );

    let indexed = upsert_count as u64;
    let deleted = delete_count as u64;

    Ok(SearchSyncReport {
        mode: SearchSyncMode::Incremental,
        indexed,
        deleted,
        failed: 0,
        latest_cursors: list_cursors(pool).await?,
    })
}

/// List durable indexing cursors.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when `PostgreSQL` cannot be queried.
pub async fn list_cursors(pool: &PgPool) -> Result<Vec<SearchIndexCursor>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT index_name, source_category, latest_skiptoken, last_indexed_at, state, last_error
         FROM search_index_cursor
         ORDER BY index_name, source_category",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(cursor_from_row).collect())
}

/// List durable indexing failures.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when `PostgreSQL` cannot be queried.
pub async fn list_failures(pool: &PgPool) -> Result<Vec<SearchIndexFailure>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, index_name, source_category, source_id, latest_skiptoken, operation,
                attempt_count, next_retry_at, error, created_at, updated_at
         FROM search_index_failure
         ORDER BY next_retry_at, id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(failure_from_row).collect())
}

/// Compare indexed counts against `PostgreSQL` source-of-truth live row counts.
///
/// # Errors
///
/// Returns when `PostgreSQL` cannot be queried or Meilisearch cannot provide
/// filtered document counts.
pub async fn verify_index_completeness<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchSyncConfig,
    completeness: &SearchCompletenessConfig,
) -> Result<Vec<SearchIndexCompleteness>, SearchSyncError>
where
    C: SearchCountClient + Sync + ?Sized,
{
    validate_config(config)?;
    let mut results = Vec::with_capacity(config.categories.len());
    for category in &config.categories {
        let postgres_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM sync_entity
             WHERE source_category = $1 AND deleted = false",
        )
        .bind(category)
        .fetch_one(pool)
        .await?;
        let search_count = client
            .count(SearchFilter {
                source_category: Some(category.clone()),
                entity_kind: None,
            })
            .await?;
        let search_count_i64 = i64::try_from(search_count).unwrap_or(i64::MAX);
        let missing_count = postgres_count.saturating_sub(search_count_i64).max(0);
        let ratio_basis_points = if postgres_count <= 0 {
            10_000
        } else {
            let postgres_count_u64 = u64::try_from(postgres_count).unwrap_or(u64::MAX);
            ((search_count.saturating_mul(10_000)) / postgres_count_u64)
                .try_into()
                .unwrap_or(10_000)
        };
        let cursor = cursor_snapshot(pool, &config.index_name, category).await?;
        let materially_incomplete = missing_count >= completeness.min_missing_documents
            && ratio_basis_points < completeness.min_ratio_basis_points;
        let (state, reason) = if materially_incomplete {
            if cursor_is_inside_grace(cursor.as_ref(), completeness.grace_period) {
                (
                    SearchIndexCompletenessState::WarmingUp,
                    Some(format!(
                        "index is incomplete but cursor is still inside grace period: postgres_count={postgres_count} search_count={search_count} missing_count={missing_count} ratio_basis_points={ratio_basis_points}"
                    )),
                )
            } else {
                (
                    SearchIndexCompletenessState::Degraded,
                    Some(format!(
                        "index materially incomplete: postgres_count={postgres_count} search_count={search_count} missing_count={missing_count} ratio_basis_points={ratio_basis_points}"
                    )),
                )
            }
        } else {
            (SearchIndexCompletenessState::Ok, None)
        };
        results.push(SearchIndexCompleteness {
            index_name: config.index_name.clone(),
            source_category: category.clone(),
            postgres_count,
            search_count,
            missing_count,
            ratio_basis_points,
            state,
            reason,
        });
    }
    Ok(results)
}

async fn run_indexing<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchSyncConfig,
    mode: SearchSyncMode,
) -> Result<SearchSyncReport, SearchSyncError>
where
    C: SearchIndexClient + Sync,
{
    let mut indexed = 0_u64;
    let mut deleted = 0_u64;
    let failed = 0_u64;

    for category in &config.categories {
        ensure_category(category)?;
        mark_cursor_running(pool, &config.index_name, category).await?;
        let mut after = match mode {
            SearchSyncMode::FullReindex => 0,
            SearchSyncMode::Incremental => cursor_skiptoken(pool, &config.index_name, category)
                .await?
                .unwrap_or(0),
        };

        loop {
            let page = project_category_page(pool, category, after, config.batch_size).await?;
            if page.changes.is_empty() {
                mark_cursor_caught_up(pool, &config.index_name, category, after).await?;
                break;
            }

            let operations = page.operations;
            apply_batch_with_retry(pool, client, config, category, &page.changes, &operations)
                .await?;
            for operation in &operations {
                match operation {
                    SearchIndexOperation::Upsert(_) => indexed += 1,
                    SearchIndexOperation::Delete(_) => deleted += 1,
                }
            }
            after = page
                .changes
                .last()
                .map_or(after, |item| item.latest_skiptoken);
            advance_cursor(pool, &config.index_name, category, after).await?;

            if !page.has_more {
                mark_cursor_caught_up(pool, &config.index_name, category, after).await?;
                break;
            }
        }
    }

    Ok(SearchSyncReport {
        mode,
        indexed,
        deleted,
        failed,
        latest_cursors: list_cursors(pool).await?,
    })
}

async fn apply_batch_with_retry<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchSyncConfig,
    category: &str,
    changes: &[EntityChange],
    operations: &[SearchIndexOperation],
) -> Result<(), SearchSyncError>
where
    C: SearchIndexClient + Sync,
{
    let mut attempt = 0_u32;
    loop {
        attempt += 1;
        match client
            .apply_batch_with_payload_limit(operations, config.max_payload_bytes)
            .await
        {
            Ok(()) => return Ok(()),
            Err(error) if is_transient_index_error(&error) => {
                let error_text =
                    format!("transient search index failure attempt {attempt}: {error}");
                record_batch_failure(pool, config, changes, &error_text).await?;
                mark_cursor_retrying(pool, &config.index_name, category, &error_text).await?;
                let delay = retry_delay(attempt);
                warn!(
                    source_category = %category,
                    attempt,
                    delay_ms = delay.as_millis(),
                    error = %error,
                    "transient search indexing failure; retrying same batch"
                );
                sleep(delay).await;
            }
            Err(error) => {
                record_batch_failure(pool, config, changes, &error.to_string()).await?;
                mark_cursor_error(pool, &config.index_name, category, &error.to_string()).await?;
                return Err(error.into());
            }
        }
    }
}

fn is_transient_index_error(error: &SearchIndexError) -> bool {
    match error {
        SearchIndexError::Http { status: None, .. } => true,
        SearchIndexError::Http {
            status: Some(status),
            ..
        } => matches!(*status, 408 | 425 | 429) || *status >= 500,
        SearchIndexError::InvalidResponse(message) => {
            let message = message.to_ascii_lowercase();
            message.contains("timeout")
                || message.contains("timed out")
                || message.contains("temporarily unavailable")
        }
        SearchIndexError::Mapping(_) | SearchIndexError::PayloadTooLarge { .. } => false,
    }
}

fn retry_delay(attempt: u32) -> TokioDuration {
    let seconds = 2_u64
        .saturating_pow(attempt.saturating_sub(1).min(6))
        .min(60);
    TokioDuration::from_secs(seconds)
}

async fn reset_cursors(pool: &PgPool, config: &SearchSyncConfig) -> Result<(), sqlx::Error> {
    for category in &config.categories {
        sqlx::query(
            "INSERT INTO search_index_cursor (
                index_name,
                source_category,
                latest_skiptoken,
                state,
                last_error
             )
             VALUES ($1, $2, 0, 'running', NULL)
             ON CONFLICT (index_name, source_category) DO UPDATE
             SET latest_skiptoken = 0,
                 state = 'running',
                 last_error = NULL",
        )
        .bind(&config.index_name)
        .bind(category)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn mark_cursor_running(
    pool: &PgPool,
    index_name: &str,
    category: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO search_index_cursor (
            index_name,
            source_category,
            latest_skiptoken,
            state,
            last_error
         )
         VALUES ($1, $2, 0, 'running', NULL)
         ON CONFLICT (index_name, source_category) DO UPDATE
         SET state = 'running',
             last_error = NULL",
    )
    .bind(index_name)
    .bind(category)
    .execute(pool)
    .await?;
    Ok(())
}

async fn advance_cursor(
    pool: &PgPool,
    index_name: &str,
    category: &str,
    latest_skiptoken: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE search_index_cursor
         SET latest_skiptoken = $3,
             last_indexed_at = $4,
             state = 'running',
             last_error = NULL
         WHERE index_name = $1 AND source_category = $2",
    )
    .bind(index_name)
    .bind(category)
    .bind(latest_skiptoken)
    .bind(Utc::now())
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_cursor_caught_up(
    pool: &PgPool,
    index_name: &str,
    category: &str,
    latest_skiptoken: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO search_index_cursor (
            index_name,
            source_category,
            latest_skiptoken,
            last_indexed_at,
            state,
            last_error
         )
         VALUES ($1, $2, $3, $4, 'caught_up', NULL)
         ON CONFLICT (index_name, source_category) DO UPDATE
         SET latest_skiptoken = EXCLUDED.latest_skiptoken,
             last_indexed_at = EXCLUDED.last_indexed_at,
             state = EXCLUDED.state,
             last_error = NULL",
    )
    .bind(index_name)
    .bind(category)
    .bind(latest_skiptoken)
    .bind(Utc::now())
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_cursor_error(
    pool: &PgPool,
    index_name: &str,
    category: &str,
    error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE search_index_cursor
         SET state = 'error',
             last_error = $3
         WHERE index_name = $1 AND source_category = $2",
    )
    .bind(index_name)
    .bind(category)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_cursor_retrying(
    pool: &PgPool,
    index_name: &str,
    category: &str,
    error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE search_index_cursor
         SET state = 'retrying',
             last_error = $3
         WHERE index_name = $1 AND source_category = $2",
    )
    .bind(index_name)
    .bind(category)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

async fn cursor_skiptoken(
    pool: &PgPool,
    index_name: &str,
    category: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT latest_skiptoken
         FROM search_index_cursor
         WHERE index_name = $1 AND source_category = $2",
    )
    .bind(index_name)
    .bind(category)
    .fetch_optional(pool)
    .await
}

async fn cursor_snapshot(
    pool: &PgPool,
    index_name: &str,
    category: &str,
) -> Result<Option<SearchIndexCursor>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT index_name, source_category, latest_skiptoken, last_indexed_at, state, last_error
         FROM search_index_cursor
         WHERE index_name = $1 AND source_category = $2",
    )
    .bind(index_name)
    .bind(category)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(cursor_from_row))
}

fn cursor_is_inside_grace(cursor: Option<&SearchIndexCursor>, grace_period: Duration) -> bool {
    let Some(cursor) = cursor else {
        return false;
    };
    if !matches!(cursor.state.as_str(), "running" | "retrying") {
        return false;
    }
    let Some(last_indexed_at) = cursor.last_indexed_at else {
        return false;
    };
    Utc::now() - last_indexed_at < grace_period
}

async fn record_batch_failure(
    pool: &PgPool,
    config: &SearchSyncConfig,
    changes: &[EntityChange],
    error: &str,
) -> Result<(), sqlx::Error> {
    if changes.is_empty() {
        return Ok(());
    }
    let now = Utc::now();
    let next_retry_at = now + Duration::minutes(i64::from(config.retry_limit));
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO search_index_failure (
                index_name,
                source_category,
                source_id,
                latest_skiptoken,
                operation,
                attempt_count,
                next_retry_at,
                error,
                created_at,
                updated_at
             )
             ",
    );
    builder.push_values(changes, |mut row, change| {
        let operation = if change.deleted { "delete" } else { "upsert" };
        row.push_bind(&config.index_name)
            .push_bind(&change.category)
            .push_bind(change.source_id)
            .push_bind(change.latest_skiptoken)
            .push_bind(operation)
            .push_bind(1_i32)
            .push_bind(next_retry_at)
            .push_bind(error)
            .push_bind(now)
            .push_bind(now);
    });
    builder.push(
        " ON CONFLICT (index_name, source_category, source_id, latest_skiptoken, operation)
          DO UPDATE
          SET attempt_count = search_index_failure.attempt_count + 1,
              next_retry_at = EXCLUDED.next_retry_at,
              error = EXCLUDED.error,
              updated_at = EXCLUDED.updated_at",
    );
    builder.build().execute(pool).await?;
    Ok(())
}

fn schema_for_config(config: &SearchSyncConfig) -> SearchIndexSchema {
    let mut schema = meilisearch_schema();
    schema.index_name = Box::leak(config.index_name.clone().into_boxed_str());
    schema
}

fn validate_config(config: &SearchSyncConfig) -> Result<(), SearchSyncError> {
    if config.batch_size <= 0 {
        return Err(SearchSyncError::InvalidBatchSize);
    }
    if config.retry_limit <= 0 {
        return Err(SearchSyncError::InvalidRetryLimit);
    }
    if config.max_payload_bytes == 0 {
        return Err(SearchSyncError::InvalidMaxPayloadBytes);
    }
    for category in &config.categories {
        ensure_category(category)?;
    }
    Ok(())
}

fn ensure_category(category: &str) -> Result<(), SearchSyncError> {
    if official_schema::entity_named(category).is_some() {
        Ok(())
    } else {
        Err(SearchSyncError::UnknownCategory(category.to_owned()))
    }
}

fn cursor_from_row(row: &sqlx::postgres::PgRow) -> SearchIndexCursor {
    SearchIndexCursor {
        index_name: row.get("index_name"),
        source_category: row.get("source_category"),
        latest_skiptoken: row.get("latest_skiptoken"),
        last_indexed_at: row.get("last_indexed_at"),
        state: row.get("state"),
        last_error: row.get("last_error"),
    }
}

fn failure_from_row(row: &sqlx::postgres::PgRow) -> SearchIndexFailure {
    SearchIndexFailure {
        id: row.get("id"),
        index_name: row.get("index_name"),
        source_category: row.get("source_category"),
        source_id: row.get("source_id"),
        latest_skiptoken: row.get("latest_skiptoken"),
        operation: row.get("operation"),
        attempt_count: row.get("attempt_count"),
        next_retry_at: row.get("next_retry_at"),
        error: row.get("error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
