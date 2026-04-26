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
