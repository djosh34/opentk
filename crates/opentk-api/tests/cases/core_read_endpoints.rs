use axum::http::StatusCode;
use serde_json::Value;

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
