use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use opentk_db::{
    search_cdc::{
        SearchCdcBatchConfig, SearchCdcBatcher, SearchCdcError, SearchCdcListener,
        SearchCdcNotification, SEARCH_CDC_CHANNEL,
    },
    search_sync::SearchSyncConfig,
};
use opentk_search::{SearchIndexClient, SearchIndexError, SearchIndexOperation, SearchIndexSchema};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::time::timeout;
use uuid::Uuid;

#[test]
fn notification_payload_parses_into_typed_change() {
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let payload = format!(
        r#"{{
            "source_category": "Document",
            "source_id": "{source_id}",
            "latest_skiptoken": 42,
            "deleted": false
        }}"#
    );

    let notification = SearchCdcNotification::from_payload(&payload).expect("payload parses");

    assert_eq!(notification.source_category(), "Document");
    assert_eq!(notification.source_id(), source_id);
    assert_eq!(notification.latest_skiptoken(), 42);
    assert!(!notification.deleted());
}

#[test]
fn invalid_notification_payload_returns_typed_error() {
    let error = SearchCdcNotification::from_payload(
        r#"{
            "source_category": "Document",
            "source_id": "11111111-1111-4111-8111-111111111111",
            "deleted": false
        }"#,
    )
    .expect_err("missing skiptoken is rejected");

    assert!(matches!(error, SearchCdcError::InvalidPayload(_)));
}

#[test]
fn batcher_deduplicates_changes_and_keeps_newest_skiptoken() {
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let mut batcher = SearchCdcBatcher::new(SearchCdcBatchConfig {
        flush_window: Duration::from_secs(1),
        max_unique_records: 100,
    });
    let now = Instant::now();

    batcher.push(notification(source_id, 41, false), now);
    batcher.push(notification(source_id, 43, true), now);
    batcher.push(notification(source_id, 42, false), now);

    let changes = batcher.drain();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].source_id(), source_id);
    assert_eq!(changes[0].latest_skiptoken(), 43);
    assert!(changes[0].deleted());
    assert!(batcher.is_empty());
}

#[test]
fn batcher_flushes_on_elapsed_window_or_unique_record_limit() {
    let mut batcher = SearchCdcBatcher::new(SearchCdcBatchConfig {
        flush_window: Duration::from_secs(1),
        max_unique_records: 2,
    });
    let first = Instant::now();

    batcher.push(notification(Uuid::new_v4(), 1, false), first);
    assert!(!batcher.should_flush(first + Duration::from_millis(999)));
    assert!(batcher.should_flush(first + Duration::from_secs(1)));

    let mut batcher = SearchCdcBatcher::new(SearchCdcBatchConfig {
        flush_window: Duration::from_secs(1),
        max_unique_records: 2,
    });
    batcher.push(notification(Uuid::new_v4(), 1, false), first);
    assert!(!batcher.should_flush(first));
    batcher.push(notification(Uuid::new_v4(), 2, false), first);
    assert!(batcher.should_flush(first));
}

#[tokio::test]
async fn sync_entity_trigger_notifies_changed_record() -> Result<(), Box<dyn std::error::Error>> {
    let (pool, database_url) = migrated_pool("search_cdc_trigger").await?;
    let mut listener = sqlx::postgres::PgListener::connect(&database_url).await?;
    listener.listen(SEARCH_CDC_CHANNEL).await?;
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");

    sqlx::query(
        "INSERT INTO sync_entity (
            source_category,
            source_id,
            latest_skiptoken,
            deleted,
            source_updated_at,
            atom_updated_at
         )
         VALUES ('Document', $1, 42, false, '2026-04-26T00:00:00Z', '2026-04-26T01:00:00Z')",
    )
    .bind(source_id)
    .execute(&pool)
    .await?;

    let notification = timeout(Duration::from_secs(3), listener.recv()).await??;
    let change = SearchCdcNotification::from_payload(notification.payload())?;
    assert_eq!(change.source_category(), "Document");
    assert_eq!(change.source_id(), source_id);
    assert_eq!(change.latest_skiptoken(), 42);
    assert!(!change.deleted());

    Ok(())
}

#[tokio::test]
async fn listener_receives_postgres_notification_and_flushes(
) -> Result<(), Box<dyn std::error::Error>> {
    let (pool, database_url) = migrated_pool("search_cdc_listener_receive").await?;
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let client = MemoryIndexClient::default();
    let mut listener = SearchCdcListener::connect(
        &database_url,
        pool.clone(),
        &client,
        SearchSyncConfig {
            index_name: "opentk_entities".to_owned(),
            categories: vec!["Document".to_owned()],
            batch_size: 10,
            retry_limit: 3,
        },
        SearchCdcBatchConfig {
            flush_window: Duration::from_secs(1),
            max_unique_records: 1,
        },
    )
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
         VALUES ('Document', $1, 43, true, '2026-04-26T00:00:00Z', '2026-04-26T01:00:00Z')",
    )
    .bind(source_id)
    .execute(&pool)
    .await?;

    let report = timeout(
        Duration::from_secs(3),
        listener.receive_once(Instant::now()),
    )
    .await??
    .expect("max unique records flushes immediately");

    assert_eq!(report.deleted, 1);
    assert_eq!(
        client.operations(),
        vec![SearchIndexOperation::Delete(format!(
            "Document:{source_id}"
        ))]
    );

    Ok(())
}

fn notification(source_id: Uuid, latest_skiptoken: i64, deleted: bool) -> SearchCdcNotification {
    SearchCdcNotification::new("Document".to_owned(), source_id, latest_skiptoken, deleted)
}

async fn migrated_pool(test_name: &str) -> Result<(PgPool, String), sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL search CDC tests");
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

    let database_url = with_search_path(&database_url, &schema_name);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok((pool, database_url))
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[derive(Default)]
struct MemoryIndexClient {
    operations: Mutex<Vec<SearchIndexOperation>>,
}

impl MemoryIndexClient {
    fn operations(&self) -> Vec<SearchIndexOperation> {
        self.operations.lock().expect("operations mutex").clone()
    }
}

impl SearchIndexClient for MemoryIndexClient {
    async fn reset_index(&self, _schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        Ok(())
    }

    async fn apply_batch(
        &self,
        operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError> {
        self.operations
            .lock()
            .expect("operations mutex")
            .extend_from_slice(operations);
        Ok(())
    }
}
