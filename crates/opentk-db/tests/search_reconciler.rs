use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use opentk_db::{
    schema_lifecycle::ensure_schema,
    search_reconciler::{reconcile_once, SearchReconcilerConfig},
    sync_writer::{write_sync_page, SyncPageWrite},
};
use opentk_search::{
    SearchIndexClient, SearchIndexDocument, SearchIndexError, SearchIndexOperation,
    SearchIndexSchema, SearchReconcilerClient,
};
use opentk_sync::payload::parse_entity_xml;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[tokio::test]
async fn reconciler_converges_empty_index_and_clears_scratch() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_reconciler_empty").await?;
    seed_document(&pool, first_document_id(), 1, "2026D00001").await;
    seed_document(&pool, second_document_id(), 2, "2026D00002").await;
    let client = MemoryReconcilerClient::default();

    let report = reconcile_once(&pool, &client, &config(1))
        .await
        .expect("reconcile succeeds");

    assert_eq!(client.documents().len(), 2);
    assert_eq!(report.categories[0].postgres_count, 2);
    assert_eq!(report.categories[0].meilisearch_count, 2);
    assert_eq!(report.categories[0].inserted_rows, 2);
    assert_eq!(report.categories[0].target_boundary, Some(2));
    assert!(report.categories[0].completed);
    let scratch_rows: i64 =
        sqlx::query_scalar("SELECT count(*)::bigint FROM search_reconciler_scratch")
            .fetch_one(&pool)
            .await?;
    println!(
        "empty_index_report={:?} final_document_count={} scratch_rows={scratch_rows}",
        report.categories[0],
        client.documents().len()
    );
    assert_eq!(scratch_rows, 0);
    Ok(())
}

#[tokio::test]
async fn reconciler_stops_when_total_counts_match() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_reconciler_stale").await?;
    seed_document(&pool, first_document_id(), 1, "2026D00001").await;
    seed_document(&pool, second_document_id(), 2, "2026D00002").await;
    let client = MemoryReconcilerClient::default();
    client.seed(stale_document(first_document_id(), 1));
    client.seed(stale_document(second_document_id(), 2));

    let report = reconcile_once(&pool, &client, &config(50))
        .await
        .expect("reconcile succeeds");

    let documents = client.documents();
    assert_eq!(documents.len(), 2);
    assert!(documents
        .values()
        .all(|document| document.title == "stale title"));
    assert_eq!(report.categories[0].verified_prefix_boundary, 2);
    assert_eq!(report.categories[0].inserted_rows, 0);
    assert_eq!(report.categories[0].target_boundary, None);
    assert!(report.categories[0].completed);
    println!(
        "same_count_stop_report={:?} untouched_titles={:?}",
        report.categories[0],
        documents
            .values()
            .map(|document| document.title.clone())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn reconciler_repairs_missing_bounded_window_without_deleting_suffix(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_reconciler_missing_window").await?;
    seed_document(&pool, first_document_id(), 1, "2026D00001").await;
    seed_document(&pool, second_document_id(), 2, "2026D00002").await;
    seed_document(
        &pool,
        Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        3,
        "2026D00003",
    )
    .await;
    let client = MemoryReconcilerClient::default();
    client.seed(stale_document(first_document_id(), 1));
    client.seed(stale_document(
        Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        3,
    ));

    let report = reconcile_once(&pool, &client, &config(1))
        .await
        .expect("reconcile succeeds");

    let documents = client.documents();
    assert_eq!(documents.len(), 3);
    assert!(documents.contains_key(&format!("Document_{}", first_document_id())));
    assert!(documents.contains_key(&format!("Document_{}", second_document_id())));
    assert!(documents.contains_key("Document_33333333-3333-4333-8333-333333333333"));
    assert_eq!(report.categories[0].postgres_count, 3);
    assert_eq!(report.categories[0].meilisearch_count, 3);
    assert_eq!(report.categories[0].verified_prefix_boundary, 1);
    assert_eq!(report.categories[0].deleted_rows, 0);
    assert!(report.categories[0].completed);
    println!(
        "missing_window_repair_report={:?} final_document_ids={:?}",
        report.categories[0],
        documents.keys().cloned().collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn reconciler_deletes_only_exact_extra_documents_then_rebuilds() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_reconciler_mismatch").await?;
    seed_document(&pool, first_document_id(), 1, "2026D00001").await;
    seed_document(&pool, second_document_id(), 2, "2026D00002").await;
    let client = MemoryReconcilerClient::default();
    client.seed(stale_document(
        Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        2,
    ));

    let report = reconcile_once(&pool, &client, &config(50))
        .await
        .expect("reconcile succeeds");

    let documents = client.documents();
    assert_eq!(documents.len(), 2);
    assert!(!documents.contains_key("Document_33333333-3333-4333-8333-333333333333"));
    assert_eq!(report.categories[0].verified_prefix_boundary, 0);
    assert_eq!(report.categories[0].deleted_rows, 1);
    assert!(report.categories[0].completed);
    println!(
        "mismatch_repair_report={:?} final_document_ids={:?}",
        report.categories[0],
        documents.keys().cloned().collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn reconciler_retries_category_until_counts_match() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_reconciler_retry_category").await?;
    seed_document(&pool, first_document_id(), 1, "2026D00001").await;
    seed_document(&pool, second_document_id(), 2, "2026D00002").await;
    let client = MemoryReconcilerClient::default();
    client.undercount_total_once();

    let report = reconcile_once(&pool, &client, &config(50))
        .await
        .expect("reconcile succeeds");

    assert_eq!(client.documents().len(), 2);
    assert_eq!(report.categories[0].postgres_count, 2);
    assert_eq!(report.categories[0].meilisearch_count, 2);
    assert_eq!(report.categories[0].verified_prefix_boundary, 2);
    assert!(report.categories[0].completed);
    assert_eq!(client.remaining_total_undercounts(), 0);
    Ok(())
}

#[tokio::test]
async fn reconciler_indexes_all_rows_tied_at_target_boundary() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_reconciler_boundary_ties").await?;
    seed_document(&pool, first_document_id(), 10, "2026D00001").await;
    seed_document(&pool, second_document_id(), 10, "2026D00002").await;
    seed_document(
        &pool,
        Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        10,
        "2026D00003",
    )
    .await;
    seed_document(
        &pool,
        Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
        10,
        "2026D00004",
    )
    .await;
    let client = MemoryReconcilerClient::default();

    let report = reconcile_once(&pool, &client, &config(2))
        .await
        .expect("reconcile succeeds");

    let documents = client.documents();
    assert_eq!(documents.len(), 4);
    assert_eq!(report.categories[0].postgres_count, 4);
    assert_eq!(report.categories[0].meilisearch_count, 4);
    assert_eq!(report.categories[0].inserted_rows, 4);
    assert_eq!(report.categories[0].target_boundary, Some(10));
    assert!(report.categories[0].completed);
    println!(
        "boundary_tie_report={:?} final_document_ids={:?}",
        report.categories[0],
        documents.keys().cloned().collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn reconciler_counts_only_live_rows_with_materialized_details() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_reconciler_missing_detail").await?;
    seed_document(&pool, first_document_id(), 1, "2026D00001").await;
    seed_document(&pool, second_document_id(), 2, "2026D00002").await;
    seed_unmaterialized_sync_entity(
        &pool,
        "Document",
        Uuid::parse_str("55555555-5555-4555-8555-555555555555").unwrap(),
        3,
    )
    .await;
    let client = MemoryReconcilerClient::default();

    let report = reconcile_once(&pool, &client, &config(50))
        .await
        .expect("reconcile succeeds");

    let raw_sync_rows: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint
         FROM sync_entity
         WHERE source_category = 'Document' AND deleted = false",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(raw_sync_rows, 3);
    assert_eq!(client.documents().len(), 2);
    assert_eq!(report.categories[0].postgres_count, 2);
    assert_eq!(report.categories[0].meilisearch_count, 2);
    assert!(report.categories[0].completed);
    println!(
        "missing_detail_report={:?} raw_sync_rows={raw_sync_rows} final_document_count={}",
        report.categories[0],
        client.documents().len()
    );
    Ok(())
}

fn config(batch_size: i64) -> SearchReconcilerConfig {
    SearchReconcilerConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size,
        max_payload_bytes: 80_000_000,
        loop_interval: Duration::from_secs(1),
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

async fn seed_unmaterialized_sync_entity(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
    skiptoken: i64,
) {
    sqlx::query(
        "INSERT INTO sync_entity (
             source_category, source_id, latest_skiptoken, deleted,
             source_updated_at, atom_updated_at
         )
         VALUES ($1, $2, $3, false, $4, $4)",
    )
    .bind(category)
    .bind(source_id)
    .bind(skiptoken)
    .bind(atom_updated_at())
    .execute(pool)
    .await
    .expect("unmaterialized sync entity writes");
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

#[derive(Default)]
struct MemoryReconcilerClient {
    documents: Mutex<BTreeMap<String, SearchIndexDocument>>,
    total_undercounts: AtomicUsize,
}

impl MemoryReconcilerClient {
    fn seed(&self, document: SearchIndexDocument) {
        self.documents
            .lock()
            .expect("documents mutex")
            .insert(document.id.clone(), document);
    }

    fn documents(&self) -> BTreeMap<String, SearchIndexDocument> {
        self.documents.lock().expect("documents mutex").clone()
    }

    fn undercount_total_once(&self) {
        self.total_undercounts.store(1, Ordering::SeqCst);
    }

    fn remaining_total_undercounts(&self) -> usize {
        self.total_undercounts.load(Ordering::SeqCst)
    }
}

impl SearchIndexClient for MemoryReconcilerClient {
    async fn reset_index(&self, _schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        self.documents.lock().expect("documents mutex").clear();
        Ok(())
    }

    async fn apply_batch(
        &self,
        operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError> {
        let mut documents = self.documents.lock().expect("documents mutex");
        for operation in operations {
            match operation {
                SearchIndexOperation::Upsert(document) => {
                    documents.insert(document.id.clone(), document.as_ref().clone());
                }
                SearchIndexOperation::Delete(document_id) => {
                    documents.remove(document_id);
                }
            }
        }
        Ok(())
    }
}

impl SearchReconcilerClient for MemoryReconcilerClient {
    async fn ensure_index(&self, _schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        Ok(())
    }

    async fn highest_skiptoken(
        &self,
        source_category: &str,
    ) -> Result<Option<i64>, SearchIndexError> {
        Ok(self
            .documents
            .lock()
            .expect("documents mutex")
            .values()
            .filter(|document| document.source_category == source_category)
            .map(|document| document.latest_skiptoken)
            .max())
    }

    async fn count_category_prefix(
        &self,
        source_category: &str,
        boundary: Option<i64>,
    ) -> Result<u64, SearchIndexError> {
        let count = self
            .documents
            .lock()
            .expect("documents mutex")
            .values()
            .filter(|document| document.source_category == source_category)
            .filter(|document| {
                boundary.is_none_or(|boundary| document.latest_skiptoken <= boundary)
            })
            .count()
            .try_into()
            .unwrap_or(u64::MAX);
        if boundary.is_none()
            && self
                .total_undercounts
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
        {
            return Ok(count.saturating_sub(1));
        }
        Ok(count)
    }

    async fn document_ids_category_after(
        &self,
        source_category: &str,
        boundary: i64,
    ) -> Result<Vec<String>, SearchIndexError> {
        Ok(self
            .documents
            .lock()
            .expect("documents mutex")
            .values()
            .filter(|document| document.source_category == source_category)
            .filter(|document| document.latest_skiptoken > boundary)
            .map(|document| document.id.clone())
            .collect())
    }
}

fn stale_document(source_id: Uuid, latest_skiptoken: i64) -> SearchIndexDocument {
    SearchIndexDocument {
        id: format!("Document_{source_id}"),
        key: format!("Document:{source_id}"),
        source_category: "Document".to_owned(),
        source_id,
        entity_kind: opentk_search::SearchEntityKind::Document,
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
