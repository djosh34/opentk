use chrono::{DateTime, Utc};
use opentk_db::{
    document_assets::{record_document_asset_fetches, record_document_content_extractions},
    sync_writer::{write_sync_page, SyncPageWrite},
};
use opentk_sync::{
    document_asset::{
        DocumentAssetFetchReport, DocumentAssetKind, DocumentSelectedSource, RetrievalStatus,
    },
    document_content::{DocumentContentExtractionReport, ExtractionStatus, ValidationStatus},
    payload::parse_entity_xml,
};
use reqwest::Url;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

#[tokio::test]
async fn records_successful_fetch_reports_as_document_assets() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("document_assets_success").await?;
    seed_document(&pool).await;
    let asset_url = Url::parse("https://example.test/document.pdf").expect("asset URL");
    let upstream_url = Url::parse("https://cdn.example.test/document.pdf").expect("upstream URL");
    let retrieved_at: DateTime<Utc> = "2026-04-26T02:00:00Z".parse().expect("timestamp");

    let outcome = record_document_asset_fetches(
        &pool,
        &[DocumentAssetFetchReport {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id(),
            asset_url: asset_url.clone(),
            upstream_url: upstream_url.clone(),
            upstream_content_type: Some("application/pdf".to_owned()),
            upstream_content_length: Some(12),
            upstream_last_modified_at: Some(retrieved_at),
            retrieval_status: RetrievalStatus::Fetched,
            retrieval_error: None,
            retrieved_at,
            source_hash: Some("hash-pdf".to_owned()),
            selected_source: Some(DocumentSelectedSource {
                url: upstream_url.clone(),
                content_type: Some("application/pdf".to_owned()),
                content_length: Some(12),
                kind: DocumentAssetKind::Pdf,
                official_source: false,
                source_rank: 10,
            }),
            selected_body: Some(b"%PDF fixture".to_vec()),
            discovered_sources: Vec::new(),
        }],
    )
    .await
    .expect("asset report records");

    assert_eq!(outcome.assets_recorded, 1);
    assert_eq!(outcome.contents_recorded, 0);
    assert_eq!(outcome.failures_recorded, 0);

    let row = sqlx::query(
        "SELECT asset_url, upstream_url, upstream_content_type, upstream_content_length,
                upstream_last_modified_at, retrieval_status, retrieval_error, retrieved_at
         FROM document_asset
         WHERE document_source_category = 'Document' AND document_source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;

    assert_eq!(row.get::<String, _>("asset_url"), asset_url.as_str());
    assert_eq!(row.get::<String, _>("upstream_url"), upstream_url.as_str());
    assert_eq!(
        row.get::<Option<String>, _>("upstream_content_type")
            .as_deref(),
        Some("application/pdf")
    );
    assert_eq!(
        row.get::<Option<i64>, _>("upstream_content_length"),
        Some(12)
    );
    assert_eq!(row.get::<String, _>("retrieval_status"), "fetched");
    assert_eq!(row.get::<Option<String>, _>("retrieval_error"), None);
    assert_eq!(
        row.get::<Option<DateTime<Utc>>, _>("retrieved_at"),
        Some(retrieved_at)
    );

    let content_rows: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM document_content")
        .fetch_one(&pool)
        .await?;
    assert_eq!(content_rows, 0);
    Ok(())
}

#[tokio::test]
async fn records_official_selected_source_as_document_content() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("document_assets_content").await?;
    seed_document(&pool).await;
    let asset_url = Url::parse("https://example.test/document.html").expect("asset URL");
    let text_url = Url::parse("https://example.test/document.txt").expect("text URL");
    let retrieved_at: DateTime<Utc> = "2026-04-26T02:00:00Z".parse().expect("timestamp");

    let outcome = record_document_asset_fetches(
        &pool,
        &[DocumentAssetFetchReport {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id(),
            asset_url,
            upstream_url: Url::parse("https://example.test/document.html").expect("upstream URL"),
            upstream_content_type: Some("text/html".to_owned()),
            upstream_content_length: Some(82),
            upstream_last_modified_at: None,
            retrieval_status: RetrievalStatus::Fetched,
            retrieval_error: None,
            retrieved_at,
            source_hash: Some("hash-text".to_owned()),
            selected_source: Some(DocumentSelectedSource {
                url: text_url.clone(),
                content_type: Some("text/plain".to_owned()),
                content_length: Some(24),
                kind: DocumentAssetKind::OfficialText,
                official_source: true,
                source_rank: 0,
            }),
            selected_body: Some(b"official transcript text".to_vec()),
            discovered_sources: Vec::new(),
        }],
    )
    .await
    .expect("official content records");

    assert_eq!(outcome.assets_recorded, 1);
    assert_eq!(outcome.contents_recorded, 1);
    assert_eq!(outcome.failures_recorded, 0);

    let row = sqlx::query(
        "SELECT selected_source_url, selected_source_content_type,
                selected_source_content_length, official_source, source_rank,
                extraction_status, validation_status, extraction_tool,
                extraction_tool_version, source_hash, output_hash,
                extraction_error, extracted_text, extracted_html
         FROM document_content
         WHERE document_source_category = 'Document' AND document_source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;

    assert_eq!(
        row.get::<String, _>("selected_source_url"),
        text_url.as_str()
    );
    assert_eq!(
        row.get::<Option<String>, _>("selected_source_content_type")
            .as_deref(),
        Some("text/plain")
    );
    assert_eq!(
        row.get::<Option<i64>, _>("selected_source_content_length"),
        Some(24)
    );
    assert!(row.get::<bool, _>("official_source"));
    assert_eq!(row.get::<i32, _>("source_rank"), 0);
    assert_eq!(row.get::<String, _>("extraction_status"), "extracted");
    assert_eq!(row.get::<String, _>("validation_status"), "unverified");
    assert_eq!(row.get::<String, _>("extraction_tool"), "official-source");
    assert_eq!(row.get::<String, _>("extraction_tool_version"), "1");
    assert_eq!(row.get::<String, _>("source_hash"), "hash-text");
    assert_eq!(
        row.get::<Option<String>, _>("output_hash").as_deref(),
        Some("0ae0f2625510c92b5b792ec6103b96fbfa40d52940ace95fe7ce1e02d93a3aa3")
    );
    assert_eq!(row.get::<Option<String>, _>("extraction_error"), None);
    assert_eq!(
        row.get::<Option<String>, _>("extracted_text").as_deref(),
        Some("official transcript text")
    );
    assert_eq!(row.get::<Option<String>, _>("extracted_html"), None);
    Ok(())
}

#[tokio::test]
async fn records_binary_extraction_content_with_provenance() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("document_extraction_success").await?;
    seed_document(&pool).await;
    let document_asset_id = seed_asset_row(&pool).await?;
    let extracted_at: DateTime<Utc> = "2026-04-26T03:00:00Z".parse().expect("timestamp");

    let outcome = record_document_content_extractions(
        &pool,
        &[DocumentContentExtractionReport {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id(),
            document_asset_id: Some(document_asset_id),
            selected_source_url: Url::parse("https://example.test/document.pdf").expect("url"),
            selected_source_content_type: Some("application/pdf".to_owned()),
            selected_source_content_length: Some(12),
            official_source: false,
            source_rank: 10,
            extraction_status: ExtractionStatus::Extracted,
            validation_status: ValidationStatus::Valid,
            extraction_error: None,
            extraction_tool: "pdf-extract".to_owned(),
            extraction_tool_version: "0.10.0".to_owned(),
            source_hash: "source-hash".to_owned(),
            output_hash: Some("output-hash".to_owned()),
            extracted_text: Some("PDF fixture heading".to_owned()),
            extracted_html: None,
            extracted_at,
        }],
    )
    .await
    .expect("extraction content records");

    assert_eq!(outcome.contents_recorded, 1);
    assert_eq!(outcome.failures_recorded, 0);

    let row = sqlx::query(
        "SELECT official_source, extraction_status, validation_status,
                extraction_tool, extraction_tool_version, source_hash,
                output_hash, extraction_error, extracted_text, extracted_at
         FROM document_content
         WHERE document_asset_id = $1",
    )
    .bind(document_asset_id)
    .fetch_one(&pool)
    .await?;

    assert!(!row.get::<bool, _>("official_source"));
    assert_eq!(row.get::<String, _>("extraction_status"), "extracted");
    assert_eq!(row.get::<String, _>("validation_status"), "valid");
    assert_eq!(row.get::<String, _>("extraction_tool"), "pdf-extract");
    assert_eq!(row.get::<String, _>("extraction_tool_version"), "0.10.0");
    assert_eq!(row.get::<String, _>("source_hash"), "source-hash");
    assert_eq!(
        row.get::<Option<String>, _>("output_hash").as_deref(),
        Some("output-hash")
    );
    assert_eq!(row.get::<Option<String>, _>("extraction_error"), None);
    assert_eq!(
        row.get::<Option<String>, _>("extracted_text").as_deref(),
        Some("PDF fixture heading")
    );
    assert_eq!(row.get::<DateTime<Utc>, _>("extracted_at"), extracted_at);
    Ok(())
}

#[tokio::test]
async fn records_extraction_failures_idempotently_without_fake_body() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("document_extraction_failure").await?;
    seed_document(&pool).await;
    let document_asset_id = seed_asset_row(&pool).await?;
    let report = DocumentContentExtractionReport {
        document_source_category: "Document".to_owned(),
        document_source_id: document_id(),
        document_asset_id: Some(document_asset_id),
        selected_source_url: Url::parse("https://example.test/document.pdf").expect("url"),
        selected_source_content_type: Some("application/pdf".to_owned()),
        selected_source_content_length: Some(12),
        official_source: false,
        source_rank: 10,
        extraction_status: ExtractionStatus::Failed,
        validation_status: ValidationStatus::Invalid,
        extraction_error: Some("fixture mismatch for extracted document content".to_owned()),
        extraction_tool: "pdf-extract".to_owned(),
        extraction_tool_version: "0.10.0".to_owned(),
        source_hash: "bad-source-hash".to_owned(),
        output_hash: None,
        extracted_text: None,
        extracted_html: None,
        extracted_at: "2026-04-26T03:00:00Z".parse().expect("timestamp"),
    };

    record_document_content_extractions(&pool, std::slice::from_ref(&report))
        .await
        .expect("failure records");
    let outcome = record_document_content_extractions(&pool, &[report])
        .await
        .expect("failure upserts");

    assert_eq!(outcome.contents_recorded, 1);
    assert_eq!(outcome.failures_recorded, 1);

    let row = sqlx::query(
        "SELECT count(*)::bigint AS row_count,
                max(extraction_status) AS extraction_status,
                max(validation_status) AS validation_status,
                max(extraction_error) AS extraction_error,
                max(extracted_text) AS extracted_text
         FROM document_content
         WHERE document_asset_id = $1",
    )
    .bind(document_asset_id)
    .fetch_one(&pool)
    .await?;

    assert_eq!(row.get::<i64, _>("row_count"), 1);
    assert_eq!(
        row.get::<Option<String>, _>("extraction_status").as_deref(),
        Some("failed")
    );
    assert_eq!(
        row.get::<Option<String>, _>("validation_status").as_deref(),
        Some("invalid")
    );
    assert_eq!(
        row.get::<Option<String>, _>("extraction_error").as_deref(),
        Some("fixture mismatch for extracted document content")
    );
    assert_eq!(row.get::<Option<String>, _>("extracted_text"), None);
    Ok(())
}

#[tokio::test]
async fn failure_reports_update_asset_rows_without_fake_content() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("document_assets_failures").await?;
    seed_document(&pool).await;
    let asset_url = Url::parse("https://example.test/document.pdf").expect("asset URL");
    let retrieved_at: DateTime<Utc> = "2026-04-26T02:00:00Z".parse().expect("timestamp");

    let first = record_document_asset_fetches(
        &pool,
        &[DocumentAssetFetchReport {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id(),
            asset_url: asset_url.clone(),
            upstream_url: asset_url.clone(),
            upstream_content_type: Some("application/pdf".to_owned()),
            upstream_content_length: Some(12),
            upstream_last_modified_at: None,
            retrieval_status: RetrievalStatus::Failed,
            retrieval_error: Some(
                "expected content length 999 did not match actual body length 12".to_owned(),
            ),
            retrieved_at,
            source_hash: None,
            selected_source: None,
            selected_body: None,
            discovered_sources: Vec::new(),
        }],
    )
    .await
    .expect("first failure records");
    assert_eq!(first.failures_recorded, 1);

    let second = record_document_asset_fetches(
        &pool,
        &[DocumentAssetFetchReport {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id(),
            asset_url: asset_url.clone(),
            upstream_url: asset_url.clone(),
            upstream_content_type: Some("image/png".to_owned()),
            upstream_content_length: Some(3),
            upstream_last_modified_at: None,
            retrieval_status: RetrievalStatus::UnsupportedContentType,
            retrieval_error: Some("unsupported content type".to_owned()),
            retrieved_at,
            source_hash: Some("must-not-create-content".to_owned()),
            selected_source: None,
            selected_body: Some(b"png".to_vec()),
            discovered_sources: Vec::new(),
        }],
    )
    .await
    .expect("second failure updates same row");
    assert_eq!(second.assets_recorded, 1);
    assert_eq!(second.failures_recorded, 1);
    assert_eq!(second.contents_recorded, 0);

    let row = sqlx::query(
        "SELECT count(*)::bigint AS row_count,
                max(retrieval_status) AS retrieval_status,
                max(retrieval_error) AS retrieval_error
         FROM document_asset
         WHERE document_source_category = 'Document'
           AND document_source_id = $1
           AND asset_url = $2",
    )
    .bind(document_id())
    .bind(asset_url.as_str())
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.get::<i64, _>("row_count"), 1);
    assert_eq!(
        row.get::<Option<String>, _>("retrieval_status").as_deref(),
        Some("unsupported_content_type")
    );
    assert_eq!(
        row.get::<Option<String>, _>("retrieval_error").as_deref(),
        Some("unsupported content type")
    );

    let content_rows: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM document_content")
        .fetch_one(&pool)
        .await?;
    assert_eq!(content_rows, 0);
    Ok(())
}

async fn seed_document(pool: &PgPool) {
    let document = parse_entity_xml("Document", &document_xml()).expect("document payload parses");
    write_sync_page(
        pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: 1,
            next_url: None,
            atom_updated_at: "2026-04-26T01:00:00Z".parse().expect("timestamp"),
            entities: vec![document],
        },
    )
    .await
    .expect("document writes");
}

async fn seed_asset_row(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let asset_url = Url::parse("https://example.test/document.pdf").expect("asset URL");
    record_document_asset_fetches(
        pool,
        &[DocumentAssetFetchReport {
            document_source_category: "Document".to_owned(),
            document_source_id: document_id(),
            asset_url: asset_url.clone(),
            upstream_url: asset_url,
            upstream_content_type: Some("application/pdf".to_owned()),
            upstream_content_length: Some(12),
            upstream_last_modified_at: None,
            retrieval_status: RetrievalStatus::Fetched,
            retrieval_error: None,
            retrieved_at: "2026-04-26T02:00:00Z".parse().expect("timestamp"),
            source_hash: Some("source-hash".to_owned()),
            selected_source: Some(DocumentSelectedSource {
                url: Url::parse("https://example.test/document.pdf").expect("url"),
                content_type: Some("application/pdf".to_owned()),
                content_length: Some(12),
                kind: DocumentAssetKind::Pdf,
                official_source: false,
                source_rank: 10,
            }),
            selected_body: Some(b"%PDF fixture".to_vec()),
            discovered_sources: Vec::new(),
        }],
    )
    .await
    .expect("asset row records");

    sqlx::query_scalar(
        "SELECT id FROM document_asset
         WHERE document_source_category = 'Document' AND document_source_id = $1",
    )
    .bind(document_id())
    .fetch_one(pool)
    .await
}

async fn migrated_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect(
            "set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL document asset tests",
        );
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
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(pool)
}

fn document_xml() -> String {
    r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
        id="11111111-1111-4111-8111-111111111111"
        verwijderd="false"
        bijgewerkt="2026-04-26T00:00:00Z"
        contentType="application/pdf"
        contentLength="12">
        <documentNummer>2026D00001</documentNummer>
        <onderwerp>Document asset task</onderwerp>
        <datum>2026-04-26T00:00:00Z</datum>
        <volgnummer>1</volgnummer>
        <vergaderjaar>2025-2026</vergaderjaar>
        <kamer>2</kamer>
    </document>"#
        .to_owned()
}

fn document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid")
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
