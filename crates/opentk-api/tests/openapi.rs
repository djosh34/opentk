use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opentk_db::{connect, DatabaseConfig};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn openapi_document_is_served_from_router() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run API OpenAPI tests");
    let pool = connect(&DatabaseConfig {
        url: database_url,
        max_connections: 1,
    })
    .await?;

    let response = opentk_api::router(pool)
        .oneshot(Request::get("/openapi.json").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert!(body.get("openapi").and_then(Value::as_str).is_some());
    assert!(body.pointer("/paths/~1health").is_some());
    assert!(body.pointer("/paths/~1openapi.json").is_some());

    Ok(())
}
