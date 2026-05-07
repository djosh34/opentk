use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration as StdDuration, Instant},
};

use chrono::{DateTime, NaiveDate, Utc};
use opentk_core::official_schema;
use opentk_search::{
    map_record_to_operation, SearchDocumentContent, SearchEntityMetadata, SearchIndexOperation,
    SearchMappingError, SearchRelationLabel, SearchSourceRecord,
};
use serde_json::{Map, Number, Value};
use sqlx::{postgres::PgRow, PgPool, Row};
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    postgres_schema::{self, ColumnSpec, SchemaSpec, SqlType, TableKind, TableSpec},
    read_model::{EntityChange, ReadModelError},
};

pub struct SearchProjectionPage {
    pub changes: Vec<EntityChange>,
    pub operations: Vec<SearchIndexOperation>,
    pub row_count: usize,
    pub upsert_count: usize,
    pub delete_count: usize,
    pub skipped_count: usize,
    pub latest_skiptoken: i64,
    pub has_more: bool,
    pub sql_duration: StdDuration,
}

#[derive(Debug, Error)]
pub enum SearchProjectionError {
    #[error("unknown search source category {0}")]
    UnknownCategory(String),
    #[error("database read failed")]
    Sql(#[from] sqlx::Error),
    #[error("read model lookup failed")]
    ReadModel(#[from] ReadModelError),
    #[error("search mapping failed")]
    Mapping(#[from] SearchMappingError),
}

/// Project one cursor page for a source category into search index operations.
///
/// # Errors
///
/// Returns [`SearchProjectionError`] when the category is unknown, `PostgreSQL`
/// cannot be queried, source timestamps cannot be decoded, or search document
/// mapping rejects a projected row.
pub async fn project_category_page(
    pool: &PgPool,
    category: &str,
    after: i64,
    batch_size: i64,
) -> Result<SearchProjectionPage, SearchProjectionError> {
    ensure_category(category)?;
    let started = Instant::now();
    let limit = batch_size.saturating_add(1);
    let rows = sqlx::query(
        "SELECT source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at
         FROM sync_entity
         WHERE source_category = $1 AND latest_skiptoken > $2
         ORDER BY latest_skiptoken, source_id
         LIMIT $3",
    )
    .bind(category)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let has_more = rows.len() > usize::try_from(batch_size).unwrap_or(usize::MAX);
    let changes = rows
        .iter()
        .take(usize::try_from(batch_size).unwrap_or(usize::MAX))
        .map(entity_change)
        .collect::<Vec<_>>();
    project_changes(pool, changes, has_more, started).await
}

/// Project all rows in an inclusive skiptoken window.
///
/// The reconciler picks target boundaries by `latest_skiptoken`, not by a
/// `(latest_skiptoken, source_id)` tuple. Loading the full window prevents rows
/// tied at the target boundary from being skipped when the reconciler advances
/// the verified prefix to that boundary.
///
/// # Errors
///
/// Returns [`SearchProjectionError`] when the category is unknown, `PostgreSQL`
/// cannot be queried, source timestamps cannot be decoded, or search document
/// mapping rejects a projected row.
pub async fn project_category_window(
    pool: &PgPool,
    category: &str,
    after: i64,
    target_boundary: i64,
) -> Result<SearchProjectionPage, SearchProjectionError> {
    ensure_category(category)?;
    let started = Instant::now();
    let rows = sqlx::query(
        "SELECT source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at
         FROM sync_entity
         WHERE source_category = $1
           AND latest_skiptoken > $2
           AND latest_skiptoken <= $3
         ORDER BY latest_skiptoken, source_id",
    )
    .bind(category)
    .bind(after)
    .bind(target_boundary)
    .fetch_all(pool)
    .await?;
    let changes = rows.iter().map(entity_change).collect::<Vec<_>>();
    project_changes(pool, changes, false, started).await
}

async fn project_changes(
    pool: &PgPool,
    changes: Vec<EntityChange>,
    has_more: bool,
    started: Instant,
) -> Result<SearchProjectionPage, SearchProjectionError> {
    let mut grouped = BTreeMap::<String, Vec<EntityChange>>::new();
    for change in changes {
        grouped
            .entry(change.category.clone())
            .or_default()
            .push(change);
    }

    let mut all_changes = Vec::new();
    let mut operations = Vec::new();
    let mut skipped_count = 0_usize;
    for (category, changes) in grouped {
        let records = source_records_for_category(pool, &category, &changes).await?;
        for change in changes {
            all_changes.push(change.clone());
            let Some(record) = records.get(&change.source_id) else {
                skipped_count += 1;
                warn!(
                    source_category = %change.category,
                    source_id = %change.source_id,
                    latest_skiptoken = change.latest_skiptoken,
                    "skipping search sync record until entity detail materializes"
                );
                continue;
            };
            operations.push(map_record_to_operation(record)?);
        }
    }
    all_changes.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then(left.latest_skiptoken.cmp(&right.latest_skiptoken))
            .then(left.source_id.cmp(&right.source_id))
    });
    let latest_skiptoken = all_changes
        .last()
        .map_or(0, |change| change.latest_skiptoken);
    let upsert_count = operations
        .iter()
        .filter(|operation| matches!(operation, SearchIndexOperation::Upsert(_)))
        .count();
    let delete_count = operations.len().saturating_sub(upsert_count);
    let sql_duration = started.elapsed();
    info!(
        source_row_count = all_changes.len(),
        upsert_count,
        delete_count,
        skipped_count,
        sql_projection_ms = sql_duration.as_millis(),
        latest_skiptoken,
        has_more,
        "search SQL projection page completed"
    );

    Ok(SearchProjectionPage {
        row_count: all_changes.len(),
        changes: all_changes,
        operations,
        upsert_count,
        delete_count,
        skipped_count,
        latest_skiptoken,
        has_more,
        sql_duration,
    })
}

async fn source_records_for_category(
    pool: &PgPool,
    category: &str,
    changes: &[EntityChange],
) -> Result<BTreeMap<Uuid, SearchSourceRecord>, SearchProjectionError> {
    let source_ids = changes
        .iter()
        .filter(|change| !change.deleted)
        .map(|change| change.source_id)
        .collect::<Vec<_>>();
    let entity_fields = entity_fields(pool, category, &source_ids).await?;
    let document_content = if category == "Document" {
        document_content(pool, &source_ids).await?
    } else {
        BTreeMap::new()
    };
    let relations = bulk_rel_labels(pool, category, &source_ids).await?;

    let mut records = BTreeMap::new();
    for change in changes {
        let source_updated_at = parse_timestamp(&change.source_updated_at)?;
        let atom_updated_at = parse_timestamp(&change.atom_updated_at)?;
        if change.deleted {
            records.insert(
                change.source_id,
                SearchSourceRecord {
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
                },
            );
            continue;
        }
        let Some(fields) = entity_fields.get(&change.source_id) else {
            continue;
        };
        records.insert(
            change.source_id,
            SearchSourceRecord {
                metadata: SearchEntityMetadata {
                    category: change.category.clone(),
                    source_id: change.source_id,
                    latest_skiptoken: change.latest_skiptoken,
                    deleted: false,
                    source_updated_at,
                    atom_updated_at,
                },
                fields: fields.clone(),
                document_content: document_content.get(&change.source_id).cloned(),
                relations: relations
                    .get(&change.source_id)
                    .cloned()
                    .unwrap_or_default(),
            },
        );
    }
    Ok(records)
}

async fn entity_fields(
    pool: &PgPool,
    category: &str,
    source_ids: &[Uuid],
) -> Result<BTreeMap<Uuid, Map<String, Value>>, SearchProjectionError> {
    if source_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let table = entity_table(category)?;
    let sql = format!(
        "SELECT * FROM {} WHERE source_category = $1 AND source_id = ANY($2::uuid[])",
        ident(&table.name)
    );
    let rows = sqlx::query(&sql)
        .bind(category)
        .bind(source_ids)
        .fetch_all(pool)
        .await?;
    rows.into_iter()
        .map(|row| Ok((row.get("source_id"), scalar_fields(&row, table)?)))
        .collect()
}

async fn document_content(
    pool: &PgPool,
    source_ids: &[Uuid],
) -> Result<BTreeMap<Uuid, SearchDocumentContent>, SearchProjectionError> {
    if source_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows = sqlx::query(
        "SELECT DISTINCT ON (document_source_id)
             document_source_id,
             selected_source_url,
             selected_source_content_type,
             official_source,
             extraction_status,
             validation_status,
             output_hash,
             extracted_text,
             extracted_html
         FROM document_content
         WHERE document_source_category = 'Document'
           AND document_source_id = ANY($1::uuid[])
         ORDER BY document_source_id,
                  official_source DESC NULLS LAST,
                  source_rank ASC NULLS LAST,
                  extracted_at DESC NULLS LAST,
                  id DESC NULLS LAST",
    )
    .bind(source_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get("document_source_id"),
                SearchDocumentContent {
                    selected_source_url: row.get("selected_source_url"),
                    selected_source_content_type: row.get("selected_source_content_type"),
                    official_source: row.get("official_source"),
                    extraction_status: row.get("extraction_status"),
                    validation_status: row.get("validation_status"),
                    output_hash: row.get("output_hash"),
                    extracted_text: row.get("extracted_text"),
                    extracted_html: row.get("extracted_html"),
                },
            )
        })
        .collect())
}

async fn bulk_rel_labels(
    pool: &PgPool,
    category: &str,
    source_ids: &[Uuid],
) -> Result<BTreeMap<Uuid, Vec<SearchRelationLabel>>, SearchProjectionError> {
    if source_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let relation_tables = relation_tables_for_source(category);
    if relation_tables.is_empty() {
        return Ok(BTreeMap::new());
    }

    let union = relation_tables
        .iter()
        .map(|table| {
            format!(
                "SELECT source_id, relation_name, target_category, target_id FROM {} \
                 WHERE source_category = $1 AND source_id = ANY($2::uuid[]) \
                   AND NOT (target_category = source_category AND target_id = source_id)",
                ident(&table.name)
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let rows = sqlx::query(&union)
        .bind(category)
        .bind(source_ids)
        .fetch_all(pool)
        .await?;
    let mut target_ids_by_category = BTreeMap::<String, Vec<Uuid>>::new();
    for row in &rows {
        target_ids_by_category
            .entry(row.get("target_category"))
            .or_default()
            .push(row.get("target_id"));
    }
    for target_ids in target_ids_by_category.values_mut() {
        target_ids.sort_unstable();
        target_ids.dedup();
    }

    let mut labels_by_target = BTreeMap::<(String, Uuid), String>::new();
    for (target_category, target_ids) in target_ids_by_category {
        match entity_fields(pool, &target_category, &target_ids).await {
            Ok(fields_by_id) => {
                for target_id in target_ids {
                    let label = fields_by_id
                        .get(&target_id)
                        .and_then(|fields| title_from_fields(&target_category, fields))
                        .unwrap_or_else(|| fallback_label(&target_category, target_id));
                    labels_by_target.insert((target_category.clone(), target_id), label);
                }
            }
            Err(SearchProjectionError::ReadModel(ReadModelError::UnknownCategory(_))) => {
                for target_id in target_ids {
                    labels_by_target.insert(
                        (target_category.clone(), target_id),
                        fallback_label(&target_category, target_id),
                    );
                }
            }
            Err(error) => return Err(error),
        }
    }

    let mut dedupe = BTreeSet::new();
    let mut labels = BTreeMap::<Uuid, Vec<SearchRelationLabel>>::new();
    for row in rows {
        let source_id = row.get("source_id");
        let relation_name = row.get::<String, _>("relation_name");
        let target_category = row.get::<String, _>("target_category");
        let target_id = row.get("target_id");
        let label = labels_by_target
            .get(&(target_category.clone(), target_id))
            .cloned()
            .unwrap_or_else(|| fallback_label(&target_category, target_id));
        if !dedupe.insert((
            source_id,
            relation_name.clone(),
            target_category.clone(),
            target_id,
            label.clone(),
        )) {
            continue;
        }
        labels
            .entry(source_id)
            .or_default()
            .push(SearchRelationLabel {
                relation_name,
                target_category,
                target_id,
                label,
            });
    }
    Ok(labels)
}

fn entity_change(row: &PgRow) -> EntityChange {
    EntityChange {
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
    }
}

fn scalar_fields(
    row: &PgRow,
    table: &TableSpec,
) -> Result<Map<String, Value>, SearchProjectionError> {
    let mut fields = Map::new();
    for column in &table.columns {
        if matches!(
            column.name.as_str(),
            "source_category"
                | "source_id"
                | "latest_skiptoken"
                | "deleted"
                | "source_updated_at"
                | "atom_updated_at"
        ) {
            continue;
        }
        fields.insert(column.name.clone(), cell_value(row, column)?);
    }
    Ok(fields)
}

fn cell_value(row: &PgRow, column: &ColumnSpec) -> Result<Value, sqlx::Error> {
    Ok(match column.sql_type {
        SqlType::Uuid => row
            .try_get::<Option<Uuid>, _>(column.name.as_str())?
            .map_or(Value::Null, |value| Value::String(value.to_string())),
        SqlType::Text | SqlType::Jsonb => row
            .try_get::<Option<String>, _>(column.name.as_str())?
            .map_or(Value::Null, Value::String),
        SqlType::Boolean => row
            .try_get::<Option<bool>, _>(column.name.as_str())?
            .map_or(Value::Null, Value::Bool),
        SqlType::Integer => row
            .try_get::<Option<i32>, _>(column.name.as_str())?
            .map_or(Value::Null, |value| Value::Number(Number::from(value))),
        SqlType::BigInteger | SqlType::BigIdentity => row
            .try_get::<Option<i64>, _>(column.name.as_str())?
            .map_or(Value::Null, |value| Value::Number(Number::from(value))),
        SqlType::TimestampTz => row
            .try_get::<Option<DateTime<Utc>>, _>(column.name.as_str())?
            .map_or(Value::Null, |value| Value::String(value.to_rfc3339())),
        SqlType::Date => row
            .try_get::<Option<NaiveDate>, _>(column.name.as_str())?
            .map_or(Value::Null, |value| Value::String(value.to_string())),
    })
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

fn entity_table(category: &str) -> Result<&'static TableSpec, SearchProjectionError> {
    read_schema()
        .tables
        .iter()
        .find(|table| {
            matches!(
                table.kind,
                TableKind::Entity {
                    category: table_category
                } if table_category == category
            )
        })
        .ok_or_else(|| ReadModelError::UnknownCategory(category.to_owned()).into())
}

fn relation_tables_for_source(category: &str) -> Vec<&'static TableSpec> {
    read_schema()
        .tables
        .iter()
        .filter(|table| {
            matches!(
                table.kind,
                TableKind::Relation {
                    source_category,
                    ..
                } if source_category == category
            )
        })
        .collect()
}

fn ensure_category(category: &str) -> Result<(), SearchProjectionError> {
    if official_schema::entity_named(category).is_some() {
        Ok(())
    } else {
        Err(SearchProjectionError::UnknownCategory(category.to_owned()))
    }
}

fn read_schema() -> &'static SchemaSpec {
    static SCHEMA: std::sync::OnceLock<SchemaSpec> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(postgres_schema::schema)
}

fn ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
