use axum::http::StatusCode;
use opentk_db::document_assets::{
    record_document_asset_fetches, record_document_content_extractions,
};
use opentk_sync::{
    document_asset::{
        DocumentAssetFetchConfig, DocumentAssetFetchRequest, DocumentAssetFetcher, RetrievalStatus,
    },
    document_content::{
        DocumentContentExtractionInput, DocumentContentExtractor, DocumentExtractionFixture,
    },
};
use reqwest::Url;
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::{num::NonZeroUsize, time::Duration};
use uuid::Uuid;

use crate::support::{
    activity_id, category, document_id, document_source_row, insert_document_with_asset_metadata,
    migrated_pool, seed_deep_fixture, start_server, third_document_id, FixtureAssetServer,
    FixtureResponse,
};

const HTML_FIXTURE: &[u8] =
    include_bytes!("../../../opentk-sync/tests/fixtures/document_content/fixture.html");
const HTML_EXPECTED_HTML: &str =
    include_str!("../../../opentk-sync/tests/fixtures/document_content/fixture.html.expected.html");
const HTML_EXPECTED_TEXT: &str =
    include_str!("../../../opentk-sync/tests/fixtures/document_content/fixture.html.expected.txt");
const DOCX_FIXTURE: &[u8] =
    include_bytes!("../../../opentk-sync/tests/fixtures/document_content/fixture.docx");
const DOCX_EXPECTED_TEXT: &str =
    include_str!("../../../opentk-sync/tests/fixtures/document_content/fixture.docx.expected.txt");
const PDF_FIXTURE: &[u8] =
    include_bytes!("../../../opentk-sync/tests/fixtures/document_content/fixture.pdf");
const PDF_EXPECTED_TEXT: &str =
    include_str!("../../../opentk-sync/tests/fixtures/document_content/fixture.pdf.expected.txt");

#[tokio::test]
async fn real_http_server_reports_health() -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_health").await?;
    let server = start_server(pool).await?;

    let body = server.get_json("/health", StatusCode::OK).await?;

    assert_eq!(body, serde_json::json!({ "status": "ok" }));
    Ok(())
}

#[tokio::test]
async fn openapi_documents_every_http_route_parameter_and_response_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_openapi").await?;
    let server = start_server(pool).await?;

    let body = server.get_json("/openapi.json", StatusCode::OK).await?;

    for path in [
        "/paths/~1health",
        "/paths/~1openapi.json",
        "/paths/~1categories",
        "/paths/~1sync~1status",
        "/paths/~1changes~1{category}",
        "/paths/~1entities~1{category}~1{source_id}",
        "/paths/~1documents~1{source_id}",
        "/paths/~1activities~1{source_id}",
        "/paths/~1persons~1{source_id}",
        "/paths/~1relations~1{category}~1{source_id}",
    ] {
        assert!(body.pointer(path).is_some(), "missing OpenAPI path {path}");
    }

    assert_parameters(
        &body,
        "/paths/~1changes~1{category}/get",
        &["category", "after", "limit"],
    );
    assert_parameters(
        &body,
        "/paths/~1entities~1{category}~1{source_id}/get",
        &["category", "source_id", "relations"],
    );
    for path in [
        "/paths/~1documents~1{source_id}/get",
        "/paths/~1activities~1{source_id}/get",
        "/paths/~1persons~1{source_id}/get",
    ] {
        assert_parameters(&body, path, &["source_id", "relations"]);
    }
    assert_parameters(
        &body,
        "/paths/~1relations~1{category}~1{source_id}/get",
        &["category", "source_id", "direction"],
    );

    for schema in [
        "HealthResponse",
        "ErrorResponse",
        "CategoryMetadataResponse",
        "SyncStatusResponse",
        "ChangePageResponse",
        "EntityDetailResponse",
        "RelationLookupResponse",
    ] {
        assert!(
            body.pointer(&format!("/components/schemas/{schema}"))
                .is_some(),
            "missing OpenAPI schema {schema}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn real_http_endpoints_return_postgres_backed_bodies(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_bodies").await?;
    seed_deep_fixture(&pool).await?;
    let server = start_server(pool.clone()).await?;

    let categories = server.get_json("/categories", StatusCode::OK).await?;
    let document_category = category(&categories, "Document");
    assert_eq!(document_category["table"], "document");
    assert!(
        document_category["field_count"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );

    let status = server.get_json("/sync/status", StatusCode::OK).await?;
    let document_status = category(&status, "Document");
    assert_eq!(document_status["latest_skiptoken"], 43);
    assert_eq!(document_status["state"], "running");
    assert_eq!(
        document_status["next_url"],
        "https://example.test/document/next"
    );

    let entity = server
        .get_json(
            &format!("/entities/Document/{}", document_id()),
            StatusCode::OK,
        )
        .await?;
    let document = server
        .get_json(&format!("/documents/{}", document_id()), StatusCode::OK)
        .await?;
    let expected_document = document_source_row(&pool, document_id()).await?;
    assert_document_body_matches_row(&entity, &expected_document);
    assert_document_body_matches_row(&document, &expected_document);

    let activity = server
        .get_json(&format!("/activities/{}", activity_id()), StatusCode::OK)
        .await?;
    assert_eq!(activity["metadata"]["category"], "Activiteit");
    assert_eq!(activity["fields"]["soort"], "Debat");
    assert_eq!(activity["fields"]["nummer"], "A-1");

    let relations = server
        .get_json(
            &format!("/relations/Document/{}?direction=outgoing", document_id()),
            StatusCode::OK,
        )
        .await?;
    assert_eq!(relations["items"].as_array().expect("items").len(), 1);
    assert_eq!(relations["items"][0]["target_category"], "Activiteit");
    assert_eq!(
        relations["items"][0]["target_id"],
        activity_id().to_string()
    );

    Ok(())
}

#[tokio::test]
async fn cursor_pagination_covers_first_next_empty_and_invalid_pages(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_pagination").await?;
    seed_deep_fixture(&pool).await?;
    let server = start_server(pool).await?;

    let first = server
        .get_json("/changes/Document?limit=1", StatusCode::OK)
        .await?;
    assert_eq!(first["items"].as_array().expect("items").len(), 1);
    assert_eq!(first["items"][0]["source_id"], document_id().to_string());
    assert_eq!(first["next_skiptoken"], 40);
    assert_eq!(first["has_more"], true);

    let next = server
        .get_json("/changes/Document?after=40&limit=2", StatusCode::OK)
        .await?;
    assert_eq!(next["items"].as_array().expect("items").len(), 2);
    assert_eq!(
        next["items"][1]["source_id"],
        third_document_id().to_string()
    );
    assert_eq!(next["has_more"], false);
    assert_eq!(next["next_skiptoken"], Value::Null);

    let empty = server
        .get_json("/changes/Document?after=43&limit=2", StatusCode::OK)
        .await?;
    assert_eq!(empty["items"].as_array().expect("items").len(), 0);
    assert_eq!(empty["has_more"], false);
    assert_eq!(empty["next_skiptoken"], Value::Null);

    assert_invalid_request(
        &server
            .get_json("/changes/Document?limit=0", StatusCode::BAD_REQUEST)
            .await?,
    );
    assert_invalid_request(
        &server
            .get_json("/changes/Document?limit=501", StatusCode::BAD_REQUEST)
            .await?,
    );
    assert_invalid_request(
        &server
            .get_json("/changes/Document?after=bad", StatusCode::BAD_REQUEST)
            .await?,
    );

    Ok(())
}

#[tokio::test]
async fn error_responses_are_json_for_invalid_ids_categories_and_parameters(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_errors").await?;
    seed_deep_fixture(&pool).await?;
    let server = start_server(pool).await?;

    assert_invalid_request(
        &server
            .get_json("/entities/Document/not-a-uuid", StatusCode::BAD_REQUEST)
            .await?,
    );
    assert_not_found(
        &server
            .get_json("/changes/Nope?limit=1", StatusCode::NOT_FOUND)
            .await?,
    );
    assert_not_found(
        &server
            .get_json(
                "/entities/Document/99999999-9999-4999-8999-999999999999",
                StatusCode::NOT_FOUND,
            )
            .await?,
    );
    assert_invalid_request(
        &server
            .get_json(
                &format!("/entities/Document/{}?relations=sideways", document_id()),
                StatusCode::BAD_REQUEST,
            )
            .await?,
    );
    assert_invalid_request(
        &server
            .get_json(
                &format!("/relations/Document/{}?direction=sideways", document_id()),
                StatusCode::BAD_REQUEST,
            )
            .await?,
    );

    Ok(())
}

#[tokio::test]
async fn relation_expansion_supports_none_outgoing_incoming_and_both(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_relation_expansion").await?;
    seed_deep_fixture(&pool).await?;
    let server = start_server(pool).await?;

    let none = server
        .get_json(
            &format!("/entities/Document/{}?relations=none", document_id()),
            StatusCode::OK,
        )
        .await?;
    assert!(none.get("relations").is_none());

    let outgoing = server
        .get_json(
            &format!("/entities/Document/{}?relations=outgoing", document_id()),
            StatusCode::OK,
        )
        .await?;
    assert_eq!(
        outgoing["relations"].as_array().expect("relations").len(),
        1
    );
    assert_eq!(outgoing["relations"][0]["target_category"], "Activiteit");

    let incoming = server
        .get_json(
            &format!("/entities/Document/{}?relations=incoming", document_id()),
            StatusCode::OK,
        )
        .await?;
    assert_eq!(
        incoming["relations"].as_array().expect("relations").len(),
        1
    );
    assert_eq!(
        incoming["relations"][0]["source_category"],
        "DocumentVersie"
    );

    let both = server
        .get_json(
            &format!("/entities/Document/{}?relations=both", document_id()),
            StatusCode::OK,
        )
        .await?;
    assert_eq!(both["relations"].as_array().expect("relations").len(), 2);

    Ok(())
}

#[tokio::test]
async fn server_handles_concurrent_representative_reads() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = migrated_pool("deep_concurrency").await?;
    seed_deep_fixture(&pool).await?;
    let server = start_server(pool).await?;
    let document_path = format!("/documents/{}", document_id());
    let relation_path = format!("/relations/Document/{}?direction=outgoing", document_id());

    let (health, categories, changes, document, relations) = tokio::try_join!(
        server.get_json("/health", StatusCode::OK),
        server.get_json("/categories", StatusCode::OK),
        server.get_json("/changes/Document?limit=1", StatusCode::OK),
        server.get_json(&document_path, StatusCode::OK),
        server.get_json(&relation_path, StatusCode::OK),
    )?;

    assert_eq!(health["status"], "ok");
    assert!(
        category(&categories, "Document")["field_count"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
    assert_eq!(changes["items"].as_array().expect("items").len(), 1);
    assert_eq!(document["metadata"]["source_id"], document_id().to_string());
    assert_eq!(relations["items"].as_array().expect("items").len(), 1);

    Ok(())
}

#[tokio::test]
async fn official_text_source_wins_over_binary_fallback_and_is_served_exactly(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_content_official").await?;
    let document_id = fixture_document_id(1);
    let landing = r#"<html><head><link rel="alternate" type="text/plain" href="/official.txt"></head><body><a href="/fallback.pdf">PDF</a></body></html>"#;
    let official_text = "Official fixture text.\n\nThis body must win.";
    let asset_server = FixtureAssetServer::start(vec![
        FixtureResponse::ok("/landing", "text/html", landing),
        FixtureResponse::ok("/official.txt", "text/plain", official_text),
    ])
    .await?;
    let asset_url = Url::parse(&format!("{}/landing", asset_server.base_url))?;
    insert_document_with_asset_metadata(
        &pool,
        document_id,
        60,
        "text/html",
        i32::try_from(landing.len())?,
        asset_url.as_str(),
    )
    .await?;

    let response = fetch_extract_store_and_serve(
        &pool,
        document_id,
        asset_url,
        Some("text/html"),
        Some(official_text),
        None,
    )
    .await?;

    assert_eq!(response["content"]["official_source"], true);
    assert_eq!(response["content"]["source_rank"], 0);
    assert_eq!(response["content"]["extracted_text"], official_text);
    assert_eq!(response["content"]["extracted_html"], Value::Null);
    assert!(!response["content"]["selected_source_url"]
        .as_str()
        .expect("selected url")
        .ends_with("fallback.pdf"));
    asset_server.assert_consumed().await;
    Ok(())
}

#[tokio::test]
async fn pdf_fixture_is_fetched_extracted_stored_and_served_exactly(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_content_pdf").await?;
    let document_id = fixture_document_id(2);
    let asset_server = FixtureAssetServer::start(vec![FixtureResponse::ok(
        "/fixture.pdf",
        "application/pdf",
        PDF_FIXTURE,
    )])
    .await?;
    let asset_url = Url::parse(&format!("{}/fixture.pdf", asset_server.base_url))?;
    insert_document_with_asset_metadata(
        &pool,
        document_id,
        61,
        "application/pdf",
        i32::try_from(PDF_FIXTURE.len())?,
        asset_url.as_str(),
    )
    .await?;

    let response = fetch_extract_store_and_serve(
        &pool,
        document_id,
        asset_url,
        Some("application/pdf"),
        Some(expected(PDF_EXPECTED_TEXT)),
        None,
    )
    .await?;

    assert_eq!(response["content"]["official_source"], false);
    assert_eq!(response["content"]["source_rank"], 10);
    assert_eq!(
        response["content"]["extracted_text"],
        expected(PDF_EXPECTED_TEXT)
    );
    assert_eq!(response["content"]["extracted_html"], Value::Null);
    asset_server.assert_consumed().await;
    Ok(())
}

#[tokio::test]
async fn docx_fixture_is_fetched_extracted_stored_and_served_exactly(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_content_docx").await?;
    let document_id = fixture_document_id(3);
    let content_type = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
    let asset_server = FixtureAssetServer::start(vec![FixtureResponse::ok(
        "/fixture.docx",
        content_type,
        DOCX_FIXTURE,
    )])
    .await?;
    let asset_url = Url::parse(&format!("{}/fixture.docx", asset_server.base_url))?;
    insert_document_with_asset_metadata(
        &pool,
        document_id,
        62,
        content_type,
        i32::try_from(DOCX_FIXTURE.len())?,
        asset_url.as_str(),
    )
    .await?;

    let response = fetch_extract_store_and_serve(
        &pool,
        document_id,
        asset_url,
        Some(content_type),
        Some(expected(DOCX_EXPECTED_TEXT)),
        None,
    )
    .await?;

    assert_eq!(response["content"]["official_source"], false);
    assert_eq!(
        response["content"]["extracted_text"],
        expected(DOCX_EXPECTED_TEXT)
    );
    assert_eq!(response["content"]["extracted_html"], Value::Null);
    asset_server.assert_consumed().await;
    Ok(())
}

#[tokio::test]
async fn html_fixture_is_fetched_stored_and_served_exactly(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_content_html").await?;
    let document_id = fixture_document_id(4);
    let asset_server = FixtureAssetServer::start(vec![FixtureResponse::ok(
        "/fixture.html",
        "text/html",
        HTML_FIXTURE,
    )])
    .await?;
    let asset_url = Url::parse(&format!("{}/fixture.html", asset_server.base_url))?;
    insert_document_with_asset_metadata(
        &pool,
        document_id,
        63,
        "text/html",
        i32::try_from(HTML_FIXTURE.len())?,
        asset_url.as_str(),
    )
    .await?;

    let response = fetch_extract_store_and_serve(
        &pool,
        document_id,
        asset_url,
        Some("text/html"),
        Some(expected(HTML_EXPECTED_TEXT)),
        Some(HTML_EXPECTED_HTML),
    )
    .await?;

    assert_eq!(response["content"]["official_source"], true);
    assert_eq!(
        response["content"]["extracted_text"],
        expected(HTML_EXPECTED_TEXT)
    );
    assert_eq!(response["content"]["extracted_html"], HTML_EXPECTED_HTML);
    asset_server.assert_consumed().await;
    Ok(())
}

#[tokio::test]
async fn retry_exhaustion_persists_failed_asset_without_fake_content(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("deep_content_retry").await?;
    let document_id = fixture_document_id(5);
    let asset_server = FixtureAssetServer::start(vec![
        FixtureResponse::status("/busy.pdf", 503, "busy"),
        FixtureResponse::status("/busy.pdf", 503, "still busy"),
    ])
    .await?;
    let asset_url = Url::parse(&format!("{}/busy.pdf", asset_server.base_url))?;
    insert_document_with_asset_metadata(
        &pool,
        document_id,
        64,
        "application/pdf",
        4,
        asset_url.as_str(),
    )
    .await?;

    let mut config = fetch_config();
    config.max_retries = 1;
    let fetcher = DocumentAssetFetcher::new(config)?;
    let report = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id,
            asset_url,
            expected_content_type: Some("application/pdf".to_owned()),
            expected_content_length: None,
        })
        .await;
    assert_eq!(report.retrieval_status, RetrievalStatus::Failed);
    record_document_asset_fetches(&pool, &[report]).await?;

    let content_rows: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM document_content WHERE document_source_id = $1",
    )
    .bind(document_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(content_rows, 0);
    let asset_error: Option<String> = sqlx::query_scalar(
        "SELECT retrieval_error FROM document_asset WHERE document_source_id = $1",
    )
    .bind(document_id)
    .fetch_one(&pool)
    .await?;
    assert!(
        asset_error
            .as_deref()
            .is_some_and(|error| error.contains("503")),
        "retry exhaustion must preserve the HTTP error, got {asset_error:?}"
    );

    let api = start_server(pool).await?;
    let body = api
        .get_json(
            &format!("/documents/{document_id}/content"),
            StatusCode::NOT_FOUND,
        )
        .await?;
    assert_eq!(body["code"], "document_content_not_found");
    assert_eq!(asset_server.requests().await.len(), 2);
    Ok(())
}

fn assert_parameters(body: &Value, operation_pointer: &str, expected: &[&str]) {
    let parameters = body
        .pointer(&format!("{operation_pointer}/parameters"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing parameters for {operation_pointer}"));
    for name in expected {
        assert!(
            parameters
                .iter()
                .any(|parameter| parameter["name"] == *name),
            "missing parameter {name} for {operation_pointer}"
        );
    }
}

fn assert_document_body_matches_row(body: &Value, expected: &Value) {
    for field in [
        "category",
        "source_id",
        "latest_skiptoken",
        "deleted",
        "source_updated_at",
        "atom_updated_at",
    ] {
        assert_eq!(body["metadata"][field], expected["metadata"][field]);
    }
    for field in [
        "content_type",
        "content_length",
        "enclosure_url",
        "document_nummer",
        "titel",
    ] {
        assert_eq!(body["fields"][field], expected["fields"][field]);
    }
}

fn assert_invalid_request(body: &Value) {
    assert_eq!(body["code"], "invalid_request");
    assert_eq!(body["message"], "invalid request");
}

fn assert_not_found(body: &Value) {
    assert_eq!(body["code"], "not_found");
    assert_eq!(body["message"], "resource not found");
}

async fn fetch_extract_store_and_serve(
    pool: &PgPool,
    document_id: Uuid,
    asset_url: Url,
    expected_content_type: Option<&str>,
    expected_text: Option<&str>,
    expected_html: Option<&str>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let fetcher = DocumentAssetFetcher::new(fetch_config())?;
    let fetch_report = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id,
            asset_url,
            expected_content_type: expected_content_type.map(str::to_owned),
            expected_content_length: None,
        })
        .await;
    assert_eq!(fetch_report.retrieval_status, RetrievalStatus::Fetched);
    record_document_asset_fetches(pool, std::slice::from_ref(&fetch_report)).await?;
    let document_asset_id: i64 = sqlx::query_scalar(
        "SELECT id FROM document_asset WHERE document_source_id = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(document_id)
    .fetch_one(pool)
    .await?;
    let source_hash = fetch_report
        .source_hash
        .clone()
        .expect("fetched report has source hash");
    let input =
        DocumentContentExtractionInput::from_fetch_report(document_asset_id, &fetch_report)?;
    let extractor = DocumentContentExtractor::new(vec![DocumentExtractionFixture {
        source_hash,
        expected_text: expected_text.map(str::to_owned),
        expected_html: expected_html.map(str::to_owned),
    }]);
    let extraction = extractor.extract(input);
    assert_eq!(
        extraction.validation_status,
        opentk_sync::document_content::ValidationStatus::Valid,
        "{extraction:?}"
    );
    record_document_content_extractions(pool, &[extraction]).await?;

    let row = stored_content_row(pool, document_id).await?;
    assert_stored_content_matches_expected(&row, expected_text, expected_html);
    let api = start_server(pool.clone()).await?;
    let body = api
        .get_json(&format!("/documents/{document_id}/content"), StatusCode::OK)
        .await?;
    assert_eq!(body["document"]["source_category"], "Document");
    assert_eq!(body["document"]["source_id"], document_id.to_string());
    assert_eq!(
        body["asset"]["retrieval_status"],
        row.get::<String, _>("retrieval_status")
    );
    assert_api_content_matches_row(&body, &row);
    Ok(body)
}

async fn stored_content_row(
    pool: &PgPool,
    document_id: Uuid,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    sqlx::query(
        "SELECT c.selected_source_url, c.selected_source_content_type,
                c.official_source, c.source_rank, c.extraction_status,
                c.validation_status, c.extraction_tool, c.extraction_tool_version,
                c.source_hash, c.output_hash, c.extraction_error,
                c.extracted_text, c.extracted_html, a.retrieval_status
         FROM document_content c
         JOIN document_asset a ON a.id = c.document_asset_id
         WHERE c.document_source_id = $1
         ORDER BY c.validation_status = 'valid' DESC, c.id DESC
         LIMIT 1",
    )
    .bind(document_id)
    .fetch_one(pool)
    .await
}

fn assert_stored_content_matches_expected(
    row: &sqlx::postgres::PgRow,
    expected_text: Option<&str>,
    expected_html: Option<&str>,
) {
    assert_eq!(row.get::<String, _>("retrieval_status"), "fetched");
    assert_eq!(row.get::<String, _>("extraction_status"), "extracted");
    assert_eq!(row.get::<String, _>("validation_status"), "valid");
    assert_eq!(row.get::<Option<String>, _>("extraction_error"), None);
    assert_eq!(
        row.get::<Option<String>, _>("extracted_text").as_deref(),
        expected_text
    );
    assert_eq!(
        row.get::<Option<String>, _>("extracted_html").as_deref(),
        expected_html
    );
}

fn assert_api_content_matches_row(body: &Value, row: &sqlx::postgres::PgRow) {
    for key in [
        "selected_source_url",
        "selected_source_content_type",
        "official_source",
        "source_rank",
        "extraction_status",
        "validation_status",
        "extraction_tool",
        "extraction_tool_version",
        "source_hash",
        "output_hash",
        "extraction_error",
        "extracted_text",
        "extracted_html",
    ] {
        let api_value = &body["content"][key];
        let row_value = match key {
            "official_source" => Value::Bool(row.get(key)),
            "source_rank" => Value::from(row.get::<i32, _>(key)),
            _ => row
                .get::<Option<String>, _>(key)
                .map_or(Value::Null, Value::String),
        };
        assert_eq!(
            *api_value, row_value,
            "API field {key} must match PostgreSQL"
        );
    }
}

fn fetch_config() -> DocumentAssetFetchConfig {
    DocumentAssetFetchConfig {
        request_timeout: Duration::from_secs(2),
        connect_timeout: Duration::from_secs(2),
        max_retries: 0,
        initial_retry_delay: Duration::from_millis(1),
        max_retry_delay: Duration::from_millis(1),
        max_concurrent_requests: NonZeroUsize::new(4).expect("non-zero"),
        max_asset_bytes: 1024 * 1024,
    }
}

fn fixture_document_id(index: u128) -> Uuid {
    Uuid::from_u128(0x55555555_5555_4555_8555_000000000000 + index)
}

fn expected(value: &str) -> &str {
    value.trim_end_matches('\n')
}
