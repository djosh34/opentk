use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use opentk_db::{connect, DatabaseConfig};
use opentk_search::{SearchIndexError, SearchQueryClient, SearchRequest, SearchResponse};
use serde_json::Value;
use sqlx::PgPool;
use std::{future::Future, pin::Pin, sync::Arc};
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

    let response = router_without_search(pool)
        .oneshot(Request::get("/openapi.json").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert!(body.get("openapi").and_then(Value::as_str).is_some());
    assert!(body.pointer("/paths/~1health").is_some());
    assert!(body.pointer("/paths/~1openapi.json").is_some());
    assert!(body.pointer("/paths/~1search/get").is_some());
    let search = &body["paths"]["/search"]["get"];
    let parameters = search["parameters"].as_array().expect("parameters array");
    for name in ["q", "limit", "offset", "category", "entity_kind"] {
        assert!(
            parameters
                .iter()
                .any(|parameter| parameter["name"].as_str() == Some(name)),
            "missing {name} search parameter"
        );
    }
    assert!(body
        .pointer("/components/schemas/SearchResponseDto")
        .is_some());
    assert!(body
        .pointer("/components/schemas/SearchResultDto")
        .is_some());
    assert!(body
        .pointer("/components/schemas/SearchSnippetDto")
        .is_some());

    Ok(())
}

fn router_without_search(pool: PgPool) -> Router {
    opentk_api::router_with_search(pool, Arc::new(UnavailableSearchClient))
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
                message: "search unavailable in OpenAPI tests".to_owned(),
            })
        })
    }
}
