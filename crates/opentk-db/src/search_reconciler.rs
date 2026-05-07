use std::time::{Duration, Instant};

use opentk_core::official_schema;
use opentk_search::{
    meilisearch_schema, SearchIndexClient, SearchIndexError, SearchIndexOperation,
    SearchReconcilerClient,
};
use serde_json::Value;
use sqlx::{types::Json, PgPool, Row};
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::postgres_schema;
use crate::search_projection::{project_category_window, SearchProjectionError};

const DEFAULT_INDEX_NAME: &str = "opentk_entities";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchReconcilerConfig {
    pub index_name: String,
    pub categories: Vec<String>,
    pub batch_size: i64,
    pub max_payload_bytes: usize,
    pub loop_interval: Duration,
}

impl Default for SearchReconcilerConfig {
    fn default() -> Self {
        Self {
            index_name: DEFAULT_INDEX_NAME.to_owned(),
            categories: official_schema::entity_types()
                .iter()
                .map(|entity| entity.category.to_owned())
                .collect(),
            batch_size: 100,
            max_payload_bytes: 80_000_000,
            loop_interval: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchReconcilerReport {
    pub categories: Vec<SearchReconcilerCategoryReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchReconcilerCategoryReport {
    pub source_category: String,
    pub postgres_count: i64,
    pub meilisearch_count: u64,
    pub highest_meilisearch_skiptoken: Option<i64>,
    pub verified_prefix_boundary: i64,
    pub target_boundary: Option<i64>,
    pub scratch_row_count: i64,
    pub payload_bytes: i64,
    pub inserted_rows: u64,
    pub deleted_rows: u64,
    pub completed: bool,
    pub loop_duration_ms: u128,
}

#[derive(Debug, Error)]
pub enum SearchReconcilerError {
    #[error("search reconciler batch_size must be greater than zero")]
    InvalidBatchSize,
    #[error("search reconciler max_payload_bytes must be greater than zero")]
    InvalidMaxPayloadBytes,
    #[error("database read/write failed")]
    Sql(#[from] sqlx::Error),
    #[error("search index failed")]
    Index(#[from] SearchIndexError),
    #[error("search projection failed")]
    Projection(#[from] SearchProjectionError),
}

/// Run one reconciliation pass over every configured category.
///
/// # Errors
///
/// Returns [`SearchReconcilerError`] when configuration is invalid, `PostgreSQL`
/// cannot be queried or updated, source rows cannot be projected, or
/// Meilisearch rejects schema, count, highest-skiptoken, or write requests.
pub async fn reconcile_once<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchReconcilerConfig,
) -> Result<SearchReconcilerReport, SearchReconcilerError>
where
    C: SearchIndexClient + SearchReconcilerClient + Sync,
{
    validate_config(config)?;
    client.ensure_index(&schema_for_config(config)).await?;

    let mut categories = Vec::with_capacity(config.categories.len());
    for category in &config.categories {
        categories.push(reconcile_category(pool, client, config, category).await?);
    }
    Ok(SearchReconcilerReport { categories })
}

#[allow(clippy::too_many_lines)]
async fn reconcile_category<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchReconcilerConfig,
    category: &str,
) -> Result<SearchReconcilerCategoryReport, SearchReconcilerError>
where
    C: SearchIndexClient + SearchReconcilerClient + Sync,
{
    let started = Instant::now();
    let mut totals = ScratchStats::default();
    let mut last_target_boundary = None;
    loop {
        let highest = client.highest_skiptoken(category).await?;
        let highest_boundary = highest.unwrap_or(0);
        let postgres_prefix = postgres_count(pool, category, Some(highest_boundary)).await?;
        let meili_prefix = client
            .count_category_prefix(category, Some(highest_boundary))
            .await?;
        let verified_prefix_boundary = if postgres_prefix == meili_prefix_i64(meili_prefix) {
            highest_boundary
        } else {
            warn!(
                source_category = category,
                highest_meilisearch_skiptoken = highest_boundary,
                postgres_prefix_count = postgres_prefix,
                meilisearch_prefix_count = meili_prefix,
                "search prefix mismatch detected; binary-search repair starts"
            );
            let boundary = find_verified_prefix(pool, client, category, highest_boundary).await?;
            client.delete_category_after(category, boundary).await?;
            boundary
        };

        let mut current_boundary = verified_prefix_boundary;
        while let Some(target_boundary) =
            next_boundary(pool, category, current_boundary, config.batch_size).await?
        {
            totals.add(
                apply_window(
                    pool,
                    client,
                    config,
                    category,
                    current_boundary,
                    target_boundary,
                )
                .await?,
            );
            current_boundary = target_boundary;
            last_target_boundary = Some(target_boundary);
        }

        let mut postgres_total = postgres_count(pool, category, None).await?;
        let mut meili_total = client.count_category_prefix(category, None).await?;
        let mut completed = postgres_total == meili_prefix_i64(meili_total);
        if completed && postgres_total > 0 && verified_prefix_boundary > 0 {
            info!(
                source_category = category,
                verified_prefix_boundary,
                "search counts match; refreshing full category to overwrite same-count stale documents"
            );
            let mut refresh_boundary = 0_i64;
            while let Some(target_boundary) =
                next_boundary(pool, category, refresh_boundary, config.batch_size).await?
            {
                totals.add(
                    apply_window(
                        pool,
                        client,
                        config,
                        category,
                        refresh_boundary,
                        target_boundary,
                    )
                    .await?,
                );
                refresh_boundary = target_boundary;
                last_target_boundary = Some(target_boundary);
            }
            postgres_total = postgres_count(pool, category, None).await?;
            meili_total = client.count_category_prefix(category, None).await?;
            completed = postgres_total == meili_prefix_i64(meili_total);
        }

        if completed {
            info!(
                source_category = category,
                postgres_count = postgres_total,
                meilisearch_count = meili_total,
                highest_meilisearch_skiptoken = highest,
                verified_prefix_boundary,
                target_boundary = last_target_boundary,
                scratch_row_count = totals.row_count,
                payload_bytes = totals.payload_bytes,
                inserted_rows = totals.upsert_count,
                deleted_rows = totals.delete_count,
                completed,
                loop_duration_ms = started.elapsed().as_millis(),
                "search reconciler category pass completed"
            );

            return Ok(SearchReconcilerCategoryReport {
                source_category: category.to_owned(),
                postgres_count: postgres_total,
                meilisearch_count: meili_total,
                highest_meilisearch_skiptoken: highest,
                verified_prefix_boundary,
                target_boundary: last_target_boundary,
                scratch_row_count: totals.row_count,
                payload_bytes: totals.payload_bytes,
                inserted_rows: totals.upsert_count,
                deleted_rows: totals.delete_count,
                completed,
                loop_duration_ms: started.elapsed().as_millis(),
            });
        }

        warn!(
            source_category = category,
            postgres_count = postgres_total,
            meilisearch_count = meili_total,
            highest_meilisearch_skiptoken = highest,
            verified_prefix_boundary,
            target_boundary = last_target_boundary,
            scratch_row_count = totals.row_count,
            payload_bytes = totals.payload_bytes,
            inserted_rows = totals.upsert_count,
            deleted_rows = totals.delete_count,
            loop_duration_ms = started.elapsed().as_millis(),
            "search reconciler category still mismatched after pass; retrying category"
        );
    }
}

async fn find_verified_prefix<C>(
    pool: &PgPool,
    client: &C,
    category: &str,
    high: i64,
) -> Result<i64, SearchReconcilerError>
where
    C: SearchReconcilerClient + Sync,
{
    let mut low = 0_i64;
    let mut high = high.max(0);
    let mut best = 0_i64;
    while low <= high {
        let mid = low + ((high - low) / 2);
        let postgres = postgres_count(pool, category, Some(mid)).await?;
        let meili = client.count_category_prefix(category, Some(mid)).await?;
        if postgres == meili_prefix_i64(meili) {
            best = mid;
            low = mid.saturating_add(1);
        } else {
            high = mid.saturating_sub(1);
        }
    }
    info!(
        source_category = category,
        verified_prefix_boundary = best,
        "search prefix repair boundary selected"
    );
    Ok(best)
}

async fn postgres_count(
    pool: &PgPool,
    category: &str,
    boundary: Option<i64>,
) -> Result<i64, sqlx::Error> {
    let table_name = quote_identifier(&postgres_schema::sql_name(category));
    if let Some(boundary) = boundary {
        sqlx::query_scalar(&format!(
            "SELECT COUNT(*)
             FROM sync_entity AS s
             JOIN {table_name} AS entity
               ON entity.source_category = s.source_category
              AND entity.source_id = s.source_id
             WHERE s.source_category = $1
               AND s.deleted = false
               AND s.latest_skiptoken <= $2"
        ))
        .bind(category)
        .bind(boundary)
        .fetch_one(pool)
        .await
    } else {
        sqlx::query_scalar(&format!(
            "SELECT COUNT(*)
             FROM sync_entity AS s
             JOIN {table_name} AS entity
               ON entity.source_category = s.source_category
              AND entity.source_id = s.source_id
             WHERE s.source_category = $1
               AND s.deleted = false"
        ))
        .bind(category)
        .fetch_one(pool)
        .await
    }
}

async fn next_boundary(
    pool: &PgPool,
    category: &str,
    after: i64,
    batch_size: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT latest_skiptoken
         FROM (
             SELECT latest_skiptoken
             FROM sync_entity
             WHERE source_category = $1
               AND latest_skiptoken > $2
             ORDER BY latest_skiptoken, source_id
             LIMIT $3
         ) AS batch
         ORDER BY latest_skiptoken DESC
         LIMIT 1",
    )
    .bind(category)
    .bind(after)
    .bind(batch_size)
    .fetch_optional(pool)
    .await
}

async fn replace_scratch(
    pool: &PgPool,
    config: &SearchReconcilerConfig,
    category: &str,
    after: i64,
) -> Result<(), sqlx::Error> {
    clear_scratch(pool, &config.index_name, category).await?;
    info!(
        source_category = category,
        index_name = config.index_name,
        verified_prefix_boundary = after,
        "search reconciler scratch replaced for next batch"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScratchStats {
    row_count: i64,
    payload_bytes: i64,
    upsert_count: u64,
    delete_count: u64,
}

impl ScratchStats {
    fn add(&mut self, other: Self) {
        self.row_count = self.row_count.saturating_add(other.row_count);
        self.payload_bytes = self.payload_bytes.saturating_add(other.payload_bytes);
        self.upsert_count = self.upsert_count.saturating_add(other.upsert_count);
        self.delete_count = self.delete_count.saturating_add(other.delete_count);
    }
}

async fn apply_window<C>(
    pool: &PgPool,
    client: &C,
    config: &SearchReconcilerConfig,
    category: &str,
    after: i64,
    target_boundary: i64,
) -> Result<ScratchStats, SearchReconcilerError>
where
    C: SearchIndexClient + SearchReconcilerClient + Sync,
{
    replace_scratch(pool, config, category, after).await?;
    let scratch = populate_scratch(pool, config, category, after, target_boundary).await?;
    let operations = operations_from_scratch(pool, &config.index_name, category).await?;
    client
        .apply_batch_with_payload_limit(&operations, config.max_payload_bytes)
        .await?;
    clear_scratch(pool, &config.index_name, category).await?;
    Ok(scratch)
}

async fn populate_scratch(
    pool: &PgPool,
    config: &SearchReconcilerConfig,
    category: &str,
    after: i64,
    target_boundary: i64,
) -> Result<ScratchStats, SearchReconcilerError> {
    let page = project_category_window(pool, category, after, target_boundary).await?;
    let mut upsert_count = 0_u64;
    let mut delete_count = 0_u64;
    let mut payload_bytes = 0_i64;
    for operation in page.operations {
        match operation {
            SearchIndexOperation::Upsert(document) => {
                let payload = serde_json::to_value(document.as_ref()).map_err(|error| {
                    SearchIndexError::InvalidResponse(format!(
                        "failed to serialize search document for scratch: {error}"
                    ))
                })?;
                let bytes = i64::try_from(
                    serde_json::to_vec(&payload)
                        .map_err(|error| {
                            SearchIndexError::InvalidResponse(format!(
                                "failed to measure search document for scratch: {error}"
                            ))
                        })?
                        .len(),
                )
                .unwrap_or(i64::MAX);
                insert_scratch_row(
                    pool,
                    &config.index_name,
                    &document.source_category,
                    document.source_id,
                    document.latest_skiptoken,
                    "upsert",
                    &document.id,
                    Some(payload),
                    bytes,
                )
                .await?;
                upsert_count += 1;
                payload_bytes = payload_bytes.saturating_add(bytes);
            }
            SearchIndexOperation::Delete(document_id) => {
                let Some((source_category, source_id)) = parse_document_id(&document_id) else {
                    continue;
                };
                let latest_skiptoken =
                    latest_skiptoken_for_source(pool, &source_category, source_id).await?;
                insert_scratch_row(
                    pool,
                    &config.index_name,
                    &source_category,
                    source_id,
                    latest_skiptoken,
                    "delete",
                    &document_id,
                    None,
                    0,
                )
                .await?;
                delete_count += 1;
            }
        }
    }
    Ok(ScratchStats {
        row_count: i64::try_from(upsert_count.saturating_add(delete_count)).unwrap_or(i64::MAX),
        payload_bytes,
        upsert_count,
        delete_count,
    })
}

#[allow(clippy::too_many_arguments)]
async fn insert_scratch_row(
    pool: &PgPool,
    index_name: &str,
    source_category: &str,
    source_id: Uuid,
    latest_skiptoken: i64,
    operation: &str,
    document_id: &str,
    payload: Option<Value>,
    payload_bytes: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO search_reconciler_scratch (
             index_name, source_category, source_id, latest_skiptoken, operation,
             document_id, payload, payload_bytes, created_at
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())",
    )
    .bind(index_name)
    .bind(source_category)
    .bind(source_id)
    .bind(latest_skiptoken)
    .bind(operation)
    .bind(document_id)
    .bind(payload.map(Json))
    .bind(payload_bytes)
    .execute(pool)
    .await?;
    Ok(())
}

async fn operations_from_scratch(
    pool: &PgPool,
    index_name: &str,
    category: &str,
) -> Result<Vec<SearchIndexOperation>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT operation, document_id, payload
         FROM search_reconciler_scratch
         WHERE index_name = $1 AND source_category = $2
         ORDER BY latest_skiptoken, source_id",
    )
    .bind(index_name)
    .bind(category)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let operation: String = row.get("operation");
            let document_id: String = row.get("document_id");
            if operation == "delete" {
                return Ok(SearchIndexOperation::Delete(document_id));
            }
            let payload: Json<Value> = row.get("payload");
            let document = serde_json::from_value(payload.0).map_err(|error| {
                sqlx::Error::Decode(format!("invalid scratch search payload: {error}").into())
            })?;
            Ok(SearchIndexOperation::Upsert(Box::new(document)))
        })
        .collect()
}

async fn clear_scratch(pool: &PgPool, index_name: &str, category: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM search_reconciler_scratch
         WHERE index_name = $1 AND source_category = $2",
    )
    .bind(index_name)
    .bind(category)
    .execute(pool)
    .await?;
    Ok(())
}

async fn latest_skiptoken_for_source(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT latest_skiptoken
         FROM sync_entity
         WHERE source_category = $1 AND source_id = $2",
    )
    .bind(category)
    .bind(source_id)
    .fetch_one(pool)
    .await
}

fn parse_document_id(document_id: &str) -> Option<(String, Uuid)> {
    let (category, source_id) = document_id.split_once('_')?;
    Some((category.to_owned(), source_id.parse().ok()?))
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn schema_for_config(config: &SearchReconcilerConfig) -> opentk_search::SearchIndexSchema {
    let base = meilisearch_schema();
    opentk_search::SearchIndexSchema {
        index_name: Box::leak(config.index_name.clone().into_boxed_str()),
        ..base
    }
}

fn validate_config(config: &SearchReconcilerConfig) -> Result<(), SearchReconcilerError> {
    if config.batch_size <= 0 {
        return Err(SearchReconcilerError::InvalidBatchSize);
    }
    if config.max_payload_bytes == 0 {
        return Err(SearchReconcilerError::InvalidMaxPayloadBytes);
    }
    Ok(())
}

fn meili_prefix_i64(count: u64) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}
