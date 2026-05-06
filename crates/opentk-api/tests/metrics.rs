#[allow(dead_code)]
mod support;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use support::{migrated_pool, router_without_search};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_text() -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_metrics_text").await?;
    let router = router_without_search(pool);
    let categories = router
        .clone()
        .oneshot(Request::get("/categories").body(Body::empty())?)
        .await?;
    assert_eq!(categories.status(), StatusCode::OK);

    let response = router
        .oneshot(Request::get("/metrics").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .expect("metrics response has content type")
        .to_str()?;
    assert!(content_type.starts_with("text/plain; version=0.0.4"));

    let body = body_text(response).await?;
    assert!(body.contains("opentk_http_requests_total"));
    assert!(body.contains("opentk_http_request_duration_seconds"));
    assert!(!body.contains("route=\"/metrics\""));

    Ok(())
}

#[tokio::test]
async fn request_metrics_use_matched_routes_not_raw_targets(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_metrics_routes").await?;
    let router = router_without_search(pool);
    let document_id = Uuid::new_v4();

    let categories = router
        .clone()
        .oneshot(Request::get("/categories?ignored=true").body(Body::empty())?)
        .await?;
    assert_eq!(categories.status(), StatusCode::OK);

    let missing_document = router
        .clone()
        .oneshot(Request::get(format!("/documents/{document_id}?include=raw")).body(Body::empty())?)
        .await?;
    assert_eq!(missing_document.status(), StatusCode::NOT_FOUND);

    let metrics = router
        .oneshot(Request::get("/metrics").body(Body::empty())?)
        .await?;
    assert_eq!(metrics.status(), StatusCode::OK);
    let body = body_text(metrics).await?;

    assert!(body.contains(
        "opentk_http_requests_total{method=\"GET\",route=\"/categories\",status_class=\"2xx\"}"
    ));
    assert!(body.contains(
        "opentk_http_requests_total{method=\"GET\",route=\"/documents/{source_id}\",status_class=\"4xx\"}"
    ));
    assert!(!body.contains(&document_id.to_string()));
    assert!(!body.contains("ignored=true"));
    assert!(!body.contains("include=raw"));

    Ok(())
}

#[tokio::test]
async fn sync_metrics_are_refreshed_from_database() -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_metrics_sync").await?;
    sqlx::query(
        "INSERT INTO sync_category
         (source_category, latest_skiptoken, state, last_fetch_at, last_synced_at, caught_up_at, next_url, resume_url)
         VALUES
         ('Document', 100, 'caught_up', '2026-04-26T10:00:00Z', '2026-04-26T10:01:00Z',
          '2026-04-26T10:02:00Z', NULL, NULL)",
    )
    .execute(&pool)
    .await?;

    let response = router_without_search(pool)
        .oneshot(Request::get("/metrics").body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await?;

    assert!(body.contains("opentk_sync_category_lag_seconds{category=\"Document\"}"));
    assert!(body.contains(
        "opentk_sync_category_last_successful_sync_timestamp_seconds{category=\"Document\"}"
    ));
    assert!(
        body.contains("opentk_sync_category_caught_up_timestamp_seconds{category=\"Document\"}")
    );

    Ok(())
}

async fn body_text(
    response: axum::response::Response,
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), usize::MAX).await?.to_vec(),
    )?)
}
