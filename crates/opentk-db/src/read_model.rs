use chrono::{DateTime, NaiveDate, Utc};
use opentk_core::official_schema::{self, FieldKind};
use serde_json::{Map, Number, Value};
use sqlx::{postgres::PgRow, PgPool, QueryBuilder, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::postgres_schema::{self, ColumnSpec, SqlType, TableKind, TableSpec};

const MIN_LIMIT: i64 = 1;
const MAX_LIMIT: i64 = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryMetadata {
    pub category: String,
    pub table: String,
    pub field_count: usize,
    pub relation_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryProgress {
    pub category: String,
    pub latest_skiptoken: Option<i64>,
    pub state: Option<String>,
    pub last_fetch_at: Option<String>,
    pub last_synced_at: Option<String>,
    pub caught_up_at: Option<String>,
    pub next_url: Option<String>,
    pub resume_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityChange {
    pub category: String,
    pub source_id: Uuid,
    pub latest_skiptoken: i64,
    pub deleted: bool,
    pub source_updated_at: String,
    pub atom_updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadPage<T> {
    pub items: Vec<T>,
    pub next_skiptoken: Option<i64>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDetail {
    pub metadata: EntityChange,
    pub fields: Map<String, Value>,
}

pub type DocumentDetail = EntityDetail;
pub type ActivityDetail = EntityDetail;
pub type PersonDetail = EntityDetail;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentContentDetail {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub asset: DocumentAssetDetail,
    pub content: DocumentContentRow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentAssetDetail {
    pub id: i64,
    pub asset_url: String,
    pub upstream_url: String,
    pub upstream_content_type: Option<String>,
    pub upstream_content_length: Option<i64>,
    pub upstream_last_modified_at: Option<String>,
    pub retrieval_status: String,
    pub retrieval_error: Option<String>,
    pub retrieved_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentContentRow {
    pub id: i64,
    pub selected_source_url: String,
    pub selected_source_content_type: Option<String>,
    pub selected_source_content_length: Option<i64>,
    pub official_source: bool,
    pub source_rank: i32,
    pub extraction_status: String,
    pub validation_status: String,
    pub extraction_tool: String,
    pub extraction_tool_version: String,
    pub source_hash: String,
    pub output_hash: Option<String>,
    pub extraction_error: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub extracted_at: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationDirection {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationRow {
    pub source_category: String,
    pub source_id: Uuid,
    pub relation_name: String,
    pub target_category: String,
    pub target_id: Uuid,
    pub ordinal: i32,
    pub source_updated_at: String,
}

#[derive(Debug, Error)]
pub enum ReadModelError {
    #[error("unknown category {0}")]
    UnknownCategory(String),
    #[error("entity not found")]
    NotFound,
    #[error("document content not found")]
    DocumentContentNotFound,
    #[error("limit must be between {MIN_LIMIT} and {MAX_LIMIT}")]
    InvalidLimit,
    #[error("database read failed")]
    Sql(#[from] sqlx::Error),
}

#[must_use]
pub fn list_category_metadata() -> Vec<CategoryMetadata> {
    official_schema::entity_types()
        .iter()
        .map(|entity| CategoryMetadata {
            category: entity.category.to_owned(),
            table: postgres_schema::sql_name(entity.category),
            field_count: entity
                .fields
                .iter()
                .filter(|field| field.kind == FieldKind::Attribute)
                .count(),
            relation_count: entity
                .fields
                .iter()
                .filter(|field| field.kind == FieldKind::Relation)
                .count(),
        })
        .collect()
}

/// Lists persisted category progress for every official category.
///
/// # Errors
///
/// Returns [`ReadModelError::Sql`] when `PostgreSQL` cannot be queried.
pub async fn list_category_progress(
    pool: &PgPool,
) -> Result<Vec<CategoryProgress>, ReadModelError> {
    let rows = sqlx::query(
        "SELECT source_category, latest_skiptoken, state, last_fetch_at, last_synced_at,
                caught_up_at, next_url, resume_url
         FROM sync_category
         ORDER BY source_category",
    )
    .fetch_all(pool)
    .await?;

    let mut by_category = rows
        .into_iter()
        .map(|row| {
            let category = row.get::<String, _>("source_category");
            (
                category.clone(),
                CategoryProgress {
                    category,
                    latest_skiptoken: Some(row.get::<i64, _>("latest_skiptoken")),
                    state: Some(row.get::<String, _>("state")),
                    last_fetch_at: timestamp_cell(&row, "last_fetch_at"),
                    last_synced_at: timestamp_cell(&row, "last_synced_at"),
                    caught_up_at: timestamp_cell(&row, "caught_up_at"),
                    next_url: row.get::<Option<String>, _>("next_url"),
                    resume_url: row.get::<Option<String>, _>("resume_url"),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    Ok(official_schema::entity_types()
        .iter()
        .map(|entity| {
            by_category
                .remove(entity.category)
                .unwrap_or_else(|| CategoryProgress {
                    category: entity.category.to_owned(),
                    latest_skiptoken: None,
                    state: None,
                    last_fetch_at: None,
                    last_synced_at: None,
                    caught_up_at: None,
                    next_url: None,
                    resume_url: None,
                })
        })
        .collect())
}

/// Lists current entity changes for one category after an exclusive skiptoken.
///
/// # Errors
///
/// Returns validation errors for unknown categories or invalid limits, and
/// [`ReadModelError::Sql`] when `PostgreSQL` cannot be queried.
pub async fn list_changes(
    pool: &PgPool,
    category: &str,
    after: Option<i64>,
    limit: i64,
) -> Result<ReadPage<EntityChange>, ReadModelError> {
    entity_table(category)?;
    if !(MIN_LIMIT..=MAX_LIMIT).contains(&limit) {
        return Err(ReadModelError::InvalidLimit);
    }

    let rows = sqlx::query(
        "SELECT source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at
         FROM sync_entity
         WHERE source_category = $1 AND ($2::bigint IS NULL OR latest_skiptoken > $2)
         ORDER BY latest_skiptoken, source_id
         LIMIT $3",
    )
    .bind(category)
    .bind(after)
    .bind(limit + 1)
    .fetch_all(pool)
    .await?;

    let limit_usize = usize::try_from(limit).map_err(|_| ReadModelError::InvalidLimit)?;
    let has_more = rows.len() > limit_usize;
    let items = rows
        .into_iter()
        .take(limit_usize)
        .map(|row| entity_change(&row))
        .collect::<Vec<_>>();
    let next_skiptoken = has_more
        .then(|| items.last().map(|item| item.latest_skiptoken))
        .flatten();

    Ok(ReadPage {
        items,
        next_skiptoken,
        has_more,
    })
}

/// Loads one generic entity detail row.
///
/// # Errors
///
/// Returns validation errors for unknown categories or missing entities, and
/// [`ReadModelError::Sql`] when `PostgreSQL` cannot be queried.
pub async fn get_entity_detail(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
) -> Result<EntityDetail, ReadModelError> {
    get_detail(pool, category, source_id).await
}

/// Loads one document detail row.
///
/// # Errors
///
/// Returns [`ReadModelError::NotFound`] for missing rows and
/// [`ReadModelError::Sql`] when `PostgreSQL` cannot be queried.
pub async fn get_document_detail(
    pool: &PgPool,
    source_id: Uuid,
) -> Result<DocumentDetail, ReadModelError> {
    get_detail(pool, "Document", source_id).await
}

/// Loads the selected content row and its source asset for one document.
///
/// # Errors
///
/// Returns [`ReadModelError::NotFound`] when the document row does not exist,
/// [`ReadModelError::DocumentContentNotFound`] when the document exists without
/// extracted content, and [`ReadModelError::Sql`] when `PostgreSQL` cannot be
/// queried.
pub async fn get_document_content(
    pool: &PgPool,
    source_id: Uuid,
) -> Result<DocumentContentDetail, ReadModelError> {
    let row = sqlx::query(
        "SELECT
            d.source_category AS document_source_category,
            d.source_id AS document_source_id,
            a.id AS asset_id,
            a.asset_url,
            a.upstream_url,
            a.upstream_content_type,
            a.upstream_content_length,
            a.upstream_last_modified_at,
            a.retrieval_status,
            a.retrieval_error,
            a.retrieved_at,
            c.id AS content_id,
            c.selected_source_url,
            c.selected_source_content_type,
            c.selected_source_content_length,
            c.official_source,
            c.source_rank,
            c.extraction_status,
            c.validation_status,
            c.extraction_tool,
            c.extraction_tool_version,
            c.source_hash,
            c.output_hash,
            c.extraction_error,
            c.extracted_text,
            c.extracted_html,
            c.extracted_at
         FROM document d
         LEFT JOIN document_content c
           ON c.document_source_category = d.source_category
          AND c.document_source_id = d.source_id
         LEFT JOIN document_asset a ON a.id = c.document_asset_id
         WHERE d.source_category = 'Document' AND d.source_id = $1
         ORDER BY c.official_source DESC NULLS LAST,
                  c.source_rank ASC NULLS LAST,
                  c.extracted_at DESC NULLS LAST,
                  c.id DESC NULLS LAST
         LIMIT 1",
    )
    .bind(source_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ReadModelError::NotFound)?;

    if row.get::<Option<i64>, _>("content_id").is_none() {
        return Err(ReadModelError::DocumentContentNotFound);
    }

    Ok(document_content_detail(&row))
}

/// Loads one activity detail row.
///
/// # Errors
///
/// Returns [`ReadModelError::NotFound`] for missing rows and
/// [`ReadModelError::Sql`] when `PostgreSQL` cannot be queried.
pub async fn get_activity_detail(
    pool: &PgPool,
    source_id: Uuid,
) -> Result<ActivityDetail, ReadModelError> {
    get_detail(pool, "Activiteit", source_id).await
}

/// Loads one person detail row.
///
/// # Errors
///
/// Returns [`ReadModelError::NotFound`] for missing rows and
/// [`ReadModelError::Sql`] when `PostgreSQL` cannot be queried.
pub async fn get_person_detail(
    pool: &PgPool,
    source_id: Uuid,
) -> Result<PersonDetail, ReadModelError> {
    get_detail(pool, "Persoon", source_id).await
}

/// Lists outgoing, incoming, or both relation rows for an entity.
///
/// # Errors
///
/// Returns validation errors for unknown categories and [`ReadModelError::Sql`]
/// when `PostgreSQL` cannot be queried.
pub async fn list_relations(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
    direction: RelationDirection,
) -> Result<Vec<RelationRow>, ReadModelError> {
    entity_table(category)?;
    let mut rows = Vec::new();
    if matches!(
        direction,
        RelationDirection::Outgoing | RelationDirection::Both
    ) {
        rows.extend(list_outgoing_relations(pool, category, source_id).await?);
    }
    if matches!(
        direction,
        RelationDirection::Incoming | RelationDirection::Both
    ) {
        rows.extend(list_incoming_relations(pool, category, source_id).await?);
    }
    rows.sort_by(|left, right| {
        (
            &left.source_category,
            left.source_id,
            &left.relation_name,
            &left.target_category,
            left.target_id,
            left.ordinal,
        )
            .cmp(&(
                &right.source_category,
                right.source_id,
                &right.relation_name,
                &right.target_category,
                right.target_id,
                right.ordinal,
            ))
    });
    Ok(rows)
}

async fn get_detail(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
) -> Result<EntityDetail, ReadModelError> {
    let table = entity_table(category)?;
    let mut builder = QueryBuilder::new("SELECT * FROM ");
    builder.push(ident(&table.name));
    builder.push(" WHERE source_category = ");
    builder.push_bind(category);
    builder.push(" AND source_id = ");
    builder.push_bind(source_id);

    let row = builder
        .build()
        .fetch_optional(pool)
        .await?
        .ok_or(ReadModelError::NotFound)?;

    Ok(EntityDetail {
        metadata: entity_change_ref(&row),
        fields: scalar_fields(&row, &table)?,
    })
}

async fn list_outgoing_relations(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
) -> Result<Vec<RelationRow>, ReadModelError> {
    let mut rows = Vec::new();
    for table in relation_tables().filter(|table| {
        matches!(
            table.kind,
            TableKind::Relation {
                source_category,
                ..
            } if source_category == category
        )
    }) {
        rows.extend(
            fetch_relation_rows(
                pool,
                &table,
                "source_category",
                category,
                "source_id",
                source_id,
            )
            .await?,
        );
    }
    Ok(rows)
}

async fn list_incoming_relations(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
) -> Result<Vec<RelationRow>, ReadModelError> {
    let mut rows = Vec::new();
    for table in relation_tables() {
        rows.extend(
            fetch_relation_rows(
                pool,
                &table,
                "target_category",
                category,
                "target_id",
                source_id,
            )
            .await?,
        );
    }
    Ok(rows)
}

async fn fetch_relation_rows(
    pool: &PgPool,
    table: &TableSpec,
    category_column: &'static str,
    category: &str,
    id_column: &'static str,
    source_id: Uuid,
) -> Result<Vec<RelationRow>, ReadModelError> {
    let mut builder = QueryBuilder::new(
        "SELECT source_category, source_id, relation_name, target_category, target_id, ordinal, source_updated_at FROM ",
    );
    builder.push(ident(&table.name));
    builder.push(" WHERE ");
    builder.push(ident(category_column));
    builder.push(" = ");
    builder.push_bind(category);
    builder.push(" AND ");
    builder.push(ident(id_column));
    builder.push(" = ");
    builder.push_bind(source_id);

    Ok(builder
        .build()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| relation_row(&row))
        .collect())
}

fn entity_table(category: &str) -> Result<TableSpec, ReadModelError> {
    postgres_schema::schema()
        .tables
        .into_iter()
        .find(|table| {
            matches!(
                table.kind,
                TableKind::Entity {
                    category: table_category
                } if table_category == category
            )
        })
        .ok_or_else(|| ReadModelError::UnknownCategory(category.to_owned()))
}

fn relation_tables() -> impl Iterator<Item = TableSpec> {
    postgres_schema::schema()
        .tables
        .into_iter()
        .filter(|table| matches!(table.kind, TableKind::Relation { .. }))
}

fn scalar_fields(row: &PgRow, table: &TableSpec) -> Result<Map<String, Value>, ReadModelError> {
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

fn entity_change_ref(row: &PgRow) -> EntityChange {
    EntityChange {
        category: row.get("source_category"),
        source_id: row.get("source_id"),
        latest_skiptoken: row.get("latest_skiptoken"),
        deleted: row.get("deleted"),
        source_updated_at: timestamp_cell(row, "source_updated_at").expect("non-null timestamp"),
        atom_updated_at: timestamp_cell(row, "atom_updated_at").expect("non-null timestamp"),
    }
}

fn entity_change(row: &PgRow) -> EntityChange {
    entity_change_ref(row)
}

fn relation_row(row: &PgRow) -> RelationRow {
    RelationRow {
        source_category: row.get("source_category"),
        source_id: row.get("source_id"),
        relation_name: row.get("relation_name"),
        target_category: row.get("target_category"),
        target_id: row.get("target_id"),
        ordinal: row.get("ordinal"),
        source_updated_at: timestamp_cell(row, "source_updated_at").expect("non-null timestamp"),
    }
}

fn document_content_detail(row: &PgRow) -> DocumentContentDetail {
    DocumentContentDetail {
        document_source_category: row.get("document_source_category"),
        document_source_id: row.get("document_source_id"),
        asset: DocumentAssetDetail {
            id: row.get("asset_id"),
            asset_url: row.get("asset_url"),
            upstream_url: row.get("upstream_url"),
            upstream_content_type: row.get("upstream_content_type"),
            upstream_content_length: row.get("upstream_content_length"),
            upstream_last_modified_at: timestamp_cell(row, "upstream_last_modified_at"),
            retrieval_status: row.get("retrieval_status"),
            retrieval_error: row.get("retrieval_error"),
            retrieved_at: timestamp_cell(row, "retrieved_at"),
        },
        content: DocumentContentRow {
            id: row.get("content_id"),
            selected_source_url: row.get("selected_source_url"),
            selected_source_content_type: row.get("selected_source_content_type"),
            selected_source_content_length: row.get("selected_source_content_length"),
            official_source: row.get("official_source"),
            source_rank: row.get("source_rank"),
            extraction_status: row.get("extraction_status"),
            validation_status: row.get("validation_status"),
            extraction_tool: row.get("extraction_tool"),
            extraction_tool_version: row.get("extraction_tool_version"),
            source_hash: row.get("source_hash"),
            output_hash: row.get("output_hash"),
            extraction_error: row.get("extraction_error"),
            extracted_text: row.get("extracted_text"),
            extracted_html: row.get("extracted_html"),
            extracted_at: timestamp_cell(row, "extracted_at").expect("non-null timestamp"),
        },
    }
}

fn timestamp_cell(row: &PgRow, column: &str) -> Option<String> {
    row.get::<Option<DateTime<Utc>>, _>(column)
        .map(|value| value.to_rfc3339())
}

fn cell_value(row: &PgRow, column: &ColumnSpec) -> Result<Value, ReadModelError> {
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

fn ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
