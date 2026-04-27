use std::collections::{HashMap, HashSet};

use sqlx::Row;
use thiserror::Error;

use crate::postgres_schema::{
    self, CheckConstraintSpec, ColumnSpec, ForeignKeyAction, ForeignKeySpec, SqlType, TableSpec,
    UniqueConstraintSpec,
};

#[derive(Debug, Error)]
pub enum SchemaLifecycleError {
    #[error("failed to apply PostgreSQL schema from Rust schema definition")]
    Apply(#[source] sqlx::Error),
    #[error("failed to inspect PostgreSQL schema catalog")]
    Inspect(#[source] sqlx::Error),
    #[error("PostgreSQL schema is missing table `{table}`")]
    MissingTable { table: String },
    #[error("PostgreSQL schema has unexpected table `{table}`")]
    UnexpectedTable { table: String },
    #[error("PostgreSQL schema is missing column `{table}.{column}`")]
    MissingColumn { table: String, column: String },
    #[error("PostgreSQL schema has unexpected column `{table}.{column}`")]
    UnexpectedColumn { table: String, column: String },
    #[error(
        "PostgreSQL schema column `{table}.{column}` is incompatible: expected {expected}, found {actual}"
    )]
    IncompatibleColumn {
        table: String,
        column: String,
        expected: String,
        actual: String,
    },
    #[error("PostgreSQL schema is missing index `{index}` on table `{table}`")]
    MissingIndex { table: String, index: String },
    #[error(
        "PostgreSQL schema index `{index}` is incompatible: expected columns ({expected}), found ({actual})"
    )]
    IncompatibleIndex {
        index: String,
        expected: String,
        actual: String,
    },
    #[error("PostgreSQL schema is missing constraint `{constraint}` on table `{table}`")]
    MissingConstraint { table: String, constraint: String },
}

/// Create missing schema objects from the Rust schema definition, then verify
/// the live `PostgreSQL` catalog matches what this binary expects.
///
/// # Errors
///
/// Returns [`SchemaLifecycleError`] when `PostgreSQL` rejects the schema DDL or
/// the resulting catalog is incompatible with the Rust schema definition.
pub async fn ensure_schema(pool: &sqlx::PgPool) -> Result<(), SchemaLifecycleError> {
    let sql = postgres_schema::render_ensure_schema(&postgres_schema::schema());
    sqlx::raw_sql(&sql)
        .execute(pool)
        .await
        .map_err(SchemaLifecycleError::Apply)?;
    validate_schema(pool).await
}

/// Verify the live `PostgreSQL` catalog without mutating it.
///
/// # Errors
///
/// Returns [`SchemaLifecycleError`] when required tables, columns, or indexes
/// are missing or incompatible.
pub async fn validate_schema(pool: &sqlx::PgPool) -> Result<(), SchemaLifecycleError> {
    let schema = postgres_schema::schema();
    let expected_tables = schema
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .collect::<HashSet<_>>();
    for table in live_tables(pool).await? {
        if !expected_tables.contains(table.as_str()) {
            return Err(SchemaLifecycleError::UnexpectedTable { table });
        }
    }

    for table in &schema.tables {
        let live_columns = live_columns(pool, &table.name).await?;
        if live_columns.is_empty() {
            return Err(SchemaLifecycleError::MissingTable {
                table: table.name.clone(),
            });
        }
        for column in &table.columns {
            validate_column(&table.name, column, &live_columns)?;
        }
        validate_no_extra_columns(table, &live_columns)?;
        validate_constraints(pool, table).await?;
    }

    for index in &schema.indexes {
        let actual_columns = live_index_columns(pool, &index.name, &index.table_name).await?;
        let Some(actual_columns) = actual_columns else {
            return Err(SchemaLifecycleError::MissingIndex {
                table: index.table_name.clone(),
                index: index.name.clone(),
            });
        };
        if actual_columns != index.columns {
            return Err(SchemaLifecycleError::IncompatibleIndex {
                index: index.name.clone(),
                expected: index.columns.join(", "),
                actual: actual_columns.join(", "),
            });
        }
    }

    Ok(())
}

fn validate_no_extra_columns(
    table: &TableSpec,
    live_columns: &HashMap<String, LiveColumn>,
) -> Result<(), SchemaLifecycleError> {
    let expected_columns = table
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<HashSet<_>>();
    for column in live_columns.keys() {
        if !expected_columns.contains(column.as_str()) {
            return Err(SchemaLifecycleError::UnexpectedColumn {
                table: table.name.clone(),
                column: column.clone(),
            });
        }
    }
    Ok(())
}

async fn validate_constraints(
    pool: &sqlx::PgPool,
    table: &TableSpec,
) -> Result<(), SchemaLifecycleError> {
    let constraints = live_constraints(pool, &table.name).await?;
    if !constraints.iter().any(|constraint| {
        constraint.kind == LiveConstraintKind::PrimaryKey && constraint.columns == table.primary_key
    }) {
        return Err(SchemaLifecycleError::MissingConstraint {
            table: table.name.clone(),
            constraint: "primary key".to_owned(),
        });
    }
    for unique in &table.unique_constraints {
        validate_unique_constraint(table, unique, &constraints)?;
    }
    for foreign_key in &table.foreign_keys {
        validate_foreign_key(table, foreign_key, &constraints)?;
    }
    for check in &table.check_constraints {
        validate_check_constraint(table, check, &constraints)?;
    }
    Ok(())
}

fn validate_unique_constraint(
    table: &TableSpec,
    unique: &UniqueConstraintSpec,
    constraints: &[LiveConstraint],
) -> Result<(), SchemaLifecycleError> {
    if constraints.iter().any(|constraint| {
        constraint.kind == LiveConstraintKind::Unique && constraint.columns == unique.columns
    }) {
        return Ok(());
    }
    Err(SchemaLifecycleError::MissingConstraint {
        table: table.name.clone(),
        constraint: format!("unique ({})", unique.columns.join(", ")),
    })
}

fn validate_foreign_key(
    table: &TableSpec,
    foreign_key: &ForeignKeySpec,
    constraints: &[LiveConstraint],
) -> Result<(), SchemaLifecycleError> {
    let expected_delete = match foreign_key.on_delete {
        ForeignKeyAction::Cascade => "c",
        ForeignKeyAction::Restrict => "r",
    };
    if constraints.iter().any(|constraint| {
        constraint.kind == LiveConstraintKind::ForeignKey
            && constraint.columns == foreign_key.columns
            && constraint.referenced_table.as_deref() == Some(foreign_key.referenced_table.as_str())
            && constraint.referenced_columns == foreign_key.referenced_columns
            && constraint.on_delete.as_deref() == Some(expected_delete)
    }) {
        return Ok(());
    }
    Err(SchemaLifecycleError::MissingConstraint {
        table: table.name.clone(),
        constraint: format!(
            "foreign key ({}) references {} ({})",
            foreign_key.columns.join(", "),
            foreign_key.referenced_table,
            foreign_key.referenced_columns.join(", ")
        ),
    })
}

fn validate_check_constraint(
    table: &TableSpec,
    check: &CheckConstraintSpec,
    constraints: &[LiveConstraint],
) -> Result<(), SchemaLifecycleError> {
    if constraints.iter().any(|constraint| {
        constraint.kind == LiveConstraintKind::Check && constraint.name == check.name
    }) {
        return Ok(());
    }
    Err(SchemaLifecycleError::MissingConstraint {
        table: table.name.clone(),
        constraint: check.name.clone(),
    })
}

fn validate_column(
    table: &str,
    column: &ColumnSpec,
    live_columns: &HashMap<String, LiveColumn>,
) -> Result<(), SchemaLifecycleError> {
    let Some(live_column) = live_columns.get(&column.name) else {
        return Err(SchemaLifecycleError::MissingColumn {
            table: table.to_owned(),
            column: column.name.clone(),
        });
    };
    let expected_type = expected_catalog_type(column.sql_type);
    if live_column.data_type != expected_type {
        return Err(SchemaLifecycleError::IncompatibleColumn {
            table: table.to_owned(),
            column: column.name.clone(),
            expected: expected_type.to_owned(),
            actual: live_column.data_type.clone(),
        });
    }
    let expected_nullable = if column.nullable { "YES" } else { "NO" };
    if live_column.is_nullable != expected_nullable {
        return Err(SchemaLifecycleError::IncompatibleColumn {
            table: table.to_owned(),
            column: column.name.clone(),
            expected: format!("{expected_type} nullable={expected_nullable}"),
            actual: format!(
                "{} nullable={}",
                live_column.data_type, live_column.is_nullable
            ),
        });
    }
    if matches!(column.sql_type, SqlType::BigIdentity) && live_column.is_identity != "YES" {
        return Err(SchemaLifecycleError::IncompatibleColumn {
            table: table.to_owned(),
            column: column.name.clone(),
            expected: "bigint identity".to_owned(),
            actual: format!(
                "{} identity={}",
                live_column.data_type, live_column.is_identity
            ),
        });
    }
    Ok(())
}

async fn live_columns(
    pool: &sqlx::PgPool,
    table: &str,
) -> Result<HashMap<String, LiveColumn>, SchemaLifecycleError> {
    let rows = sqlx::query(
        "SELECT column_name, data_type, is_nullable, is_identity
         FROM information_schema.columns
         WHERE table_schema = current_schema()
           AND table_name = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(SchemaLifecycleError::Inspect)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("column_name"),
                LiveColumn {
                    data_type: row.get("data_type"),
                    is_nullable: row.get("is_nullable"),
                    is_identity: row.get("is_identity"),
                },
            )
        })
        .collect())
}

async fn live_tables(pool: &sqlx::PgPool) -> Result<Vec<String>, SchemaLifecycleError> {
    sqlx::query_scalar(
        "SELECT table_name
         FROM information_schema.tables
         WHERE table_schema = current_schema()
           AND table_type = 'BASE TABLE'",
    )
    .fetch_all(pool)
    .await
    .map_err(SchemaLifecycleError::Inspect)
}

async fn live_constraints(
    pool: &sqlx::PgPool,
    table: &str,
) -> Result<Vec<LiveConstraint>, SchemaLifecycleError> {
    let rows = sqlx::query(
        "SELECT
             constraint_info.conname AS name,
             constraint_info.contype AS kind,
             column_info.columns AS columns,
             referenced_table.relname AS referenced_table,
             referenced_column_info.columns AS referenced_columns,
             constraint_info.confdeltype::text AS on_delete
         FROM pg_constraint constraint_info
         JOIN pg_class table_info ON table_info.oid = constraint_info.conrelid
         JOIN pg_namespace namespace_info ON namespace_info.oid = table_info.relnamespace
         LEFT JOIN pg_class referenced_table ON referenced_table.oid = constraint_info.confrelid
         LEFT JOIN LATERAL (
             SELECT array_agg(attribute.attname ORDER BY key.ordinality) AS columns
             FROM unnest(constraint_info.conkey) WITH ORDINALITY AS key(attnum, ordinality)
             JOIN pg_attribute attribute
               ON attribute.attrelid = constraint_info.conrelid
              AND attribute.attnum = key.attnum
         ) column_info ON true
         LEFT JOIN LATERAL (
             SELECT array_agg(attribute.attname ORDER BY key.ordinality) AS columns
             FROM unnest(constraint_info.confkey) WITH ORDINALITY AS key(attnum, ordinality)
             JOIN pg_attribute attribute
               ON attribute.attrelid = constraint_info.confrelid
              AND attribute.attnum = key.attnum
         ) referenced_column_info ON true
         WHERE namespace_info.nspname = current_schema()
           AND table_info.relname = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(SchemaLifecycleError::Inspect)?;

    rows.into_iter()
        .map(|row| {
            let kind = match row.get::<i8, _>("kind").cast_unsigned() as char {
                'p' => LiveConstraintKind::PrimaryKey,
                'u' => LiveConstraintKind::Unique,
                'f' => LiveConstraintKind::ForeignKey,
                'c' => LiveConstraintKind::Check,
                _ => LiveConstraintKind::Other,
            };
            Ok(LiveConstraint {
                name: row.get("name"),
                kind,
                columns: row.try_get("columns").unwrap_or_default(),
                referenced_table: row.try_get("referenced_table").ok(),
                referenced_columns: row.try_get("referenced_columns").unwrap_or_default(),
                on_delete: row.try_get("on_delete").ok(),
            })
        })
        .collect()
}

async fn live_index_columns(
    pool: &sqlx::PgPool,
    index_name: &str,
    table_name: &str,
) -> Result<Option<Vec<String>>, SchemaLifecycleError> {
    let row = sqlx::query(
        "SELECT array_agg(attribute.attname ORDER BY key.ordinality) AS columns
         FROM pg_class table_class
         JOIN pg_namespace namespace ON namespace.oid = table_class.relnamespace
         JOIN pg_index index_info ON index_info.indrelid = table_class.oid
         JOIN pg_class index_class ON index_class.oid = index_info.indexrelid
         JOIN unnest(index_info.indkey) WITH ORDINALITY AS key(attnum, ordinality) ON true
         JOIN pg_attribute attribute
           ON attribute.attrelid = table_class.oid
          AND attribute.attnum = key.attnum
         WHERE namespace.nspname = current_schema()
           AND table_class.relname = $1
           AND index_class.relname = $2
         GROUP BY index_class.relname",
    )
    .bind(table_name)
    .bind(index_name)
    .fetch_optional(pool)
    .await
    .map_err(SchemaLifecycleError::Inspect)?;

    Ok(row.map(|row| row.get::<Vec<String>, _>("columns")))
}

fn expected_catalog_type(sql_type: SqlType) -> &'static str {
    match sql_type {
        SqlType::Uuid => "uuid",
        SqlType::Text => "text",
        SqlType::Boolean => "boolean",
        SqlType::Integer => "integer",
        SqlType::BigInteger | SqlType::BigIdentity => "bigint",
        SqlType::TimestampTz => "timestamp with time zone",
        SqlType::Date => "date",
        SqlType::Jsonb => "jsonb",
    }
}

#[derive(Debug)]
struct LiveColumn {
    data_type: String,
    is_nullable: String,
    is_identity: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveConstraintKind {
    PrimaryKey,
    Unique,
    ForeignKey,
    Check,
    Other,
}

#[derive(Debug)]
struct LiveConstraint {
    name: String,
    kind: LiveConstraintKind,
    columns: Vec<String>,
    referenced_table: Option<String>,
    referenced_columns: Vec<String>,
    on_delete: Option<String>,
}
