use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::{net::SocketAddr, str::FromStr};
use tower::ServiceExt;

#[tokio::test]
async fn api_config_builds_router_connected_to_configured_database(
) -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run API startup tests");
    let bind_address = SocketAddr::from_str("127.0.0.1:0")?;

    let server = opentk_api::build_app(opentk_api::ApiConfig {
        bind_address,
        database: opentk_db::DatabaseConfig {
            url: database_url,
            max_connections: 5,
        },
        search: opentk_api::SearchBackendConfig {
            url: "http://search.example.test:7700".to_owned(),
            api_key: Some("startup-key".to_owned()),
            index_name: "startup_index".to_owned(),
        },
    })
    .await?;

    assert_eq!(server.bind_address, bind_address);
    let response = server
        .router
        .oneshot(Request::get("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert_eq!(body, serde_json::json!({ "status": "ok" }));

    Ok(())
}
