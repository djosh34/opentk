use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opentk_search::{
    SearchEntityKind, SearchIndexError, SearchQueryClient, SearchRequest, SearchResponse,
    SearchResult, SearchSnippet,
};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn search_endpoint_returns_document_results() -> Result<(), Box<dyn std::error::Error>> {
    let source_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111")?;
    let search = FakeSearchClient::with_response(SearchResponse {
        query: "fixture".to_owned(),
        limit: 2,
        offset: 0,
        estimated_total_hits: Some(1),
        results: vec![SearchResult {
            key: format!("Document:{source_id}"),
            source_category: "Document".to_owned(),
            source_id,
            entity_kind: SearchEntityKind::Document,
            title: "Fixture document".to_owned(),
            summary: Some("Read endpoint".to_owned()),
            source_url: Some("https://example.test/document.pdf".to_owned()),
            date: Some("2026-04-26T00:00:00Z".to_owned()),
            document_number: Some("2026D00001".to_owned()),
            snippets: vec![SearchSnippet {
                field: "extracted_text".to_owned(),
                text: "plain snippet".to_owned(),
                highlighted: Some("plain <em>snippet</em>".to_owned()),
            }],
            ranking_score: Some(0.98),
        }],
    });

    let body = router_json_with_search(search.clone(), "/search?q=fixture&limit=2", StatusCode::OK)
        .await?;

    assert_eq!(
        search.requests(),
        vec![SearchRequest {
            query: "fixture".to_owned(),
            limit: 2,
            offset: 0,
            filter: None,
        }]
    );
    assert_eq!(body["query"], "fixture");
    assert_eq!(body["limit"], 2);
    assert_eq!(body["offset"], 0);
    assert_eq!(body["estimated_total_hits"], 1);
    assert_eq!(body["items"][0]["key"], format!("Document:{source_id}"));
    assert_eq!(body["items"][0]["source_category"], "Document");
    assert_eq!(body["items"][0]["source_id"], source_id.to_string());
    assert_eq!(body["items"][0]["entity_kind"], "Document");
    assert_eq!(body["items"][0]["title"], "Fixture document");
    assert_eq!(
        body["items"][0]["source_url"],
        "https://example.test/document.pdf"
    );
    assert_eq!(
        body["items"][0]["api_url"],
        format!("/documents/{source_id}")
    );
    assert_eq!(body["items"][0]["snippets"][0]["field"], "extracted_text");
    assert_eq!(
        body["items"][0]["snippets"][0]["highlighted"],
        "plain <em>snippet</em>"
    );
    assert_eq!(body["items"][0]["ranking_score"], 0.98);

    Ok(())
}

#[tokio::test]
async fn search_endpoint_returns_entity_metadata_results() -> Result<(), Box<dyn std::error::Error>>
{
    let source_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222")?;
    let search = FakeSearchClient::with_response(SearchResponse {
        query: "azimullah".to_owned(),
        limit: 20,
        offset: 0,
        estimated_total_hits: Some(1),
        results: vec![SearchResult {
            key: format!("Persoon:{source_id}"),
            source_category: "Persoon".to_owned(),
            source_id,
            entity_kind: SearchEntityKind::Person,
            title: "J. Azimullah".to_owned(),
            summary: None,
            source_url: None,
            date: None,
            document_number: None,
            snippets: Vec::new(),
            ranking_score: None,
        }],
    });

    let body = router_json_with_search(search, "/search?q=azimullah", StatusCode::OK).await?;

    assert_eq!(body["items"][0]["source_category"], "Persoon");
    assert_eq!(body["items"][0]["entity_kind"], "Person");
    assert_eq!(body["items"][0]["api_url"], format!("/persons/{source_id}"));

    Ok(())
}

#[tokio::test]
async fn search_endpoint_rejects_invalid_queries() -> Result<(), Box<dyn std::error::Error>> {
    for path in [
        "/search",
        "/search?q=%20%20",
        "/search?q=fixture&limit=0",
        "/search?q=fixture&limit=101",
    ] {
        let search = FakeSearchClient::with_response(SearchResponse {
            query: String::new(),
            limit: 20,
            offset: 0,
            estimated_total_hits: None,
            results: Vec::new(),
        });
        let body = router_json_with_search(search, path, StatusCode::BAD_REQUEST).await?;
        assert_eq!(body["code"], "invalid_request");
    }

    Ok(())
}

#[tokio::test]
async fn search_endpoint_reports_backend_unavailable() -> Result<(), Box<dyn std::error::Error>> {
    let search = FakeSearchClient::with_error();

    let body =
        router_json_with_search(search, "/search?q=fixture", StatusCode::SERVICE_UNAVAILABLE)
            .await?;

    assert_eq!(body["code"], "search_unavailable");

    Ok(())
}

#[derive(Clone)]
struct FakeSearchClient {
    response: FakeSearchResponse,
    requests: Arc<Mutex<Vec<SearchRequest>>>,
}

#[derive(Clone)]
enum FakeSearchResponse {
    Ok(SearchResponse),
    Unavailable,
}

impl FakeSearchClient {
    fn with_response(response: SearchResponse) -> Self {
        Self {
            response: FakeSearchResponse::Ok(response),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_error() -> Self {
        Self {
            response: FakeSearchResponse::Unavailable,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<SearchRequest> {
        self.requests.lock().expect("requests mutex").clone()
    }
}

impl SearchQueryClient for FakeSearchClient {
    fn search<'a>(
        &'a self,
        request: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchIndexError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests.lock().expect("requests mutex").push(request);
            match &self.response {
                FakeSearchResponse::Ok(response) => Ok(response.clone()),
                FakeSearchResponse::Unavailable => Err(SearchIndexError::Http {
                    status: Some(503),
                    message: "fixture backend error".to_owned(),
                }),
            }
        })
    }
}

async fn router_json_with_search(
    search: FakeSearchClient,
    path: &str,
    expected_status: StatusCode,
) -> Result<Value, Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://opentk.invalid/opentk")?;
    let response = opentk_api::router_with_search(pool, Arc::new(search))
        .oneshot(Request::get(path).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), expected_status);
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX).await?,
    )?)
}
