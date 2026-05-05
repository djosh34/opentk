use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use opentk_core::official_schema;
use opentk_search::{
    map_record_to_operation, meilisearch_schema, SearchDocumentContent, SearchEntityMetadata,
    SearchIndexClient, SearchIndexError, SearchIndexOperation, SearchIndexSchema,
    SearchMappingError, SearchRelationLabel, SearchSourceRecord,
};
use serde_json::{Map, Value};
use sqlx::{PgPool, Row};
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::read_model::{self, EntityChange, ReadModelError, RelationDirection};

const DEFAULT_INDEX_NAME: &str = "opentk_entities";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSyncConfig {
    pub index_name: String,
    pub categories: Vec<String>,
    pub batch_size: i64,
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
pub struct SearchSyncRecordKey {
    source_category: String,
    source_id: Uuid,
    latest_skiptoken: i64,
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
    let changes = targeted_changes(pool, records).await?;
    info!(
        record_count = records.len(),
        change_count = changes.len(),
        "search CDC targeted changes materialized"
    );
    if changes.is_empty() {
        return Ok(SearchSyncReport {
            mode: SearchSyncMode::Incremental,
            indexed: 0,
            deleted: 0,
            failed: 0,
            latest_cursors: list_cursors(pool).await?,
        });
    }

    let operations = operations_for_changes(pool, &changes).await;
    let operations = match operations {
        Ok(operations) => operations,
        Err(error) => {
            record_batch_failure(pool, config, &changes, &error.to_string()).await?;
            return Err(error);
        }
    };

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
    if let Err(error) = client.apply_batch(&operations).await {
        record_batch_failure(pool, config, &changes, &error.to_string()).await?;
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

async fn targeted_changes(
    pool: &PgPool,
    records: &[SearchSyncRecordKey],
) -> Result<Vec<EntityChange>, SearchSyncError> {
    let mut deduped = BTreeMap::new();
    for record in records {
        ensure_category(&record.source_category)?;
        deduped
            .entry((record.source_category.clone(), record.source_id))
            .and_modify(|latest_skiptoken| {
                if record.latest_skiptoken > *latest_skiptoken {
                    *latest_skiptoken = record.latest_skiptoken;
                }
            })
            .or_insert(record.latest_skiptoken);
    }

    let mut changes = Vec::with_capacity(deduped.len());
    for ((source_category, source_id), _latest_skiptoken) in deduped {
        changes.push(targeted_change(pool, &source_category, source_id).await?);
    }
    Ok(changes)
}

async fn targeted_change(
    pool: &PgPool,
    source_category: &str,
    source_id: Uuid,
) -> Result<EntityChange, SearchSyncError> {
    let row = sqlx::query(
        "SELECT source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at
         FROM sync_entity
         WHERE source_category = $1 AND source_id = $2",
    )
    .bind(source_category)
    .bind(source_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ReadModelError::NotFound)?;

    Ok(EntityChange {
        category: row.get("source_category"),
        source_id: row.get("source_id"),
        latest_skiptoken: row.get("latest_skiptoken"),
        deleted: row.get("deleted"),
        source_updated_at: row
            .get::<Option<DateTime<Utc>>, _>("source_updated_at")
            .expect("sync_entity.source_updated_at is non-null")
            .to_rfc3339(),
        atom_updated_at: row
            .get::<Option<DateTime<Utc>>, _>("atom_updated_at")
            .expect("sync_entity.atom_updated_at is non-null")
            .to_rfc3339(),
    })
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
            let page =
                read_model::list_changes(pool, category, Some(after), config.batch_size).await?;
            if page.items.is_empty() {
                mark_cursor_caught_up(pool, &config.index_name, category, after).await?;
                break;
            }

            let operations = operations_for_changes(pool, &page.items).await;
            match operations {
                Ok(operations) => {
                    let batch_result = client.apply_batch(&operations).await;
                    if let Err(error) = batch_result {
                        record_batch_failure(pool, config, &page.items, &error.to_string()).await?;
                        mark_cursor_error(pool, &config.index_name, category, &error.to_string())
                            .await?;
                        return Err(error.into());
                    }
                    for operation in &operations {
                        match operation {
                            SearchIndexOperation::Upsert(_) => indexed += 1,
                            SearchIndexOperation::Delete(_) => deleted += 1,
                        }
                    }
                    after = page
                        .items
                        .last()
                        .map_or(after, |item| item.latest_skiptoken);
                    advance_cursor(pool, &config.index_name, category, after).await?;
                }
                Err(error) => {
                    record_batch_failure(pool, config, &page.items, &error.to_string()).await?;
                    mark_cursor_error(pool, &config.index_name, category, &error.to_string())
                        .await?;
                    return Err(error);
                }
            }

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

async fn operations_for_changes(
    pool: &PgPool,
    changes: &[EntityChange],
) -> Result<Vec<SearchIndexOperation>, SearchSyncError> {
    let mut operations = Vec::with_capacity(changes.len());
    for change in changes {
        let Some(record) = source_record(pool, change).await? else {
            continue;
        };
        operations.push(map_record_to_operation(&record)?);
    }
    Ok(operations)
}

async fn source_record(
    pool: &PgPool,
    change: &EntityChange,
) -> Result<Option<SearchSourceRecord>, SearchSyncError> {
    let source_updated_at = parse_timestamp(&change.source_updated_at)?;
    let atom_updated_at = parse_timestamp(&change.atom_updated_at)?;
    if change.deleted {
        return Ok(Some(SearchSourceRecord {
            metadata: SearchEntityMetadata {
                category: change.category.clone(),
                source_id: change.source_id,
                latest_skiptoken: change.latest_skiptoken,
                deleted: true,
                source_updated_at,
                atom_updated_at,
            },
            fields: Map::new(),
            document_content: None,
            relations: Vec::new(),
        }));
    }

    let detail = match read_model::get_entity_detail(pool, &change.category, change.source_id).await
    {
        Ok(detail) => detail,
        Err(ReadModelError::NotFound) => {
            warn!(
                source_category = %change.category,
                source_id = %change.source_id,
                latest_skiptoken = change.latest_skiptoken,
                "skipping search sync record until entity detail materializes"
            );
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let document_content = if change.category == "Document" {
        search_document_content(pool, change).await?
    } else {
        None
    };
    let relations = relation_labels(pool, &change.category, change.source_id).await?;

    Ok(Some(SearchSourceRecord {
        metadata: SearchEntityMetadata {
            category: change.category.clone(),
            source_id: change.source_id,
            latest_skiptoken: change.latest_skiptoken,
            deleted: false,
            source_updated_at,
            atom_updated_at,
        },
        fields: detail.fields,
        document_content,
        relations,
    }))
}

async fn search_document_content(
    pool: &PgPool,
    change: &EntityChange,
) -> Result<Option<SearchDocumentContent>, SearchSyncError> {
    let detail = match read_model::get_document_content(pool, change.source_id).await {
        Ok(detail) => detail,
        Err(ReadModelError::DocumentContentNotFound) => {
            warn!(
                source_category = %change.category,
                source_id = %change.source_id,
                latest_skiptoken = change.latest_skiptoken,
                "indexing document without extracted content"
            );
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    Ok(Some(SearchDocumentContent {
        selected_source_url: detail.content.selected_source_url,
        selected_source_content_type: detail.content.selected_source_content_type,
        official_source: detail.content.official_source,
        extraction_status: detail.content.extraction_status,
        validation_status: detail.content.validation_status,
        output_hash: detail.content.output_hash,
        extracted_text: detail.content.extracted_text,
        extracted_html: detail.content.extracted_html,
    }))
}

async fn relation_labels(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
) -> Result<Vec<SearchRelationLabel>, SearchSyncError> {
    let relations =
        read_model::list_relations(pool, category, source_id, RelationDirection::Both).await?;
    let mut labels = Vec::with_capacity(relations.len());
    for relation in relations {
        let label = match read_model::get_entity_detail(
            pool,
            &relation.target_category,
            relation.target_id,
        )
        .await
        {
            Ok(detail) => title_from_fields(&relation.target_category, &detail.fields)
                .unwrap_or_else(|| fallback_label(&relation.target_category, relation.target_id)),
            Err(ReadModelError::NotFound | ReadModelError::UnknownCategory(_)) => {
                fallback_label(&relation.target_category, relation.target_id)
            }
            Err(error) => return Err(error.into()),
        };
        labels.push(SearchRelationLabel {
            relation_name: relation.relation_name,
            target_category: relation.target_category,
            target_id: relation.target_id,
            label,
        });
    }
    Ok(labels)
}

fn title_from_fields(category: &str, fields: &Map<String, Value>) -> Option<String> {
    for field in ["titel", "onderwerp", "nummer", "document_nummer", "naam_nl"] {
        if let Some(value) = fields
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_owned());
        }
    }
    if category == "Persoon" {
        let first = string_field(fields, "roepnaam").or_else(|| string_field(fields, "initialen"));
        let display = [
            first,
            string_field(fields, "tussenvoegsel"),
            string_field(fields, "achternaam"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        if !display.is_empty() {
            return Some(display);
        }
    }
    None
}

fn string_field(fields: &Map<String, Value>, field: &str) -> Option<String> {
    fields
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn fallback_label(category: &str, source_id: Uuid) -> String {
    format!("{category} {source_id}")
}

fn parse_timestamp(timestamp: &str) -> Result<DateTime<Utc>, sqlx::Error> {
    timestamp
        .parse()
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))
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

async fn record_batch_failure(
    pool: &PgPool,
    config: &SearchSyncConfig,
    changes: &[EntityChange],
    error: &str,
) -> Result<(), sqlx::Error> {
    for change in changes {
        let operation = if change.deleted { "delete" } else { "upsert" };
        let now = Utc::now();
        let next_retry_at = now + Duration::minutes(i64::from(config.retry_limit));
        sqlx::query(
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
             VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $8)
             ON CONFLICT (index_name, source_category, source_id, latest_skiptoken, operation)
             DO UPDATE
             SET attempt_count = search_index_failure.attempt_count + 1,
                 next_retry_at = EXCLUDED.next_retry_at,
                 error = EXCLUDED.error,
                 updated_at = EXCLUDED.updated_at",
        )
        .bind(&config.index_name)
        .bind(&change.category)
        .bind(change.source_id)
        .bind(change.latest_skiptoken)
        .bind(operation)
        .bind(next_retry_at)
        .bind(error)
        .bind(now)
        .execute(pool)
        .await?;
    }
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
