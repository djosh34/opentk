use std::time::Duration;

use chrono::{DateTime, Utc};
use opentk_db::{
    schema_lifecycle::ensure_schema,
    search_reconciler::{reconcile_once, SearchReconcilerConfig},
    sync_writer::{write_sync_page, SyncPageWrite},
};
use opentk_search::{
    meilisearch_schema, MeilisearchClient, SearchEntityKind, SearchIndexClient,
    SearchIndexDocument, SearchIndexOperation, SearchIndexSchema, SearchReconcilerClient,
};
use opentk_sync::payload::parse_entity_xml;
use reqwest::StatusCode;
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires local PostgreSQL and Meilisearch; run with OPENTK_TEST_DATABASE_URL and OPENTK_TEST_MEILI_URL"]
async fn real_meilisearch_reconciler_converges_empty_stale_and_mismatch(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = migrated_pool("search_reconciler_meili").await?;
    seed_document(&pool, first_document_id(), 1, "2026D00001").await;
    seed_document(&pool, second_document_id(), 2, "2026D00002").await;
    let meili_url = std::env::var("OPENTK_TEST_MEILI_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7700".to_owned());
    let index_name = format!("opentk_reconciler_test_{}", std::process::id());
    let client = MeilisearchClient::new(meili_url.clone(), None, index_name.clone());
    let http = reqwest::Client::new();
    delete_index_if_exists(&http, &meili_url, &index_name).await?;

    let config = config(index_name.clone(), 50);

    let empty_report = reconcile_once(&pool, &client, &config).await?;
    assert!(empty_report.categories[0].completed);
    assert_eq!(empty_report.categories[0].postgres_count, 2);
    assert_eq!(empty_report.categories[0].meilisearch_count, 2);
    let empty_highest = client.highest_skiptoken("Document").await?;
    let empty_count = client.count_category_prefix("Document", None).await?;
    assert_eq!(empty_highest, Some(2));
    assert_eq!(empty_count, 2);
    let sorted_highest = assert_sortable_latest_skiptoken(&http, &meili_url, &index_name).await?;
    println!(
        "empty_convergence_report={:?} highest_skiptoken={empty_highest:?} meili_count={empty_count} sorted_highest={sorted_highest}",
        empty_report.categories[0]
    );

    client.reset_index(&schema(index_name.clone())).await?;
    client
        .apply_batch(&[
            SearchIndexOperation::Upsert(Box::new(stale_document(first_document_id(), 1))),
            SearchIndexOperation::Upsert(Box::new(stale_document(second_document_id(), 2))),
        ])
        .await?;
    let stale_report = reconcile_once(&pool, &client, &config).await?;
    assert!(stale_report.categories[0].completed);
    assert_eq!(stale_report.categories[0].verified_prefix_boundary, 2);
    assert_eq!(stale_report.categories[0].inserted_rows, 2);
    let refreshed = document(&http, &meili_url, &index_name, first_document_id()).await?;
    assert_ne!(refreshed["title"], "stale title");
    println!(
        "stale_refresh_report={:?} refreshed_title={}",
        stale_report.categories[0], refreshed["title"]
    );

    client.reset_index(&schema(index_name.clone())).await?;
    client
        .apply_batch(&[SearchIndexOperation::Upsert(Box::new(stale_document(
            Uuid::parse_str("33333333-3333-4333-8333-333333333333")?,
            2,
        )))])
        .await?;
    let mismatch_report = reconcile_once(&pool, &client, &config).await?;
    assert!(mismatch_report.categories[0].completed);
    assert_eq!(mismatch_report.categories[0].verified_prefix_boundary, 0);
    assert_eq!(client.count_category_prefix("Document", None).await?, 2);
    let extra = http
        .get(format!(
            "{meili_url}/indexes/{index_name}/documents/Document_33333333-3333-4333-8333-333333333333"
        ))
        .send()
        .await?;
    assert_eq!(extra.status(), StatusCode::NOT_FOUND);
    println!(
        "mismatch_repair_report={:?} extra_document_status={}",
        mismatch_report.categories[0],
        extra.status()
    );

    delete_index_if_exists(&http, &meili_url, &index_name).await?;
    Ok(())
}

fn config(index_name: String, batch_size: i64) -> SearchReconcilerConfig {
    SearchReconcilerConfig {
        index_name,
        categories: vec!["Document".to_owned()],
        batch_size,
        max_payload_bytes: 80_000_000,
        loop_interval: Duration::from_secs(1),
    }
}

fn schema(index_name: String) -> SearchIndexSchema {
    SearchIndexSchema {
        index_name: Box::leak(index_name.into_boxed_str()),
        ..meilisearch_schema()
    }
}

async fn assert_sortable_latest_skiptoken(
    http: &reqwest::Client,
    meili_url: &str,
    index_name: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    let sortable = http
        .get(format!(
            "{meili_url}/indexes/{index_name}/settings/sortable-attributes"
        ))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let sortable: Value = serde_json::from_str(&sortable)?;
    assert!(sortable
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == "latest_skiptoken")));
    println!("sortable_attributes={sortable}");

    let highest = http
        .post(format!("{meili_url}/indexes/{index_name}/documents/fetch"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "filter": "source_category = Document",
                "sort": ["latest_skiptoken:desc"],
                "limit": 1,
                "fields": ["latest_skiptoken"]
            })
            .to_string(),
        )
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let highest: Value = serde_json::from_str(&highest)?;
    assert_eq!(highest["results"][0]["latest_skiptoken"], 2);
    println!("documents_fetch_highest_response={highest}");
    Ok(2)
}

async fn document(
    http: &reqwest::Client,
    meili_url: &str,
    index_name: &str,
    source_id: Uuid,
) -> Result<Value, Box<dyn std::error::Error>> {
    let body = http
        .get(format!(
            "{meili_url}/indexes/{index_name}/documents/Document_{source_id}"
        ))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(serde_json::from_str(&body)?)
}

async fn delete_index_if_exists(
    http: &reqwest::Client,
    meili_url: &str,
    index_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = http
        .delete(format!("{meili_url}/indexes/{index_name}"))
        .send()
        .await?;
    if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
        Ok(())
    } else {
        Err(format!("failed to delete test index: {}", response.status()).into())
    }
}

async fn migrated_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run search reconciler tests");
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let schema_name = format!("opentk_{test_name}_{}", std::process::id());
    sqlx::query(&format!(
        "DROP SCHEMA IF EXISTS {} CASCADE",
        quote_ident(&schema_name)
    ))
    .execute(&admin_pool)
    .await?;
    sqlx::query(&format!("CREATE SCHEMA {}", quote_ident(&schema_name)))
        .execute(&admin_pool)
        .await?;
    admin_pool.close().await;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_search_path(&database_url, &schema_name))
        .await?;
    ensure_schema(&pool)
        .await
        .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
    Ok(pool)
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

async fn seed_document(pool: &PgPool, source_id: Uuid, skiptoken: i64, document_number: &str) {
    let document = parse_entity_xml("Document", &document_xml(source_id, document_number))
        .expect("document payload parses");
    write_sync_page(
        pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: skiptoken,
            next_url: None,
            atom_updated_at: atom_updated_at(),
            entities: vec![document],
        },
    )
    .await
    .expect("document writes");
}

fn document_xml(source_id: Uuid, document_nummer: &str) -> String {
    format!(
        r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="{source_id}"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z"
            contentType="application/pdf"
            contentLength="12">
            <documentNummer>{document_nummer}</documentNummer>
            <onderwerp>Document asset task</onderwerp>
            <datum>2026-04-26T00:00:00Z</datum>
            <volgnummer>1</volgnummer>
            <vergaderjaar>2025-2026</vergaderjaar>
            <kamer>2</kamer>
        </document>"#
    )
}

fn atom_updated_at() -> DateTime<Utc> {
    "2026-04-26T01:00:00Z".parse().expect("valid timestamp")
}

fn first_document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid")
}

fn second_document_id() -> Uuid {
    Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid")
}

fn stale_document(source_id: Uuid, latest_skiptoken: i64) -> SearchIndexDocument {
    SearchIndexDocument {
        id: format!("Document_{source_id}"),
        key: format!("Document:{source_id}"),
        source_category: "Document".to_owned(),
        source_id,
        entity_kind: SearchEntityKind::Document,
        title: "stale title".to_owned(),
        summary: None,
        source_url: None,
        date: None,
        document_number: None,
        extracted_text: None,
        extracted_html: None,
        metadata_text: Vec::new(),
        relation_labels: Vec::new(),
        filter_categories: vec!["Document".to_owned()],
        latest_skiptoken,
        source_updated_at: atom_updated_at(),
        atom_updated_at: atom_updated_at(),
    }
}
