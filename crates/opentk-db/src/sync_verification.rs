use opentk_core::official_schema;
use opentk_sync::runner::CategorySyncState;
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::postgres_schema::{self, IndexPurpose, TableKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncVerificationConfig {
    pub categories: Vec<String>,
    pub required_relation_samples: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncVerificationReport {
    pub categories: Vec<CategoryVerification>,
    pub relation_tables: Vec<RelationVerification>,
    pub direct_queries: Vec<DirectQueryVerification>,
    pub table_snapshots: Vec<TableSnapshot>,
    pub storage: StorageVerification,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryVerification {
    pub category: String,
    pub table_name: String,
    pub current_rows: i64,
    pub registry_rows: i64,
    pub latest_skiptoken: Option<i64>,
    pub state: CategorySyncState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationVerification {
    pub source_category: String,
    pub relation_name: String,
    pub table_name: String,
    pub rows: i64,
    pub queryable_from_source: bool,
    pub queryable_from_target: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectQueryVerification {
    pub name: String,
    pub rows: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableSnapshot {
    pub table_name: String,
    pub rows: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StorageVerification {
    pub table_bytes: i64,
    pub index_bytes: i64,
    pub link_metadata_bytes: i64,
    pub extracted_text_bytes: i64,
    pub stored_html_bytes: i64,
    pub constraint_count: i64,
    pub binary_asset_metadata_rows: i64,
    pub document_content_rows: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentContentVerification {
    pub document_source_id: Uuid,
    pub selected_source_url: String,
    pub upstream_content_type: Option<String>,
    pub selected_source_content_type: Option<String>,
    pub official_source: bool,
    pub source_rank: i32,
    pub extraction_status: String,
    pub validation_status: String,
    pub extraction_tool: String,
    pub extraction_tool_version: String,
    pub source_hash: String,
    pub output_hash: Option<String>,
    pub extracted_text_bytes: i64,
    pub extracted_html_bytes: i64,
}

#[derive(Debug, Error)]
pub enum SyncVerificationError {
    #[error("unknown category {category}")]
    UnknownCategory { category: String },
    #[error("missing generated table {table}")]
    MissingTable { table: String },
    #[error("unknown sync category state {state:?} for {category}")]
    UnknownState { category: String, state: String },
    #[error("expected at least {required} relation sample rows, found {found}")]
    MissingRelationSamples { required: usize, found: i64 },
    #[error("document content not found for {document_source_id}")]
    DocumentContentNotFound { document_source_id: Uuid },
    #[error("database verification failed: {0}")]
    Sql(#[from] sqlx::Error),
}

/// Verify that synced `PostgreSQL` tables are directly usable as the read model.
///
/// # Errors
///
/// Returns an error when the requested categories are not official model
/// categories, generated schema coverage is missing, sync state contains an
/// unknown durable state value, or `PostgreSQL` rejects a verification query.
pub async fn verify_sync_database(
    pool: &PgPool,
    config: SyncVerificationConfig,
) -> Result<SyncVerificationReport, SyncVerificationError> {
    let schema = postgres_schema::schema();
    let categories = selected_categories(&config)?;
    let mut category_reports = Vec::with_capacity(categories.len());
    for category in &categories {
        let table_name = postgres_schema::sql_name(category);
        if schema.table_named(&table_name).is_none() {
            return Err(SyncVerificationError::MissingTable { table: table_name });
        }
        category_reports.push(verify_category(pool, category, &table_name).await?);
    }

    let relation_tables = verify_relations(pool, &schema, &categories).await?;
    let relation_sample_rows: i64 = relation_tables.iter().map(|relation| relation.rows).sum();
    if relation_sample_rows < i64::try_from(config.required_relation_samples).unwrap_or(i64::MAX) {
        return Err(SyncVerificationError::MissingRelationSamples {
            required: config.required_relation_samples,
            found: relation_sample_rows,
        });
    }
    let direct_queries = verify_direct_queries(pool, &categories).await?;
    let table_snapshots = snapshot_tables(pool, &schema).await?;
    let storage = verify_storage(pool, &schema).await?;

    Ok(SyncVerificationReport {
        categories: category_reports,
        relation_tables,
        direct_queries,
        table_snapshots,
        storage,
    })
}

/// Verify row-backed document-content provenance for one stored document.
///
/// # Errors
///
/// Returns [`SyncVerificationError::DocumentContentNotFound`] when the
/// document has no stored content row, or [`SyncVerificationError::Sql`] when
/// `PostgreSQL` rejects the read.
pub async fn verify_document_content(
    pool: &PgPool,
    document_source_id: Uuid,
) -> Result<DocumentContentVerification, SyncVerificationError> {
    let row = sqlx::query(
        r"
        SELECT c.document_source_id,
               c.selected_source_url,
               a.upstream_content_type,
               c.selected_source_content_type,
               c.official_source,
               c.source_rank,
               c.extraction_status,
               c.validation_status,
               c.extraction_tool,
               c.extraction_tool_version,
               c.source_hash,
               c.output_hash,
               COALESCE(octet_length(c.extracted_text), 0)::bigint AS extracted_text_bytes,
               COALESCE(octet_length(c.extracted_html), 0)::bigint AS extracted_html_bytes
        FROM document_content c
        JOIN document_asset a ON a.id = c.document_asset_id
        WHERE c.document_source_category = 'Document'
          AND c.document_source_id = $1
        ORDER BY c.official_source DESC,
                 c.source_rank ASC,
                 c.extracted_at DESC,
                 c.id DESC
        LIMIT 1
        ",
    )
    .bind(document_source_id)
    .fetch_optional(pool)
    .await?
    .ok_or(SyncVerificationError::DocumentContentNotFound { document_source_id })?;

    Ok(DocumentContentVerification {
        document_source_id: row.get("document_source_id"),
        selected_source_url: row.get("selected_source_url"),
        upstream_content_type: row.get("upstream_content_type"),
        selected_source_content_type: row.get("selected_source_content_type"),
        official_source: row.get("official_source"),
        source_rank: row.get("source_rank"),
        extraction_status: row.get("extraction_status"),
        validation_status: row.get("validation_status"),
        extraction_tool: row.get("extraction_tool"),
        extraction_tool_version: row.get("extraction_tool_version"),
        source_hash: row.get("source_hash"),
        output_hash: row.get("output_hash"),
        extracted_text_bytes: row.get("extracted_text_bytes"),
        extracted_html_bytes: row.get("extracted_html_bytes"),
    })
}

fn selected_categories(
    config: &SyncVerificationConfig,
) -> Result<Vec<String>, SyncVerificationError> {
    if config.categories.is_empty() {
        return Ok(official_schema::entity_types()
            .iter()
            .map(|entity| entity.category.to_owned())
            .collect());
    }

    for category in &config.categories {
        if official_schema::entity_named(category).is_none() {
            return Err(SyncVerificationError::UnknownCategory {
                category: category.clone(),
            });
        }
    }
    Ok(config.categories.clone())
}

async fn verify_category(
    pool: &PgPool,
    category: &str,
    table_name: &str,
) -> Result<CategoryVerification, SyncVerificationError> {
    let current_rows = count_category_rows(pool, table_name, category).await?;
    let registry_rows: i64 =
        sqlx::query_scalar("SELECT count(*)::bigint FROM sync_entity WHERE source_category = $1")
            .bind(category)
            .fetch_one(pool)
            .await?;
    let sync_row =
        sqlx::query("SELECT latest_skiptoken, state FROM sync_category WHERE source_category = $1")
            .bind(category)
            .fetch_optional(pool)
            .await?;
    let (latest_skiptoken, state) = match sync_row {
        Some(row) => {
            let state_text: String = row.try_get("state")?;
            let state = parse_state(category, &state_text)?;
            (Some(row.try_get("latest_skiptoken")?), state)
        }
        None => (None, CategorySyncState::NotStarted),
    };

    Ok(CategoryVerification {
        category: category.to_owned(),
        table_name: table_name.to_owned(),
        current_rows,
        registry_rows,
        latest_skiptoken,
        state,
    })
}

async fn verify_direct_queries(
    pool: &PgPool,
    categories: &[String],
) -> Result<Vec<DirectQueryVerification>, SyncVerificationError> {
    let mut queries = Vec::new();
    if categories.iter().any(|category| category == "Document") {
        queries.push(DirectQueryVerification {
            name: "document_by_document_nummer".to_owned(),
            rows: sqlx::query_scalar(
                r"
                SELECT count(*)::bigint
                FROM document
                WHERE document_nummer IS NOT NULL
                  AND (content_type IS NOT NULL OR enclosure_url IS NOT NULL)
                ",
            )
            .fetch_one(pool)
            .await?,
        });
        queries.push(DirectQueryVerification {
            name: "document_kamerstukdossier_join".to_owned(),
            rows: sqlx::query_scalar(
                r"
                SELECT count(*)::bigint
                FROM document
                JOIN document__kamerstukdossier
                  ON document__kamerstukdossier.source_category = document.source_category
                 AND document__kamerstukdossier.source_id = document.source_id
                JOIN sync_entity AS target
                  ON target.source_category = document__kamerstukdossier.target_category
                 AND target.source_id = document__kamerstukdossier.target_id
                WHERE document.document_nummer IS NOT NULL
                ",
            )
            .fetch_one(pool)
            .await?,
        });
    }
    queries.push(DirectQueryVerification {
        name: "recent_registry_changes".to_owned(),
        rows: sqlx::query_scalar(
            "SELECT count(*)::bigint FROM sync_entity WHERE latest_skiptoken IS NOT NULL",
        )
        .fetch_one(pool)
        .await?,
    });
    queries.push(DirectQueryVerification {
        name: "category_sync_status".to_owned(),
        rows: sqlx::query_scalar("SELECT count(*)::bigint FROM sync_category")
            .fetch_one(pool)
            .await?,
    });
    Ok(queries)
}

async fn verify_relations(
    pool: &PgPool,
    schema: &postgres_schema::SchemaSpec,
    categories: &[String],
) -> Result<Vec<RelationVerification>, SyncVerificationError> {
    let mut reports = Vec::new();
    for table in &schema.tables {
        let TableKind::Relation {
            source_category,
            relation_name,
        } = table.kind
        else {
            continue;
        };
        if !categories
            .iter()
            .any(|category| category == source_category)
        {
            continue;
        }
        let rows = count_rows(pool, &table.name).await?;
        // The probes deliberately use the same predicates HTTP reads will use.
        probe_relation_by_source(pool, &table.name).await?;
        probe_relation_by_target(pool, &table.name).await?;
        reports.push(RelationVerification {
            source_category: source_category.to_owned(),
            relation_name: relation_name.to_owned(),
            table_name: table.name.clone(),
            rows,
            queryable_from_source: has_index(schema, &table.name, IndexPurpose::RelationSource),
            queryable_from_target: has_index(schema, &table.name, IndexPurpose::RelationTarget),
        });
    }
    reports.sort_by(|left, right| left.table_name.cmp(&right.table_name));
    Ok(reports)
}

async fn snapshot_tables(
    pool: &PgPool,
    schema: &postgres_schema::SchemaSpec,
) -> Result<Vec<TableSnapshot>, SyncVerificationError> {
    let mut snapshots = Vec::with_capacity(schema.tables.len());
    for table in &schema.tables {
        snapshots.push(TableSnapshot {
            table_name: table.name.clone(),
            rows: count_rows(pool, &table.name).await?,
        });
    }
    snapshots.sort_by(|left, right| left.table_name.cmp(&right.table_name));
    Ok(snapshots)
}

async fn verify_storage(
    pool: &PgPool,
    schema: &postgres_schema::SchemaSpec,
) -> Result<StorageVerification, SyncVerificationError> {
    let table_names: Vec<_> = schema
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .collect();
    let table_bytes: i64 = sqlx::query_scalar(
        r"
        SELECT COALESCE(sum(pg_total_relation_size(format('%I', table_name)::regclass)), 0)::bigint
        FROM unnest($1::text[]) AS table_name
        ",
    )
    .bind(&table_names)
    .fetch_one(pool)
    .await?;
    let index_bytes: i64 = sqlx::query_scalar(
        r"
        SELECT COALESCE(sum(pg_indexes_size(format('%I', table_name)::regclass)), 0)::bigint
        FROM unnest($1::text[]) AS table_name
        ",
    )
    .bind(&table_names)
    .fetch_one(pool)
    .await?;
    let binary_asset_metadata_rows: i64 = sqlx::query_scalar(
        r"
        SELECT count(*)::bigint
        FROM document
        WHERE enclosure_url IS NOT NULL
           OR content_type IS NOT NULL
           OR content_length IS NOT NULL
        ",
    )
    .fetch_one(pool)
    .await?;
    let link_metadata_bytes: i64 = sqlx::query_scalar(
        r"
        SELECT COALESCE(sum(
            COALESCE(pg_column_size(asset_url), 0)
          + COALESCE(pg_column_size(upstream_url), 0)
          + COALESCE(pg_column_size(upstream_content_type), 0)
          + COALESCE(pg_column_size(upstream_content_length), 0)
          + COALESCE(pg_column_size(retrieval_status), 0)
          + COALESCE(pg_column_size(retrieval_error), 0)
        ), 0)::bigint
        FROM document_asset
        ",
    )
    .fetch_one(pool)
    .await?;
    let extracted_text_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(octet_length(extracted_text)), 0)::bigint FROM document_content",
    )
    .fetch_one(pool)
    .await?;
    let stored_html_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(octet_length(extracted_html)), 0)::bigint FROM document_content",
    )
    .fetch_one(pool)
    .await?;
    let constraint_count: i64 = sqlx::query_scalar(
        r"
        SELECT count(*)::bigint
        FROM pg_constraint constraint_row
        JOIN pg_class table_row ON table_row.oid = constraint_row.conrelid
        WHERE table_row.relname = ANY($1::text[])
        ",
    )
    .bind(&table_names)
    .fetch_one(pool)
    .await?;
    let document_content_rows: i64 =
        sqlx::query_scalar("SELECT count(*)::bigint FROM document_content")
            .fetch_one(pool)
            .await?;

    Ok(StorageVerification {
        table_bytes,
        index_bytes,
        link_metadata_bytes,
        extracted_text_bytes,
        stored_html_bytes,
        constraint_count,
        binary_asset_metadata_rows,
        document_content_rows,
    })
}

async fn count_category_rows(
    pool: &PgPool,
    table_name: &str,
    category: &str,
) -> Result<i64, sqlx::Error> {
    let sql = format!(
        "SELECT count(*)::bigint FROM {} WHERE source_category = $1",
        ident(table_name)
    );
    sqlx::query_scalar(&sql)
        .bind(category)
        .fetch_one(pool)
        .await
}

async fn count_rows(pool: &PgPool, table_name: &str) -> Result<i64, sqlx::Error> {
    let sql = format!("SELECT count(*)::bigint FROM {}", ident(table_name));
    sqlx::query_scalar(&sql).fetch_one(pool).await
}

async fn probe_relation_by_source(pool: &PgPool, table_name: &str) -> Result<(), sqlx::Error> {
    let sql = format!(
        "SELECT target_category, target_id FROM {} WHERE source_category = $1 LIMIT 1",
        ident(table_name)
    );
    sqlx::query(&sql).bind("").fetch_optional(pool).await?;
    Ok(())
}

async fn probe_relation_by_target(pool: &PgPool, table_name: &str) -> Result<(), sqlx::Error> {
    let sql = format!(
        "SELECT source_category, source_id FROM {} WHERE target_category = $1 LIMIT 1",
        ident(table_name)
    );
    sqlx::query(&sql).bind("").fetch_optional(pool).await?;
    Ok(())
}

fn has_index(
    schema: &postgres_schema::SchemaSpec,
    table_name: &str,
    purpose: IndexPurpose,
) -> bool {
    schema
        .indexes
        .iter()
        .any(|index| index.table_name == table_name && index.purpose == purpose)
}

fn parse_state(category: &str, state: &str) -> Result<CategorySyncState, SyncVerificationError> {
    match state {
        "not_started" => Ok(CategorySyncState::NotStarted),
        "running" => Ok(CategorySyncState::Running),
        "caught_up" => Ok(CategorySyncState::CaughtUp),
        "error" => Ok(CategorySyncState::Error),
        _ => Err(SyncVerificationError::UnknownState {
            category: category.to_owned(),
            state: state.to_owned(),
        }),
    }
}

fn ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
