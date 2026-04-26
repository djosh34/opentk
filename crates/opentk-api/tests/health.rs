use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opentk_db::{connect, DatabaseConfig};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
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

    let response = opentk_api::router(pool)
        .oneshot(Request::get("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body, serde_json::json!({ "status": "ok" }));

    Ok(())
}

#[tokio::test]
async fn health_returns_json_error_when_database_is_unreachable(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(10))
        .connect_lazy("postgres://postgres@127.0.0.1:1/postgres")?;

    let response = opentk_api::router(pool)
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
