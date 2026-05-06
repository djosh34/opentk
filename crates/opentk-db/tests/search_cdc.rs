use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use opentk_db::{
    schema_lifecycle::ensure_schema,
    search_cdc::{
        run_search_cdc_listener, SearchCdcBatchConfig, SearchCdcBatchReport, SearchCdcBatcher,
        SearchCdcError, SearchCdcListener, SearchCdcNotification, SearchCdcRuntimeState,
        SearchCdcRuntimeStatus, SEARCH_CDC_CHANNEL,
    },
    search_sync::SearchSyncConfig,
};
use opentk_search::{SearchIndexClient, SearchIndexError, SearchIndexOperation, SearchIndexSchema};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::sync::broadcast;
use tokio::time::timeout;
use uuid::Uuid;

#[test]
fn runtime_status_starts_without_activity() {
    let status = SearchCdcRuntimeStatus::new().snapshot();

    assert_eq!(status.state, SearchCdcRuntimeState::Starting);
    assert_eq!(status.last_notification_at, None);
    assert_eq!(status.pending_count, 0);
    assert_eq!(status.last_batch, None);
    assert_eq!(status.last_error, None);
    assert_eq!(status.last_loop_at, None);
    assert_eq!(status.last_receive_started_at, None);
    assert_eq!(status.last_flush_started_at, None);
    assert_eq!(status.active_flush, None);
    assert_eq!(status.last_flush_duration_ms, None);
}

#[test]
fn runtime_status_records_transitions() {
    let status = SearchCdcRuntimeStatus::new();

    status.mark_running();
    status.record_loop();
    status.record_receive_started();
    status.record_notification(2);
    status.record_flush_started(2);
    status.record_batch(SearchCdcBatchReport {
        indexed: 42,
        deleted: 3,
        failed: 0,
        duration_ms: 1500,
    });
    status.record_error(&"fixture error");

    let snapshot = status.snapshot();
    assert_eq!(snapshot.state, SearchCdcRuntimeState::Degraded);
    assert!(snapshot.last_notification_at.is_some());
    assert_eq!(snapshot.pending_count, 0);
    assert!(snapshot.last_loop_at.is_some());
    assert!(snapshot.last_receive_started_at.is_some());
    assert!(snapshot.last_flush_started_at.is_some());
    assert_eq!(snapshot.active_flush, None);
    assert_eq!(snapshot.last_flush_duration_ms, Some(1500));
    assert_eq!(
        snapshot.last_batch,
        Some(SearchCdcBatchReport {
            indexed: 42,
            deleted: 3,
            failed: 0,
            duration_ms: 1500,
        })
    );
    assert_eq!(snapshot.last_error.as_deref(), Some("fixture error"));
}

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
    let schema_name: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await?;
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

    let change = timeout(Duration::from_secs(3), async {
        loop {
            let notification = listener.recv().await?;
            let change = SearchCdcNotification::from_payload(notification.payload())?;
            if change.schema() == Some(schema_name.as_str()) {
                return Ok::<_, Box<dyn std::error::Error>>(change);
            }
        }
    })
    .await??;
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
            max_payload_bytes: 80_000_000,
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

    let report = timeout(Duration::from_secs(3), listener.receive_once())
        .await??
        .expect("max unique records flushes immediately");

    assert_eq!(report.deleted, 1);
    assert_eq!(
        client.operations(),
        vec![SearchIndexOperation::Delete(format!(
            "Document_{source_id}"
        ))]
    );

    Ok(())
}

#[tokio::test]
async fn listener_flushes_after_elapsed_wall_time_between_notifications(
) -> Result<(), Box<dyn std::error::Error>> {
    let (pool, database_url) = migrated_pool("search_cdc_listener_elapsed_window").await?;
    let first_source_id =
        Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let second_source_id =
        Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid");
    let client = MemoryIndexClient::default();
    let mut listener = SearchCdcListener::connect(
        &database_url,
        pool.clone(),
        &client,
        SearchSyncConfig {
            index_name: "opentk_entities".to_owned(),
            categories: vec!["Document".to_owned()],
            batch_size: 10,
            max_payload_bytes: 80_000_000,
            retry_limit: 3,
        },
        SearchCdcBatchConfig {
            flush_window: Duration::from_millis(50),
            max_unique_records: 100,
        },
    )
    .await?;

    insert_sync_entity(&pool, first_source_id, 43, true).await?;
    let report = timeout(Duration::from_secs(3), listener.receive_once()).await??;
    assert!(report.is_none(), "single notification should stay buffered");

    let second_receive = listener.receive_once();
    tokio::time::sleep(Duration::from_millis(75)).await;
    let report = timeout(Duration::from_secs(3), second_receive)
        .await??
        .expect("elapsed wall time should flush the buffered batch");

    assert_eq!(report.deleted, 1);
    assert_eq!(
        client.operations(),
        vec![SearchIndexOperation::Delete(format!(
            "Document_{first_source_id}"
        ))]
    );

    insert_sync_entity(&pool, second_source_id, 44, true).await?;
    let report = timeout(Duration::from_secs(3), listener.receive_once()).await??;
    assert!(
        report.is_none(),
        "second notification should start a new batch"
    );

    let report = timeout(Duration::from_secs(3), listener.receive_once())
        .await??
        .expect("second batch should flush on its own timer");
    assert_eq!(report.deleted, 1);
    assert_eq!(
        client.operations(),
        vec![
            SearchIndexOperation::Delete(format!("Document_{first_source_id}")),
            SearchIndexOperation::Delete(format!("Document_{second_source_id}")),
        ]
    );

    Ok(())
}

#[tokio::test]
async fn daemon_records_batch_and_stops_after_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    let (pool, database_url) = migrated_pool("search_cdc_daemon_success").await?;
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let client = Arc::new(MemoryIndexClient::default());
    let status = SearchCdcRuntimeStatus::new();
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let task_status = status.clone();
    let task = tokio::spawn(run_search_cdc_listener(
        database_url,
        pool.clone(),
        client.clone(),
        search_sync_config(),
        SearchCdcBatchConfig {
            flush_window: Duration::from_secs(1),
            max_unique_records: 1,
        },
        shutdown_rx,
        task_status,
    ));

    wait_for_state(&status, SearchCdcRuntimeState::Running).await;
    insert_sync_entity(&pool, source_id, 43, true).await?;
    wait_for_last_batch(&status).await;
    shutdown_tx.send(())?;
    task.await??;

    let snapshot = status.snapshot();
    assert_eq!(snapshot.state, SearchCdcRuntimeState::Stopped);
    assert_eq!(snapshot.pending_count, 0);
    assert_eq!(snapshot.active_flush, None);
    assert!(snapshot.last_flush_duration_ms.is_some());
    assert_eq!(snapshot.last_batch.expect("last batch").deleted, 1);
    assert_eq!(
        client.operations(),
        vec![SearchIndexOperation::Delete(format!(
            "Document_{source_id}"
        ))]
    );

    Ok(())
}

#[tokio::test]
async fn daemon_flushes_single_notification_after_elapsed_window(
) -> Result<(), Box<dyn std::error::Error>> {
    let (pool, database_url) = migrated_pool("search_cdc_daemon_timer_flush").await?;
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let client = Arc::new(MemoryIndexClient::default());
    let status = SearchCdcRuntimeStatus::new();
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let task_status = status.clone();
    let task = tokio::spawn(run_search_cdc_listener(
        database_url,
        pool.clone(),
        client.clone(),
        search_sync_config(),
        SearchCdcBatchConfig {
            flush_window: Duration::from_millis(50),
            max_unique_records: 100,
        },
        shutdown_rx,
        task_status,
    ));

    wait_for_state(&status, SearchCdcRuntimeState::Running).await;
    insert_sync_entity(&pool, source_id, 45, true).await?;
    wait_for_pending_count(&status, 1).await;
    wait_for_last_batch(&status).await;
    shutdown_tx.send(())?;
    task.await??;

    let snapshot = status.snapshot();
    assert_eq!(snapshot.state, SearchCdcRuntimeState::Stopped);
    assert_eq!(snapshot.pending_count, 0);
    assert_eq!(snapshot.active_flush, None);
    assert!(snapshot.last_flush_duration_ms.is_some());
    assert_eq!(snapshot.last_batch.expect("last batch").deleted, 1);
    assert_eq!(
        client.operations(),
        vec![SearchIndexOperation::Delete(format!(
            "Document_{source_id}"
        ))]
    );

    Ok(())
}

#[tokio::test]
async fn daemon_exposes_active_flush_while_indexing_is_in_progress(
) -> Result<(), Box<dyn std::error::Error>> {
    let (pool, database_url) = migrated_pool("search_cdc_daemon_active_flush").await?;
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let client = Arc::new(BlockingIndexClient::default());
    let status = SearchCdcRuntimeStatus::new();
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let task_status = status.clone();
    let task = tokio::spawn(run_search_cdc_listener(
        database_url,
        pool.clone(),
        client.clone(),
        search_sync_config(),
        SearchCdcBatchConfig {
            flush_window: Duration::from_secs(1),
            max_unique_records: 1,
        },
        shutdown_rx,
        task_status,
    ));

    wait_for_state(&status, SearchCdcRuntimeState::Running).await;
    insert_sync_entity(&pool, source_id, 46, true).await?;
    wait_for_apply_started(&client).await;
    let active_flush = wait_for_active_flush(&status).await;
    assert_eq!(active_flush.pending_count, 1);
    assert!(status.snapshot().last_flush_started_at.is_some());
    assert_eq!(status.snapshot().last_batch, None);

    client.release();
    wait_for_last_batch(&status).await;
    shutdown_tx.send(())?;
    task.await??;

    let snapshot = status.snapshot();
    assert_eq!(snapshot.active_flush, None);
    assert_eq!(snapshot.pending_count, 0);
    assert!(snapshot.last_flush_duration_ms.is_some());
    assert_eq!(
        client.operations(),
        vec![SearchIndexOperation::Delete(format!(
            "Document_{source_id}"
        ))]
    );

    Ok(())
}

#[tokio::test]
async fn daemon_records_indexing_error_and_keeps_running_until_shutdown(
) -> Result<(), Box<dyn std::error::Error>> {
    let (pool, database_url) = migrated_pool("search_cdc_daemon_error").await?;
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid");
    let client = Arc::new(FailingIndexClient);
    let status = SearchCdcRuntimeStatus::new();
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let task_status = status.clone();
    let task = tokio::spawn(run_search_cdc_listener(
        database_url,
        pool.clone(),
        client,
        search_sync_config(),
        SearchCdcBatchConfig {
            flush_window: Duration::from_secs(1),
            max_unique_records: 1,
        },
        shutdown_rx,
        task_status,
    ));

    wait_for_state(&status, SearchCdcRuntimeState::Running).await;
    insert_sync_entity(&pool, source_id, 44, true).await?;
    wait_for_state(&status, SearchCdcRuntimeState::Degraded).await;
    assert!(status
        .snapshot()
        .last_error
        .as_deref()
        .is_some_and(|error| error.contains("fixture indexing failure")));
    assert!(
        !task.is_finished(),
        "daemon must keep listening after batch errors"
    );
    shutdown_tx.send(())?;
    task.await??;

    Ok(())
}

fn notification(source_id: Uuid, latest_skiptoken: i64, deleted: bool) -> SearchCdcNotification {
    SearchCdcNotification::new("Document".to_owned(), source_id, latest_skiptoken, deleted)
}

fn search_sync_config() -> SearchSyncConfig {
    SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    }
}

async fn insert_sync_entity(
    pool: &PgPool,
    source_id: Uuid,
    latest_skiptoken: i64,
    deleted: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sync_entity (
            source_category,
            source_id,
            latest_skiptoken,
            deleted,
            source_updated_at,
            atom_updated_at
         )
         VALUES ('Document', $1, $2, $3, '2026-04-26T00:00:00Z', '2026-04-26T01:00:00Z')",
    )
    .bind(source_id)
    .bind(latest_skiptoken)
    .bind(deleted)
    .execute(pool)
    .await?;
    Ok(())
}

async fn wait_for_state(status: &SearchCdcRuntimeStatus, state: SearchCdcRuntimeState) {
    timeout(Duration::from_secs(3), async {
        loop {
            if status.snapshot().state == state {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("status reaches expected state");
}

async fn wait_for_last_batch(status: &SearchCdcRuntimeStatus) {
    timeout(Duration::from_secs(3), async {
        loop {
            if status.snapshot().last_batch.is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("daemon records a batch");
}

async fn wait_for_active_flush(
    status: &SearchCdcRuntimeStatus,
) -> opentk_db::search_cdc::SearchCdcActiveFlushStatus {
    timeout(Duration::from_secs(3), async {
        loop {
            if let Some(active_flush) = status.snapshot().active_flush {
                return active_flush;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("daemon exposes active flush")
}

async fn wait_for_pending_count(status: &SearchCdcRuntimeStatus, pending_count: usize) {
    timeout(Duration::from_secs(3), async {
        loop {
            if status.snapshot().pending_count == pending_count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("daemon records expected pending count");
}

async fn wait_for_apply_started(client: &BlockingIndexClient) {
    timeout(Duration::from_secs(3), async {
        loop {
            if client.apply_started.load(Ordering::SeqCst) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("blocking client starts applying a batch");
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
    ensure_schema(&pool)
        .await
        .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
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

struct FailingIndexClient;

impl SearchIndexClient for FailingIndexClient {
    async fn reset_index(&self, _schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        Ok(())
    }

    async fn apply_batch(
        &self,
        _operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError> {
        Err(SearchIndexError::Http {
            status: Some(503),
            message: "fixture indexing failure".to_owned(),
        })
    }
}

#[derive(Default)]
struct BlockingIndexClient {
    operations: Mutex<Vec<SearchIndexOperation>>,
    apply_started: AtomicBool,
    release: tokio::sync::Notify,
}

impl BlockingIndexClient {
    fn operations(&self) -> Vec<SearchIndexOperation> {
        self.operations.lock().expect("operations mutex").clone()
    }

    fn release(&self) {
        self.release.notify_waiters();
    }
}

impl SearchIndexClient for BlockingIndexClient {
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
        self.apply_started.store(true, Ordering::SeqCst);
        self.release.notified().await;
        Ok(())
    }
}
