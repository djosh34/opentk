use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, NaiveDate, Utc};
use opentk_core::official_schema;
use opentk_sync::payload::{ParsedEntity, ParsedScalar, ParsedValue};
use sqlx::{postgres::PgArguments, PgPool, Postgres, QueryBuilder};
use thiserror::Error;
use uuid::Uuid;

use crate::postgres_schema::{self, ColumnSpec, SqlType, TableKind, TableSpec};

#[derive(Clone, Debug)]
pub struct SyncPageWrite {
    pub category: String,
    pub latest_skiptoken: i64,
    pub next_url: Option<String>,
    pub atom_updated_at: DateTime<Utc>,
    pub entities: Vec<ParsedEntity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncPageWriteOutcome {
    pub entities_seen: usize,
    pub entities_written: usize,
    pub entities_deleted: usize,
    pub relations_written: usize,
    pub repeated_scalars_written: usize,
}

#[derive(Debug, Error)]
pub enum SyncPageWriteError {
    #[error("unknown category {category}")]
    UnknownCategory { category: String },
    #[error("entity category mismatch: page {page_category}, entity {entity_category}")]
    CategoryMismatch {
        page_category: String,
        entity_category: String,
    },
    #[error("missing schema table {table}")]
    MissingTable { table: String },
    #[error("{category}.{field} value {value:?} cannot be written to {sql_type:?}")]
    TypeMismatch {
        category: String,
        field: String,
        value: ParsedValue,
        sql_type: SqlType,
    },
    #[error("missing non-null database value for {table}.{column}")]
    MissingNonNullValue { table: String, column: String },
    #[error("database write failed: {0}")]
    Sql(#[from] sqlx::Error),
}

/// Write one parsed `SyncFeed` page into `PostgreSQL` in a single transaction.
///
/// # Errors
///
/// Returns an error when the page category is not modeled, an entity category
/// does not match the page, the generated schema is missing an expected table,
/// a parsed value cannot be represented by the destination SQL type, a
/// non-null column has no value, or `PostgreSQL` rejects any statement.
pub async fn write_sync_page(
    pool: &PgPool,
    page: SyncPageWrite,
) -> Result<SyncPageWriteOutcome, SyncPageWriteError> {
    let entity_type = official_schema::entity_named(&page.category).ok_or_else(|| {
        SyncPageWriteError::UnknownCategory {
            category: page.category.clone(),
        }
    })?;
    let schema = postgres_schema::schema();
    let entity_table_name = postgres_schema::sql_name(entity_type.category);
    let entity_table = table(&schema.tables, &entity_table_name)?;
    let sync_entity_keys = collect_sync_entity_keys(&page, entity_type.category)?;
    let mut tx = pool.begin().await?;
    acquire_sync_entity_locks(&mut tx, &sync_entity_keys).await?;
    let mut outcome = SyncPageWriteOutcome {
        entities_seen: page.entities.len(),
        entities_written: 0,
        entities_deleted: 0,
        relations_written: 0,
        repeated_scalars_written: 0,
    };

    for entity in &page.entities {
        if entity.category != entity_type.category {
            return Err(SyncPageWriteError::CategoryMismatch {
                page_category: entity_type.category.to_owned(),
                entity_category: entity.category.clone(),
            });
        }

        upsert_sync_entity(&mut tx, entity, page.latest_skiptoken, page.atom_updated_at).await?;
        delete_current_rows(
            &mut tx,
            &entity_table.name,
            entity_type.category,
            entity.source_id,
        )
        .await?;
        if entity.deleted {
            outcome.entities_deleted += 1;
            continue;
        }

        for relation in &entity.relations {
            upsert_relation_target(
                &mut tx,
                relation,
                entity,
                page.latest_skiptoken,
                page.atom_updated_at,
            )
            .await?;
        }
        insert_entity_row(
            &mut tx,
            entity_table,
            entity,
            page.latest_skiptoken,
            page.atom_updated_at,
        )
        .await?;
        outcome.entities_written += 1;

        for table in relation_tables(&schema.tables, entity_type.category) {
            outcome.relations_written +=
                insert_relation_rows(&mut tx, &table.name, entity, page.latest_skiptoken).await?;
        }
        for table in repeated_scalar_tables(&schema.tables, entity_type.category) {
            outcome.repeated_scalars_written +=
                insert_repeated_scalar_rows(&mut tx, table, entity, page.latest_skiptoken).await?;
        }
    }

    sqlx::query(
        r"
        INSERT INTO sync_category (
            source_category,
            latest_skiptoken,
            last_synced_at,
            next_url,
            state,
            last_fetch_at
        )
        VALUES ($1, $2, $3, $4, 'running', $3)
        ON CONFLICT (source_category) DO UPDATE
        SET latest_skiptoken = EXCLUDED.latest_skiptoken,
            last_synced_at = EXCLUDED.last_synced_at,
            next_url = EXCLUDED.next_url,
            state = EXCLUDED.state,
            last_fetch_at = EXCLUDED.last_fetch_at
        ",
    )
    .bind(entity_type.category)
    .bind(page.latest_skiptoken)
    .bind(page.atom_updated_at)
    .bind(&page.next_url)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(outcome)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SyncEntityKey {
    source_category: String,
    source_id: Uuid,
}

fn collect_sync_entity_keys(
    page: &SyncPageWrite,
    page_category: &str,
) -> Result<Vec<SyncEntityKey>, SyncPageWriteError> {
    let mut keys = BTreeSet::new();
    for entity in &page.entities {
        if entity.category != page_category {
            return Err(SyncPageWriteError::CategoryMismatch {
                page_category: page_category.to_owned(),
                entity_category: entity.category.clone(),
            });
        }
        keys.insert(SyncEntityKey {
            source_category: entity.category.clone(),
            source_id: entity.source_id,
        });
        for relation in &entity.relations {
            keys.insert(SyncEntityKey {
                source_category: relation.target_category.clone(),
                source_id: relation.target_id,
            });
        }
    }
    Ok(keys.into_iter().collect())
}

async fn acquire_sync_entity_locks(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    keys: &[SyncEntityKey],
) -> Result<(), SyncPageWriteError> {
    for key in keys {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{}:{}", key.source_category, key.source_id))
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

async fn upsert_sync_entity(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    entity: &ParsedEntity,
    latest_skiptoken: i64,
    atom_updated_at: DateTime<Utc>,
) -> Result<(), SyncPageWriteError> {
    sqlx::query(
        r"
        INSERT INTO sync_entity (
            source_category,
            source_id,
            latest_skiptoken,
            deleted,
            source_updated_at,
            atom_updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (source_category, source_id) DO UPDATE
        SET latest_skiptoken = EXCLUDED.latest_skiptoken,
            deleted = EXCLUDED.deleted,
            source_updated_at = EXCLUDED.source_updated_at,
            atom_updated_at = EXCLUDED.atom_updated_at
        ",
    )
    .bind(&entity.category)
    .bind(entity.source_id)
    .bind(latest_skiptoken)
    .bind(entity.deleted)
    .bind(entity.source_updated_at)
    .bind(atom_updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_relation_target(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    relation: &opentk_sync::payload::ParsedRelation,
    source: &ParsedEntity,
    latest_skiptoken: i64,
    atom_updated_at: DateTime<Utc>,
) -> Result<(), SyncPageWriteError> {
    let source_updated_at = relation
        .target_updated_at
        .unwrap_or(source.source_updated_at);
    sqlx::query(
        r"
        INSERT INTO sync_entity (
            source_category,
            source_id,
            latest_skiptoken,
            deleted,
            source_updated_at,
            atom_updated_at
        )
        VALUES ($1, $2, $3, false, $4, $5)
        ON CONFLICT (source_category, source_id) DO UPDATE
        SET latest_skiptoken = GREATEST(sync_entity.latest_skiptoken, EXCLUDED.latest_skiptoken),
            source_updated_at = GREATEST(sync_entity.source_updated_at, EXCLUDED.source_updated_at),
            atom_updated_at = GREATEST(sync_entity.atom_updated_at, EXCLUDED.atom_updated_at)
        ",
    )
    .bind(&relation.target_category)
    .bind(relation.target_id)
    .bind(latest_skiptoken)
    .bind(source_updated_at)
    .bind(atom_updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn delete_current_rows(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    table_name: &str,
    category: &str,
    source_id: Uuid,
) -> Result<(), SyncPageWriteError> {
    let sql = format!(
        "DELETE FROM {} WHERE source_category = $1 AND source_id = $2",
        ident(table_name)
    );
    sqlx::query(&sql)
        .bind(category)
        .bind(source_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn insert_entity_row(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    table: &TableSpec,
    entity: &ParsedEntity,
    latest_skiptoken: i64,
    atom_updated_at: DateTime<Utc>,
) -> Result<(), SyncPageWriteError> {
    let values = entity_column_values(table, entity, latest_skiptoken, atom_updated_at)?;
    let columns: Vec<_> = values.iter().map(|(column, _)| column.as_str()).collect();
    let placeholders: Vec<_> = (1..=values.len())
        .map(|index| format!("${index}"))
        .collect();
    let assignments: Vec<_> = columns
        .iter()
        .filter(|column| !["source_category", "source_id"].contains(column))
        .map(|column| format!("{} = EXCLUDED.{}", ident(column), ident(column)))
        .collect();
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT (source_category, source_id) DO UPDATE SET {}",
        ident(&table.name),
        ident_str_list(&columns),
        placeholders.join(", "),
        assignments.join(", ")
    );
    let mut query = sqlx::query_with(&sql, PgArguments::default());
    for (_, value) in values {
        query = bind_value(query, value);
    }
    query.execute(&mut **tx).await?;
    Ok(())
}

async fn insert_relation_rows(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    table_name: &str,
    entity: &ParsedEntity,
    _latest_skiptoken: i64,
) -> Result<usize, SyncPageWriteError> {
    let relation_name = table_name
        .split_once("__")
        .map_or(table_name, |(_, relation)| relation);
    let rows: Vec<_> = entity
        .relations
        .iter()
        .filter(|relation| postgres_schema::sql_name(&relation.name) == relation_name)
        .collect();
    if rows.is_empty() {
        return Ok(0);
    }

    let mut builder = QueryBuilder::new(format!(
        "INSERT INTO {} (source_category, source_id, relation_name, target_category, target_id, ordinal, source_updated_at) ",
        ident(table_name)
    ));
    builder.push_values(rows.iter(), |mut row, relation| {
        row.push_bind(&entity.category)
            .push_bind(entity.source_id)
            .push_bind(&relation.name)
            .push_bind(&relation.target_category)
            .push_bind(relation.target_id)
            .push_bind(relation.ordinal)
            .push_bind(entity.source_updated_at);
    });
    builder.build().execute(&mut **tx).await?;
    Ok(rows.len())
}

async fn insert_repeated_scalar_rows(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    table: &TableSpec,
    entity: &ParsedEntity,
    _latest_skiptoken: i64,
) -> Result<usize, SyncPageWriteError> {
    let TableKind::RepeatedScalar { field_name, .. } = table.kind else {
        return Ok(0);
    };
    let value_column = table
        .columns
        .iter()
        .find(|column| column.name == "value")
        .ok_or_else(|| SyncPageWriteError::MissingTable {
            table: table.name.clone(),
        })?;
    let rows: Vec<_> = entity
        .scalars
        .iter()
        .filter(|scalar| scalar.name == field_name)
        .collect();
    if rows.is_empty() {
        return Ok(0);
    }

    let sql = format!(
        "INSERT INTO {} (source_category, source_id, ordinal, value) VALUES ($1, $2, $3, $4)",
        ident(&table.name)
    );
    for scalar in &rows {
        let value = bind_from_parsed_value(
            &entity.category,
            &scalar.name,
            &scalar.value,
            value_column.sql_type,
        )?;
        let mut query = sqlx::query_with(&sql, PgArguments::default())
            .bind(&entity.category)
            .bind(entity.source_id)
            .bind(scalar.ordinal);
        query = bind_value(query, value);
        query.execute(&mut **tx).await?;
    }
    Ok(rows.len())
}

fn entity_column_values(
    table: &TableSpec,
    entity: &ParsedEntity,
    latest_skiptoken: i64,
    atom_updated_at: DateTime<Utc>,
) -> Result<Vec<(String, BindValue)>, SyncPageWriteError> {
    let scalars: HashMap<_, _> = entity
        .scalars
        .iter()
        .map(|scalar| (postgres_schema::sql_name(&scalar.name), scalar))
        .collect();
    let mut values = Vec::new();

    for column in &table.columns {
        let value = match column.name.as_str() {
            "source_category" => BindValue::Text(Some(entity.category.clone())),
            "source_id" => BindValue::Uuid(Some(entity.source_id)),
            "latest_skiptoken" => BindValue::I64(Some(latest_skiptoken)),
            "deleted" => BindValue::Bool(Some(false)),
            "source_updated_at" => BindValue::DateTime(Some(entity.source_updated_at)),
            "atom_updated_at" => BindValue::DateTime(Some(atom_updated_at)),
            "content_type" => BindValue::Text(entity.content_type.clone()),
            "content_length" => {
                let value = entity
                    .content_length
                    .map(|value| {
                        i32::try_from(value).map_err(|_| SyncPageWriteError::TypeMismatch {
                            category: entity.category.clone(),
                            field: "contentLength".to_owned(),
                            value: ParsedValue::I64(value),
                            sql_type: SqlType::Integer,
                        })
                    })
                    .transpose()?;
                BindValue::I32(value)
            }
            "enclosure_url" => {
                BindValue::Text(entity.enclosure_url.as_ref().map(ToString::to_string))
            }
            name => match scalars.get(name) {
                Some(scalar) => bind_scalar(entity, scalar, column)?,
                None if column.nullable => null_for(column.sql_type),
                None => {
                    return Err(SyncPageWriteError::MissingNonNullValue {
                        table: table.name.clone(),
                        column: column.name.clone(),
                    });
                }
            },
        };
        values.push((column.name.clone(), value));
    }

    Ok(values)
}

fn bind_scalar(
    entity: &ParsedEntity,
    scalar: &ParsedScalar,
    column: &ColumnSpec,
) -> Result<BindValue, SyncPageWriteError> {
    bind_from_parsed_value(
        &entity.category,
        &scalar.name,
        &scalar.value,
        column.sql_type,
    )
}

fn bind_from_parsed_value(
    category: &str,
    field: &str,
    value: &ParsedValue,
    sql_type: SqlType,
) -> Result<BindValue, SyncPageWriteError> {
    match (sql_type, value) {
        (SqlType::Uuid, ParsedValue::Text(value)) => {
            let uuid = Uuid::parse_str(value).map_err(|_| SyncPageWriteError::TypeMismatch {
                category: category.to_owned(),
                field: field.to_owned(),
                value: ParsedValue::Text(value.clone()),
                sql_type,
            })?;
            Ok(BindValue::Uuid(Some(uuid)))
        }
        (SqlType::Text | SqlType::Jsonb, ParsedValue::Text(value)) => {
            Ok(BindValue::Text(Some(value.clone())))
        }
        (SqlType::Boolean, ParsedValue::Bool(value)) => Ok(BindValue::Bool(Some(*value))),
        (SqlType::Integer, ParsedValue::I32(value)) => Ok(BindValue::I32(Some(*value))),
        (SqlType::Integer, ParsedValue::I64(value)) => i32::try_from(*value)
            .map(|value| BindValue::I32(Some(value)))
            .map_err(|_| SyncPageWriteError::TypeMismatch {
                category: category.to_owned(),
                field: field.to_owned(),
                value: ParsedValue::I64(*value),
                sql_type,
            }),
        (SqlType::BigInteger, ParsedValue::I64(value)) => Ok(BindValue::I64(Some(*value))),
        (SqlType::BigInteger, ParsedValue::I32(value)) => {
            Ok(BindValue::I64(Some(i64::from(*value))))
        }
        (SqlType::TimestampTz, ParsedValue::DateTime(value)) => {
            Ok(BindValue::DateTime(Some(*value)))
        }
        (SqlType::Date, ParsedValue::Date(value)) => Ok(BindValue::Date(Some(*value))),
        _ => Err(SyncPageWriteError::TypeMismatch {
            category: category.to_owned(),
            field: field.to_owned(),
            value: value.clone(),
            sql_type,
        }),
    }
}

fn null_for(sql_type: SqlType) -> BindValue {
    match sql_type {
        SqlType::Uuid => BindValue::Uuid(None),
        SqlType::Text | SqlType::Jsonb => BindValue::Text(None),
        SqlType::Boolean => BindValue::Bool(None),
        SqlType::Integer => BindValue::I32(None),
        SqlType::BigInteger | SqlType::BigIdentity => BindValue::I64(None),
        SqlType::TimestampTz => BindValue::DateTime(None),
        SqlType::Date => BindValue::Date(None),
    }
}

#[derive(Clone, Debug)]
enum BindValue {
    Uuid(Option<Uuid>),
    Text(Option<String>),
    Bool(Option<bool>),
    I32(Option<i32>),
    I64(Option<i64>),
    DateTime(Option<DateTime<Utc>>),
    Date(Option<NaiveDate>),
}

fn bind_value(
    query: sqlx::query::Query<'_, Postgres, PgArguments>,
    value: BindValue,
) -> sqlx::query::Query<'_, Postgres, PgArguments> {
    match value {
        BindValue::Uuid(value) => query.bind(value),
        BindValue::Text(value) => query.bind(value),
        BindValue::Bool(value) => query.bind(value),
        BindValue::I32(value) => query.bind(value),
        BindValue::I64(value) => query.bind(value),
        BindValue::DateTime(value) => query.bind(value),
        BindValue::Date(value) => query.bind(value),
    }
}

fn relation_tables<'a>(tables: &'a [TableSpec], category: &str) -> Vec<&'a TableSpec> {
    tables
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

fn repeated_scalar_tables<'a>(tables: &'a [TableSpec], category: &str) -> Vec<&'a TableSpec> {
    tables
        .iter()
        .filter(|table| {
            matches!(
                table.kind,
                TableKind::RepeatedScalar {
                    category: table_category,
                    ..
                } if table_category == category
            )
        })
        .collect()
}

fn table<'a>(tables: &'a [TableSpec], name: &str) -> Result<&'a TableSpec, SyncPageWriteError> {
    tables
        .iter()
        .find(|table| table.name == name)
        .ok_or_else(|| SyncPageWriteError::MissingTable {
            table: name.to_owned(),
        })
}

fn ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn ident_str_list(identifiers: &[&str]) -> String {
    identifiers
        .iter()
        .map(|identifier| ident(identifier))
        .collect::<Vec<_>>()
        .join(", ")
}
