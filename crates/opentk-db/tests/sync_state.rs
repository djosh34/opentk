use chrono::{DateTime, Utc};
use opentk_db::sync_state::PostgresSyncStore;
use opentk_sync::payload::parse_entity_xml;
use opentk_sync::runner::{
    CategorySyncState, DurableSyncError, PreparedSyncPage, StoredCategoryCursor, SyncPhase,
    SyncStore,
};
use reqwest::Url;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[tokio::test]
async fn postgres_store_reports_cursor_state_lag_fetch_time_and_last_error(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("sync_state_status").await?;
    let store = PostgresSyncStore::new(pool.clone());
    let next_url = Url::parse(
        "https://example.test/SyncFeed/2.0/Feed?category=Document&skiptoken=7&content=internal",
    )
    .expect("valid next URL");

    store
        .write_page(PreparedSyncPage {
            category: "Document".to_owned(),
            latest_skiptoken: 7,
            next_url: next_url.clone(),
            atom_updated_at: atom_updated_at(),
            entities: vec![
                parse_entity_xml("Document", &document_xml()).expect("document payload parses")
            ],
        })
        .await
        .expect("page writes through store");
    store
        .record_error(DurableSyncError {
            phase: SyncPhase::Write,
            category: "Document".to_owned(),
            skiptoken: Some(7),
            entity_id: Some(document_id()),
            message: "write failed".to_owned(),
        })
        .await
        .expect("error records");

    let cursor = store
        .load_category_cursor("Document")
        .await
        .expect("cursor loads")
        .expect("cursor exists");
    assert_eq!(
        cursor,
        StoredCategoryCursor {
            category: "Document".to_owned(),
            latest_skiptoken: 7,
            next_url,
            caught_up: false,
        }
    );

    let statuses = store
        .status(&["Document".to_owned(), "Zaak".to_owned()])
        .await
        .expect("status loads");

    assert_eq!(statuses[0].category, "Document");
    assert_eq!(statuses[0].latest_skiptoken, Some(7));
    assert_eq!(statuses[0].state, CategorySyncState::Error);
    assert!(statuses[0].lag.is_some());
    assert!(statuses[0].last_fetch_at.is_some());
    assert_eq!(
        statuses[0].last_error.as_ref().expect("last error").phase,
        SyncPhase::Write
    );
    assert_eq!(statuses[1].state, CategorySyncState::NotStarted);
    assert_eq!(statuses[1].latest_skiptoken, None);

    let stored_phase: String = sqlx::query_scalar("SELECT phase FROM ingest_error LIMIT 1")
        .fetch_one(&pool)
        .await?;
    assert_eq!(stored_phase, "write");
    Ok(())
}

#[tokio::test]
async fn postgres_store_marks_caught_up_from_resume_cursor() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("sync_state_caught_up").await?;
    let store = PostgresSyncStore::new(pool);
    let resume_url = Url::parse(
        "https://example.test/SyncFeed/2.0/Feed?category=Document&skiptoken=9&content=internal",
    )
    .expect("valid resume URL");

    store
        .mark_caught_up("Document", resume_url.clone(), atom_updated_at())
        .await
        .expect("caught-up mark writes");

    let cursor = store
        .load_category_cursor("Document")
        .await
        .expect("cursor loads")
        .expect("cursor exists");
    assert_eq!(cursor.latest_skiptoken, 9);
    assert_eq!(cursor.next_url, resume_url);
    assert!(cursor.caught_up);
    Ok(())
}

#[tokio::test]
async fn postgres_store_rejects_future_last_fetch_lag() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("sync_state_future_lag").await?;
    let store = PostgresSyncStore::new(pool.clone());
    let future_fetch_at = Utc::now() + chrono::Duration::days(1);

    sqlx::query(
        r"
        INSERT INTO sync_category (
            source_category,
            latest_skiptoken,
            next_url,
            state,
            last_fetch_at
        )
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind("Document")
    .bind(7_i64)
    .bind("https://example.test/SyncFeed/2.0/Feed?category=Document&skiptoken=7")
    .bind(CategorySyncState::Running.as_str())
    .bind(future_fetch_at)
    .execute(&pool)
    .await?;

    let error = store
        .status(&["Document".to_owned()])
        .await
        .expect_err("future last_fetch_at makes lag invalid");

    assert!(error.message.contains("last_fetch_at"));
    assert!(error.message.contains("future"));
    Ok(())
}

async fn migrated_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL sync_state tests");
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

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_search_path(&database_url, &schema_name))
        .await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(pool)
}

fn document_xml() -> String {
    r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
        id="11111111-1111-4111-8111-111111111111"
        verwijderd="false"
        bijgewerkt="2026-04-26T00:00:00Z"
        contentType="application/pdf"
        contentLength="12345">
        <documentNummer>2026D00001</documentNummer>
        <onderwerp>State task</onderwerp>
        <datum>2026-04-26T00:00:00Z</datum>
        <volgnummer>1</volgnummer>
        <vergaderjaar>2025-2026</vergaderjaar>
        <kamer>2</kamer>
    </document>"#
        .to_owned()
}

fn atom_updated_at() -> DateTime<Utc> {
    "2025-04-26T01:00:00Z".parse().expect("valid timestamp")
}

fn document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid")
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
