use axum::http::StatusCode;
use serde_json::Value;
use sqlx::PgPool;

use crate::support::{
    activity_id, category, document_id, insert_activity, insert_document, insert_person,
    insert_sync_entity, migrated_pool, person_id, router_json, second_document_id,
    third_document_id,
};

#[tokio::test]
async fn categories_returns_schema_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_categories").await?;

    let body = router_json(pool, "/categories", StatusCode::OK).await?;
    let categories = body["categories"]
        .as_array()
        .expect("categories is an array");

    assert!(categories.iter().any(|category| {
        category["category"] == "Document"
            && category["table"] == "document"
            && category["field_count"].as_u64().unwrap_or_default() > 0
            && category["relation_count"].as_u64().unwrap_or_default() > 0
    }));
    assert!(categories
        .iter()
        .any(|category| category["category"] == "Activiteit" && category["table"] == "activiteit"));
    assert!(categories
        .iter()
        .any(|category| category["category"] == "Persoon" && category["table"] == "persoon"));

    Ok(())
}

#[tokio::test]
async fn sync_status_returns_persisted_category_progress() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = migrated_pool("api_sync_status").await?;
    sqlx::query(
        "INSERT INTO sync_category
         (source_category, latest_skiptoken, state, last_fetch_at, last_synced_at, caught_up_at, next_url, resume_url)
         VALUES
         ('Document', 10, 'running', '2026-04-26T10:00:00Z', '2026-04-26T10:01:00Z', NULL, 'https://example.test/next', 'https://example.test/resume'),
         ('Persoon', 20, 'caught_up', '2026-04-26T11:00:00Z', '2026-04-26T11:01:00Z', '2026-04-26T11:02:00Z', NULL, NULL)",
    )
    .execute(&pool)
    .await?;

    let body = router_json(pool, "/sync/status", StatusCode::OK).await?;
    let document = category(&body, "Document");
    assert_eq!(document["latest_skiptoken"], 10);
    assert_eq!(document["state"], "running");
    assert_eq!(document["next_url"], "https://example.test/next");
    let person = category(&body, "Persoon");
    assert_eq!(person["latest_skiptoken"], 20);
    assert_eq!(person["state"], "caught_up");

    Ok(())
}

#[tokio::test]
async fn changes_page_by_category_and_skiptoken() -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_changes").await?;
    insert_sync_entity(&pool, "Document", document_id(), 10).await?;
    insert_sync_entity(&pool, "Document", second_document_id(), 11).await?;
    insert_sync_entity(&pool, "Document", third_document_id(), 12).await?;
    insert_sync_entity(&pool, "Persoon", person_id(), 13).await?;

    let body = router_json(
        pool.clone(),
        "/changes/Document?after=10&limit=2",
        StatusCode::OK,
    )
    .await?;
    assert_eq!(body["category"], "Document");
    assert_eq!(body["items"].as_array().expect("items").len(), 2);
    assert_eq!(
        body["items"][0]["source_id"],
        second_document_id().to_string()
    );
    assert_eq!(body["items"][0]["latest_skiptoken"], 11);
    assert_eq!(
        body["items"][1]["source_id"],
        third_document_id().to_string()
    );
    assert_eq!(body["has_more"], false);
    assert_eq!(body["next_skiptoken"], Value::Null);

    let error = router_json(pool, "/changes/Nope", StatusCode::NOT_FOUND).await?;
    assert_eq!(error["code"], "not_found");

    Ok(())
}

#[tokio::test]
async fn entity_and_typed_details_return_database_rows() -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_details").await?;
    insert_document(&pool).await?;
    insert_activity(&pool).await?;
    insert_person(&pool).await?;

    let entity = router_json(
        pool.clone(),
        &format!("/entities/Document/{}", document_id()),
        StatusCode::OK,
    )
    .await?;
    assert_eq!(entity["metadata"]["category"], "Document");
    assert_eq!(entity["fields"]["document_nummer"], "2026D00001");
    assert_eq!(entity["fields"]["titel"], "Fixture document");
    assert!(entity.get("relations").is_none());

    let document = router_json(
        pool.clone(),
        &format!("/documents/{}", document_id()),
        StatusCode::OK,
    )
    .await?;
    assert_eq!(document["fields"]["content_type"], "application/pdf");
    assert_eq!(document["fields"]["content_length"], 12345);
    assert_eq!(
        document["fields"]["enclosure_url"],
        "https://example.test/document.pdf"
    );

    let activity = router_json(
        pool.clone(),
        &format!("/activities/{}", activity_id()),
        StatusCode::OK,
    )
    .await?;
    assert_eq!(activity["fields"]["soort"], "Debat");
    assert_eq!(activity["fields"]["nummer"], "A-1");
    assert_eq!(activity["fields"]["locatie"], "Plenaire zaal");

    let person = router_json(pool, &format!("/persons/{}", person_id()), StatusCode::OK).await?;
    assert_eq!(person["fields"]["nummer"], "P-1");
    assert_eq!(person["fields"]["achternaam"], "Jansen");
    assert_eq!(person["fields"]["roepnaam"], "Jan");

    Ok(())
}

#[tokio::test]
async fn document_content_returns_extracted_text_and_source_metadata(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_document_content_text").await?;
    insert_document(&pool).await?;
    insert_document_content(
        &pool,
        DocumentContentSeed {
            asset_url: "https://example.test/document.pdf",
            upstream_content_type: Some("application/pdf"),
            upstream_content_length: Some(12345),
            selected_source_url: "https://example.test/document.pdf",
            selected_source_content_type: Some("application/pdf"),
            selected_source_content_length: Some(12345),
            official_source: false,
            source_rank: 20,
            extraction_status: "extracted",
            validation_status: "valid",
            extraction_tool: "pdf-extract",
            extraction_tool_version: "0.10.0",
            source_hash: "sha256-source",
            output_hash: Some("sha256-output"),
            extraction_error: None,
            extracted_text: Some("Stored document text"),
            extracted_html: None,
        },
    )
    .await?;

    let body = router_json(
        pool,
        &format!("/documents/{}/content", document_id()),
        StatusCode::OK,
    )
    .await?;

    assert_eq!(body["document"]["source_category"], "Document");
    assert_eq!(body["document"]["source_id"], document_id().to_string());
    assert_eq!(
        body["asset"]["asset_url"],
        "https://example.test/document.pdf"
    );
    assert_eq!(
        body["asset"]["upstream_url"],
        "https://example.test/document.pdf"
    );
    assert_eq!(body["asset"]["upstream_content_type"], "application/pdf");
    assert_eq!(body["asset"]["upstream_content_length"], 12345);
    assert_eq!(body["asset"]["retrieval_status"], "fetched");
    assert_eq!(body["asset"]["retrieval_error"], Value::Null);
    assert_eq!(body["asset"]["retrieved_at"], "2026-04-26T12:02:00+00:00");
    assert_eq!(
        body["content"]["selected_source_url"],
        "https://example.test/document.pdf"
    );
    assert_eq!(
        body["content"]["selected_source_content_type"],
        "application/pdf"
    );
    assert_eq!(body["content"]["selected_source_content_length"], 12345);
    assert_eq!(body["content"]["official_source"], false);
    assert_eq!(body["content"]["source_rank"], 20);
    assert_eq!(body["content"]["extraction_status"], "extracted");
    assert_eq!(body["content"]["validation_status"], "valid");
    assert_eq!(body["content"]["extraction_tool"], "pdf-extract");
    assert_eq!(body["content"]["extraction_tool_version"], "0.10.0");
    assert_eq!(body["content"]["source_hash"], "sha256-source");
    assert_eq!(body["content"]["output_hash"], "sha256-output");
    assert_eq!(body["content"]["extraction_error"], Value::Null);
    assert_eq!(body["content"]["extracted_text"], "Stored document text");
    assert_eq!(body["content"]["extracted_html"], Value::Null);
    assert_eq!(body["content"]["extracted_at"], "2026-04-26T12:03:00+00:00");

    Ok(())
}

#[tokio::test]
async fn document_content_returns_extracted_html_for_official_html_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_document_content_html").await?;
    insert_document(&pool).await?;
    insert_document_content(
        &pool,
        DocumentContentSeed {
            asset_url: "https://example.test/document.html",
            upstream_content_type: Some("text/html"),
            upstream_content_length: Some(82),
            selected_source_url: "https://example.test/document.html",
            selected_source_content_type: Some("text/html"),
            selected_source_content_length: Some(82),
            official_source: true,
            source_rank: 0,
            extraction_status: "extracted",
            validation_status: "valid",
            extraction_tool: "official-source",
            extraction_tool_version: "1",
            source_hash: "sha256-html-source",
            output_hash: Some("sha256-html-output"),
            extraction_error: None,
            extracted_text: Some("Extracted HTML text"),
            extracted_html: Some("<main><p>Extracted HTML text</p></main>"),
        },
    )
    .await?;

    let body = router_json(
        pool,
        &format!("/documents/{}/content", document_id()),
        StatusCode::OK,
    )
    .await?;

    assert_eq!(body["asset"]["upstream_content_type"], "text/html");
    assert_eq!(body["content"]["selected_source_content_type"], "text/html");
    assert_eq!(body["content"]["official_source"], true);
    assert_eq!(body["content"]["source_rank"], 0);
    assert_eq!(body["content"]["extraction_tool"], "official-source");
    assert_eq!(body["content"]["extracted_text"], "Extracted HTML text");
    assert_eq!(
        body["content"]["extracted_html"],
        "<main><p>Extracted HTML text</p></main>"
    );

    Ok(())
}

#[tokio::test]
async fn document_content_returns_failed_extraction_as_durable_content_state(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_document_content_failed").await?;
    insert_document(&pool).await?;
    insert_document_content(
        &pool,
        DocumentContentSeed {
            asset_url: "https://example.test/document.pdf",
            upstream_content_type: Some("application/pdf"),
            upstream_content_length: Some(12345),
            selected_source_url: "https://example.test/document.pdf",
            selected_source_content_type: Some("application/pdf"),
            selected_source_content_length: Some(12345),
            official_source: false,
            source_rank: 20,
            extraction_status: "failed",
            validation_status: "invalid",
            extraction_tool: "pdf-extract",
            extraction_tool_version: "0.10.0",
            source_hash: "sha256-failed-source",
            output_hash: None,
            extraction_error: Some("unsupported PDF encryption"),
            extracted_text: None,
            extracted_html: None,
        },
    )
    .await?;

    let body = router_json(
        pool,
        &format!("/documents/{}/content", document_id()),
        StatusCode::OK,
    )
    .await?;

    assert_eq!(body["content"]["extraction_status"], "failed");
    assert_eq!(body["content"]["validation_status"], "invalid");
    assert_eq!(body["content"]["source_hash"], "sha256-failed-source");
    assert_eq!(body["content"]["output_hash"], Value::Null);
    assert_eq!(
        body["content"]["extraction_error"],
        "unsupported PDF encryption"
    );
    assert_eq!(body["content"]["extracted_text"], Value::Null);
    assert_eq!(body["content"]["extracted_html"], Value::Null);

    Ok(())
}

#[tokio::test]
async fn document_content_distinguishes_missing_content_from_missing_document(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_document_content_missing").await?;
    insert_document(&pool).await?;

    let missing_content = router_json(
        pool.clone(),
        &format!("/documents/{}/content", document_id()),
        StatusCode::NOT_FOUND,
    )
    .await?;
    assert_eq!(missing_content["code"], "document_content_not_found");
    assert_eq!(missing_content["message"], "document content not found");

    let missing_document = router_json(
        pool,
        "/documents/99999999-9999-4999-8999-999999999999/content",
        StatusCode::NOT_FOUND,
    )
    .await?;
    assert_eq!(missing_document["code"], "not_found");
    assert_eq!(missing_document["message"], "resource not found");

    Ok(())
}

#[tokio::test]
async fn relations_support_direction_and_detail_expansion() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = migrated_pool("api_relations").await?;
    insert_document(&pool).await?;
    insert_activity(&pool).await?;
    sqlx::query(
        "INSERT INTO document__activiteit
         (source_category, source_id, relation_name, target_category, target_id, ordinal, source_updated_at)
         VALUES ('Document', $1, 'activiteit', 'Activiteit', $2, 0, '2026-04-26T12:02:00Z')",
    )
    .bind(document_id())
    .bind(activity_id())
    .execute(&pool)
    .await?;

    let outgoing = router_json(
        pool.clone(),
        &format!("/relations/Document/{}?direction=outgoing", document_id()),
        StatusCode::OK,
    )
    .await?;
    assert_eq!(outgoing["items"].as_array().expect("items").len(), 1);
    assert_eq!(outgoing["items"][0]["target_category"], "Activiteit");
    assert_eq!(outgoing["items"][0]["target_id"], activity_id().to_string());

    let incoming = router_json(
        pool.clone(),
        &format!("/relations/Activiteit/{}?direction=incoming", activity_id()),
        StatusCode::OK,
    )
    .await?;
    assert_eq!(incoming["items"].as_array().expect("items").len(), 1);
    assert_eq!(incoming["items"][0]["source_category"], "Document");
    assert_eq!(incoming["items"][0]["source_id"], document_id().to_string());

    let expanded = router_json(
        pool.clone(),
        &format!("/entities/Document/{}?relations=outgoing", document_id()),
        StatusCode::OK,
    )
    .await?;
    assert_eq!(
        expanded["relations"].as_array().expect("relations").len(),
        1
    );

    let unexpanded = router_json(
        pool,
        &format!("/entities/Document/{}?relations=none", document_id()),
        StatusCode::OK,
    )
    .await?;
    assert!(unexpanded.get("relations").is_none());

    Ok(())
}

struct DocumentContentSeed {
    asset_url: &'static str,
    upstream_content_type: Option<&'static str>,
    upstream_content_length: Option<i64>,
    selected_source_url: &'static str,
    selected_source_content_type: Option<&'static str>,
    selected_source_content_length: Option<i64>,
    official_source: bool,
    source_rank: i32,
    extraction_status: &'static str,
    validation_status: &'static str,
    extraction_tool: &'static str,
    extraction_tool_version: &'static str,
    source_hash: &'static str,
    output_hash: Option<&'static str>,
    extraction_error: Option<&'static str>,
    extracted_text: Option<&'static str>,
    extracted_html: Option<&'static str>,
}

async fn insert_document_content(
    pool: &PgPool,
    seed: DocumentContentSeed,
) -> Result<(), sqlx::Error> {
    let asset_id: i64 = sqlx::query_scalar(
        "INSERT INTO document_asset
         (document_source_category, document_source_id, asset_url, upstream_url,
          upstream_content_type, upstream_content_length, upstream_last_modified_at,
          retrieval_status, retrieval_error, retrieved_at, created_at, updated_at)
         VALUES
         ('Document', $1, $2, $2, $3, $4, NULL, 'fetched', NULL,
          '2026-04-26T12:02:00Z', '2026-04-26T12:02:00Z', '2026-04-26T12:02:00Z')
         RETURNING id",
    )
    .bind(document_id())
    .bind(seed.asset_url)
    .bind(seed.upstream_content_type)
    .bind(seed.upstream_content_length)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO document_content
         (document_asset_id, document_source_category, document_source_id,
          selected_source_url, selected_source_content_type, selected_source_content_length,
          official_source, source_rank, extraction_status, validation_status,
          extraction_tool, extraction_tool_version, source_hash, output_hash,
          extraction_error, extracted_text, extracted_html, extracted_at, created_at, updated_at)
         VALUES
         ($1, 'Document', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
          $14, $15, $16, '2026-04-26T12:03:00Z', '2026-04-26T12:03:00Z',
          '2026-04-26T12:03:00Z')",
    )
    .bind(asset_id)
    .bind(document_id())
    .bind(seed.selected_source_url)
    .bind(seed.selected_source_content_type)
    .bind(seed.selected_source_content_length)
    .bind(seed.official_source)
    .bind(seed.source_rank)
    .bind(seed.extraction_status)
    .bind(seed.validation_status)
    .bind(seed.extraction_tool)
    .bind(seed.extraction_tool_version)
    .bind(seed.source_hash)
    .bind(seed.output_hash)
    .bind(seed.extraction_error)
    .bind(seed.extracted_text)
    .bind(seed.extracted_html)
    .execute(pool)
    .await?;

    Ok(())
}

#[tokio::test]
async fn openapi_includes_core_read_endpoints() -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("api_openapi").await?;
    let body = router_json(pool, "/openapi.json", StatusCode::OK).await?;

    for path in [
        "/paths/~1categories",
        "/paths/~1sync~1status",
        "/paths/~1changes~1{category}",
        "/paths/~1entities~1{category}~1{source_id}",
        "/paths/~1documents~1{source_id}",
        "/paths/~1documents~1{source_id}~1content",
        "/paths/~1activities~1{source_id}",
        "/paths/~1persons~1{source_id}",
        "/paths/~1relations~1{category}~1{source_id}",
    ] {
        assert!(body.pointer(path).is_some(), "missing OpenAPI path {path}");
    }
    for schema in [
        "CategoryMetadataResponse",
        "SyncStatusResponse",
        "ChangePageResponse",
        "EntityDetailResponse",
        "DocumentContentResponse",
        "DocumentAssetResponse",
        "DocumentContentBodyResponse",
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
