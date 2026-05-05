use opentk_db::schema_lifecycle::{ensure_schema, validate_schema};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

#[tokio::test]
async fn ensure_schema_creates_expected_schema_in_empty_postgres_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("schema_lifecycle_empty").await?;

    ensure_schema(&pool).await?;

    let sync_entity =
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('sync_entity')::text")
            .fetch_one(&pool)
            .await?;
    assert_eq!(sync_entity.as_deref(), Some("sync_entity"));

    let document_number_indexes: i64 = sqlx::query(
        "SELECT count(*)::bigint AS count
         FROM pg_indexes
         WHERE schemaname = current_schema()
           AND tablename = 'document'
           AND indexdef LIKE '%document_nummer%'",
    )
    .fetch_one(&pool)
    .await?
    .get("count");
    assert!(
        document_number_indexes > 0,
        "document.document_nummer lookup index must be created"
    );
    let change_feed_page_indexes: i64 = sqlx::query(
        "SELECT count(*)::bigint AS count
         FROM pg_indexes
         WHERE schemaname = current_schema()
           AND tablename = 'sync_entity'
           AND indexdef LIKE '%source_category%'
           AND indexdef LIKE '%latest_skiptoken%'
           AND indexdef LIKE '%source_id%'",
    )
    .fetch_one(&pool)
    .await?
    .get("count");
    assert!(
        change_feed_page_indexes > 0,
        "sync_entity changes pagination index must be created"
    );

    Ok(())
}

#[tokio::test]
async fn ensure_schema_is_idempotent_and_preserves_sync_state(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("schema_lifecycle_preserve").await?;
    ensure_schema(&pool).await?;
    sqlx::query(
        "INSERT INTO sync_category (
            source_category,
            latest_skiptoken,
            state,
            next_url
        )
        VALUES ('Document', 42, 'caught_up', 'https://example.test/SyncFeed?skiptoken=42')",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "INSERT INTO sync_entity (
            source_category,
            source_id,
            latest_skiptoken,
            deleted,
            source_updated_at,
            atom_updated_at
        )
        VALUES (
            'Document',
            '11111111-1111-4111-8111-111111111111',
            42,
            false,
            '2026-04-26T00:00:00Z',
            '2026-04-26T00:00:00Z'
        )",
    )
    .execute(&pool)
    .await?;

    ensure_schema(&pool).await?;

    let latest_skiptoken: i64 = sqlx::query_scalar(
        "SELECT latest_skiptoken FROM sync_category WHERE source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    let entity_count: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM sync_entity")
        .fetch_one(&pool)
        .await?;
    assert_eq!(latest_skiptoken, 42);
    assert_eq!(entity_count, 1);
    Ok(())
}

#[tokio::test]
async fn validate_schema_fails_when_schema_is_missing() -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("schema_lifecycle_missing").await?;

    let error = validate_schema(&pool)
        .await
        .expect_err("missing schema must fail validation");

    assert!(error.to_string().contains("missing table"), "{error}");
    Ok(())
}

#[tokio::test]
async fn validate_schema_fails_when_column_shape_is_incompatible(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("schema_lifecycle_incompatible").await?;
    sqlx::query("CREATE TABLE sync_category (source_category integer NOT NULL)")
        .execute(&pool)
        .await?;

    let error = validate_schema(&pool)
        .await
        .expect_err("incompatible schema must fail validation");

    assert!(error.to_string().contains("sync_category"), "{error}");
    assert!(error.to_string().contains("source_category"), "{error}");
    Ok(())
}

#[tokio::test]
async fn validate_schema_accepts_schema_created_by_ensure_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("schema_lifecycle_valid").await?;

    ensure_schema(&pool).await?;

    validate_schema(&pool).await?;
    Ok(())
}

async fn empty_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL schema tests");
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let schema_name = format!("opentk_{test_name}_{}", std::process::id());
    sqlx::query(&format!(
        "DROP SCHEMA IF EXISTS {} CASCADE",
        quote_ident(&schema_name)
    ))
    .execute(&admin_pool)
    .await?;
    sqlx::query(&format!("CREATE SCHEMA {}", quote_ident(&schema_name)))
        .execute(&admin_pool)
        .await?;
    admin_pool.close().await;

    PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_search_path(&database_url, &schema_name))
        .await
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
