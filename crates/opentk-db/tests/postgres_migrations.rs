use sqlx::{postgres::PgPoolOptions, Row};

#[tokio::test]
async fn sqlx_migrations_run_and_revert_against_fresh_postgres_schema() -> Result<(), sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL migration tests");

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    let schema_name = format!("opentk_migration_test_{}", std::process::id());
    let drop_schema_sql = format!(
        "DROP SCHEMA IF EXISTS {schema} CASCADE",
        schema = quote_ident(&schema_name)
    );
    sqlx::query(&drop_schema_sql).execute(&admin_pool).await?;

    let create_schema_sql = format!("CREATE SCHEMA {schema}", schema = quote_ident(&schema_name));
    sqlx::query(&create_schema_sql).execute(&admin_pool).await?;

    let schema_url = with_search_path(&database_url, &schema_name);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&schema_url)
        .await?;

    sqlx::migrate!("../../migrations").run(&pool).await?;

    let sync_entity =
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('sync_entity')::text")
            .fetch_one(&pool)
            .await?;
    assert_eq!(sync_entity.as_deref(), Some("sync_entity"));

    let document_number_index_count: i64 = sqlx::query(
        "SELECT count(*)::bigint AS count
         FROM pg_indexes
         WHERE schemaname = $1
           AND tablename = 'document'
           AND indexdef LIKE '%document_nummer%'",
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await?
    .get("count");
    assert!(
        document_number_index_count > 0,
        "document.document_nummer lookup index must be created"
    );

    let document_asset =
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('document_asset')::text")
            .fetch_one(&pool)
            .await?;
    assert_eq!(document_asset.as_deref(), Some("document_asset"));

    let document_content =
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('document_content')::text")
            .fetch_one(&pool)
            .await?;
    assert_eq!(document_content.as_deref(), Some("document_content"));

    let document_asset_url_index_count: i64 = sqlx::query(
        "SELECT count(*)::bigint AS count
         FROM pg_indexes
         WHERE schemaname = $1
           AND tablename = 'document_asset'
           AND indexdef LIKE '%asset_url%'",
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await?
    .get("count");
    assert!(
        document_asset_url_index_count > 0,
        "document_asset.asset_url lookup index must be created"
    );

    let document_content_owner_index_count: i64 = sqlx::query(
        "SELECT count(*)::bigint AS count
         FROM pg_indexes
         WHERE schemaname = $1
           AND tablename = 'document_content'
           AND indexdef LIKE '%document_source_category%'
           AND indexdef LIKE '%document_source_id%'",
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await?
    .get("count");
    assert!(
        document_content_owner_index_count > 0,
        "document_content document owner lookup index must be created"
    );

    sqlx::migrate!("../../migrations").undo(&pool, 0).await?;

    let reverted_sync_entity =
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('sync_entity')::text")
            .fetch_one(&pool)
            .await?;
    assert_eq!(reverted_sync_entity, None);

    pool.close().await;
    sqlx::query(&drop_schema_sql).execute(&admin_pool).await?;
    admin_pool.close().await;

    Ok(())
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
