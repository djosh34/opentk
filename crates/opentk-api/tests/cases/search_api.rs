use std::{
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opentk_db::search_cdc::SearchCdcRuntimeStatus;
use opentk_search::{
    SearchCountClient, SearchEntityKind, SearchFilter, SearchHealthClient, SearchIndexError,
    SearchQueryClient, SearchRequest, SearchResponse, SearchResult, SearchRuntimeClient,
    SearchSnippet,
};
use opentk_search_eval::{load_quality_benchmark, BenchmarkSearchClient};
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

#[tokio::test]
async fn search_endpoint_reports_search_sync_degraded_without_calling_backend(
) -> Result<(), Box<dyn std::error::Error>> {
    let search = FakeSearchClient::with_response(SearchResponse {
        query: "fixture".to_owned(),
        limit: 20,
        offset: 0,
        estimated_total_hits: None,
        results: Vec::new(),
    });
    let status = SearchCdcRuntimeStatus::new();
    status.record_error(&"fixture CDC failure");

    let body = router_json_with_search_client_and_status(
        Arc::new(search.clone()),
        status,
        "/search?q=fixture",
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await?;

    assert_eq!(body["code"], "search_sync_degraded");
    assert_eq!(search.requests(), Vec::new());

    Ok(())
}

#[tokio::test]
async fn search_endpoint_matches_deep_quality_benchmark_queries(
) -> Result<(), Box<dyn std::error::Error>> {
    let benchmark = load_quality_benchmark(Path::new("../opentk-search-eval/fixtures"))?;
    let search = Arc::new(BenchmarkSearchClient::new(&benchmark.input));

    for (path, expected_top) in [
        ("/search?q=35567-12", "doc-2024-35567"),
        (
            "/search?q=Fatma%20Kaija%20digitalisering",
            "person-fatima-kaya",
        ),
        (
            "/search?q=commissie%20EZK%20technische%20briefing%20netcongestie",
            "dossier-energie-2030",
        ),
    ] {
        let body = router_json_with_search_client(search.clone(), path, StatusCode::OK).await?;

        assert_eq!(
            body["items"][0]["key"],
            expected_top_key(expected_top, &benchmark)
        );
        assert!(body["items"][0]["snippets"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
    }

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

impl SearchHealthClient for FakeSearchClient {
    fn health<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<(), SearchIndexError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
}

impl SearchCountClient for FakeSearchClient {
    fn count<'a>(
        &'a self,
        _filter: SearchFilter,
    ) -> Pin<Box<dyn Future<Output = Result<u64, SearchIndexError>> + Send + 'a>> {
        Box::pin(async { Ok(0) })
    }
}

async fn router_json_with_search(
    search: FakeSearchClient,
    path: &str,
    expected_status: StatusCode,
) -> Result<Value, Box<dyn std::error::Error>> {
    router_json_with_search_client(Arc::new(search), path, expected_status).await
}

async fn router_json_with_search_client(
    search: Arc<dyn SearchRuntimeClient + Send + Sync>,
    path: &str,
    expected_status: StatusCode,
) -> Result<Value, Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://opentk.invalid/opentk")?;
    let response = opentk_api::router_with_search(pool, search)
        .oneshot(Request::get(path).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), expected_status);
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX).await?,
    )?)
}

async fn router_json_with_search_client_and_status(
    search: Arc<dyn SearchRuntimeClient + Send + Sync>,
    status: SearchCdcRuntimeStatus,
    path: &str,
    expected_status: StatusCode,
) -> Result<Value, Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://opentk.invalid/opentk")?;
    let response = opentk_api::router_with_search_and_cdc_status(
        pool,
        search,
        opentk_db::search_sync::SearchSyncConfig::default(),
        status,
    )
    .oneshot(Request::get(path).body(Body::empty())?)
    .await?;
    assert_eq!(response.status(), expected_status);
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX).await?,
    )?)
}

fn expected_top_key(
    expected_top: &str,
    benchmark: &opentk_search_eval::SearchQualityBenchmark,
) -> String {
    let document = benchmark
        .input
        .corpus
        .iter()
        .find(|document| document.id == expected_top)
        .expect("expected benchmark document");
    format!("{}:{}", document.source_category, document.source_id)
}
