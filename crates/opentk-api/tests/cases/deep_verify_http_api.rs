use axum::http::StatusCode;
use serde_json::Value;

use crate::support::{
    activity_id, category, document_id, document_source_row, migrated_pool, seed_deep_fixture,
    start_server, third_document_id,
};

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
