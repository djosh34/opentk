use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use opentk_db::{
    connect,
    search_cdc::{SearchCdcBatchReport, SearchCdcRuntimeStatus},
    DatabaseConfig,
};
use opentk_search::{
    SearchCountClient, SearchFilter, SearchHealthClient, SearchIndexError, SearchQueryClient,
    SearchRequest, SearchResponse,
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{future::Future, pin::Pin, sync::Arc};
use tower::ServiceExt;

#[tokio::test]
async fn health_reports_ok_when_database_is_reachable() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run API health tests");
    let pool = connect(&DatabaseConfig {
        url: database_url,
        max_connections: 1,
    })
    .await?;

    let response = router_without_search(pool)
        .oneshot(Request::get("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["postgres"], "ok");
    assert_eq!(body["meilisearch"], "unavailable");
    assert_eq!(body["search_sync"]["state"], "starting");

    Ok(())
}

#[tokio::test]
async fn health_returns_json_error_when_database_is_unreachable(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(10))
        .connect_lazy("postgres://postgres@127.0.0.1:1/postgres")?;

    let response = router_without_search(pool)
        .oneshot(Request::get("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(
        body,
        serde_json::json!({
            "code": "database_unavailable",
            "message": "database unavailable"
        })
    );

    Ok(())
}

#[tokio::test]
async fn health_reports_search_sync_degraded() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run API health tests");
    let pool = connect(&DatabaseConfig {
        url: database_url,
        max_connections: 1,
    })
    .await?;
    let status = SearchCdcRuntimeStatus::new();
    status.record_error(&"fixture CDC failure");

    let response = router_without_search_with_status(pool, status)
        .oneshot(Request::get("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["postgres"], "ok");
    assert_eq!(body["meilisearch"], "unavailable");
    assert_eq!(body["search_sync"]["state"], "degraded");
    assert_eq!(body["search_sync"]["last_error"], "fixture CDC failure");

    Ok(())
}

#[tokio::test]
async fn admin_search_sync_status_returns_daemon_snapshot() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://opentk.invalid/opentk")?;
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

    let response = router_without_search_with_status(pool, status.clone())
        .oneshot(Request::get("/admin/search-sync/status").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body["state"], "running");
    assert!(body["last_notification_at"].as_str().is_some());
    assert_eq!(body["pending_count"], 0);
    assert!(body["last_loop_at"].as_str().is_some());
    assert!(body["last_receive_started_at"].as_str().is_some());
    assert!(body["last_flush_started_at"].as_str().is_some());
    assert_eq!(body["active_flush"], Value::Null);
    assert_eq!(body["last_flush_duration_ms"], 1500);
    assert_eq!(
        body["last_batch"],
        serde_json::json!({
            "indexed": 42,
            "deleted": 3,
            "failed": 0,
            "duration_ms": 1500
        })
    );
    assert_eq!(body["last_error"], Value::Null);

    status.record_flush_started(4);
    let response = router_without_search_with_status(
        PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://opentk.invalid/opentk")?,
        status,
    )
    .oneshot(Request::get("/admin/search-sync/status").body(Body::empty())?)
    .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert!(body["active_flush"]["started_at"].as_str().is_some());
    assert_eq!(body["active_flush"]["pending_count"], 4);

    Ok(())
}

fn router_without_search(pool: PgPool) -> Router {
    opentk_api::router_with_search(pool, Arc::new(UnavailableSearchClient))
}

fn router_without_search_with_status(pool: PgPool, status: SearchCdcRuntimeStatus) -> Router {
    opentk_api::router_with_search_and_cdc_status(
        pool,
        Arc::new(UnavailableSearchClient),
        opentk_db::search_sync::SearchSyncConfig::default(),
        status,
    )
}

struct UnavailableSearchClient;

impl SearchQueryClient for UnavailableSearchClient {
    fn search<'a>(
        &'a self,
        _request: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search unavailable in health tests".to_owned(),
            })
        })
    }
}

impl SearchHealthClient for UnavailableSearchClient {
    fn health<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<(), SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search unavailable in health tests".to_owned(),
            })
        })
    }
}

impl SearchCountClient for UnavailableSearchClient {
    fn count<'a>(
        &'a self,
        _filter: SearchFilter,
    ) -> Pin<Box<dyn Future<Output = Result<u64, SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search unavailable in health tests".to_owned(),
            })
        })
    }
}
