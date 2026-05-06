use std::sync::Mutex;

use chrono::{DateTime, Utc};
use opentk_db::{
    document_assets::{record_document_asset_fetches, record_document_content_extractions},
    schema_lifecycle::ensure_schema,
    search_sync::{
        full_reindex, incremental_index, list_failures, verify_index_completeness,
        SearchCompletenessConfig, SearchIndexCompletenessState, SearchSyncConfig,
    },
    search_sync::{index_records, SearchSyncError, SearchSyncRecordKey},
    sync_writer::{write_sync_page, SyncPageWrite},
};
use opentk_search::{
    SearchCountClient, SearchFilter, SearchIndexClient, SearchIndexError, SearchIndexOperation,
    SearchIndexSchema,
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
async fn schema_lifecycle_creates_durable_search_index_state() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_state").await?;

    let cursor =
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('search_index_cursor')::text")
            .fetch_one(&pool)
            .await?;
    assert_eq!(cursor.as_deref(), Some("search_index_cursor"));

    let failure =
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('search_index_failure')::text")
            .fetch_one(&pool)
            .await?;
    assert_eq!(failure.as_deref(), Some("search_index_failure"));

    sqlx::query(
        "INSERT INTO search_index_cursor (
            index_name,
            source_category,
            latest_skiptoken,
            state
         )
         VALUES ('opentk_entities', 'Document', 42, 'caught_up')",
    )
    .execute(&pool)
    .await?;

    let cursor_row = sqlx::query(
        "SELECT latest_skiptoken, state
         FROM search_index_cursor
         WHERE index_name = 'opentk_entities' AND source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor_row.get::<i64, _>("latest_skiptoken"), 42);
    assert_eq!(cursor_row.get::<String, _>("state"), "caught_up");

    Ok(())
}

#[tokio::test]
async fn full_reindex_builds_document_index_from_postgres() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_full_reindex").await?;
    seed_document_with_content(&pool, 42, "2026D00001", "stored text", "<p>stored html</p>").await;
    let client = MemoryIndexClient::default();

    let report = full_reindex(
        &pool,
        &client,
        &SearchSyncConfig {
            index_name: "opentk_entities".to_owned(),
            categories: vec!["Document".to_owned()],
            batch_size: 501,
            max_payload_bytes: 80_000_000,
            retry_limit: 3,
        },
    )
    .await
    .expect("full reindex succeeds");

    assert_eq!(report.indexed, 1);
    assert_eq!(report.deleted, 0);
    assert_eq!(report.failed, 0);
    assert!(client.reset_called());
    let operations = client.operations();
    assert_eq!(operations.len(), 1);
    let SearchIndexOperation::Upsert(document) = &operations[0] else {
        panic!("document is upserted");
    };
    assert_eq!(document.key, format!("Document:{}", document_id()));
    assert_eq!(document.source_category, "Document");
    assert_eq!(document.source_id, document_id());
    assert_eq!(document.title, "Document asset task");
    assert_eq!(document.document_number.as_deref(), Some("2026D00001"));
    assert_eq!(document.extracted_text.as_deref(), Some("stored text"));
    assert_eq!(
        document.extracted_html.as_deref(),
        Some("<p>stored html</p>")
    );
    assert!(
        document
            .metadata_text
            .iter()
            .any(|value| value == "validation_status: valid"),
        "document content validation metadata is indexed"
    );
    assert_eq!(document.latest_skiptoken, 42);

    let cursor_skiptoken: i64 = sqlx::query_scalar(
        "SELECT latest_skiptoken
         FROM search_index_cursor
         WHERE index_name = 'opentk_entities' AND source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor_skiptoken, 42);

    Ok(())
}

#[tokio::test]
async fn incremental_index_reflects_postgres_document_update() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_incremental_update").await?;
    seed_document_with_content(&pool, 1, "2026D00001", "old text", "<p>old html</p>").await;
    let client = MemoryIndexClient::default();
    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };
    full_reindex(&pool, &client, &config)
        .await
        .expect("full reindex succeeds");

    seed_document_with_content(&pool, 2, "2026D00002", "new text", "<p>new html</p>").await;
    let report = incremental_index(&pool, &client, &config)
        .await
        .expect("incremental index succeeds");

    assert_eq!(report.indexed, 1);
    let operations = client.operations();
    assert_eq!(operations.len(), 2);
    let SearchIndexOperation::Upsert(document) = operations.last().expect("last operation") else {
        panic!("updated document is upserted");
    };
    assert_eq!(document.document_number.as_deref(), Some("2026D00002"));
    assert_eq!(document.extracted_text.as_deref(), Some("new text"));
    assert_eq!(document.latest_skiptoken, 2);

    let cursor_skiptoken: i64 = sqlx::query_scalar(
        "SELECT latest_skiptoken
         FROM search_index_cursor
         WHERE index_name = 'opentk_entities' AND source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor_skiptoken, 2);

    Ok(())
}

#[tokio::test]
async fn incremental_index_applies_created_and_deleted_records() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_incremental_create_delete").await?;
    let first_id = document_id();
    let second_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid");
    seed_document_with_content_for(
        &pool,
        first_id,
        1,
        "2026D00001",
        "first text",
        "<p>first html</p>",
    )
    .await;
    let client = MemoryIndexClient::default();
    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };
    full_reindex(&pool, &client, &config)
        .await
        .expect("full reindex succeeds");

    write_document_delete(&pool, first_id, 2).await;
    seed_document_with_content_for(
        &pool,
        second_id,
        3,
        "2026D00003",
        "created text",
        "<p>created html</p>",
    )
    .await;
    let report = incremental_index(&pool, &client, &config)
        .await
        .expect("incremental index succeeds");

    assert_eq!(report.indexed, 1);
    assert_eq!(report.deleted, 1);
    let operations = client.operations();
    assert_eq!(operations.len(), 3);
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation, SearchIndexOperation::Delete(key) if key == &format!("Document_{first_id}"))),
        "deleted document is removed from index"
    );
    assert!(
        operations.iter().any(|operation| {
            matches!(operation, SearchIndexOperation::Upsert(document)
                if document.id == format!("Document_{second_id}")
                    && document.key == format!("Document:{second_id}")
                    && document.source_id == second_id
                    && document.document_number.as_deref() == Some("2026D00003"))
        }),
        "created document is indexed"
    );

    Ok(())
}

#[tokio::test]
async fn index_records_applies_only_targeted_records_without_advancing_cursor(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_targeted_records").await?;
    let first_id = document_id();
    let second_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid");
    seed_document_with_content_for(&pool, first_id, 8, "2026D00008", "first", "<p>first</p>").await;
    seed_document_with_content_for(&pool, second_id, 9, "2026D00009", "second", "<p>second</p>")
        .await;
    sqlx::query(
        "INSERT INTO search_index_cursor (
            index_name,
            source_category,
            latest_skiptoken,
            state
         )
         VALUES ('opentk_entities', 'Document', 7, 'caught_up')",
    )
    .execute(&pool)
    .await?;
    let client = MemoryIndexClient::default();
    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };

    let report = index_records(
        &pool,
        &client,
        &config,
        &[SearchSyncRecordKey::new(
            "Document".to_owned(),
            second_id,
            9,
        )],
    )
    .await
    .expect("targeted indexing succeeds");

    assert_eq!(report.indexed, 1);
    assert_eq!(report.deleted, 0);
    let operations = client.operations();
    assert_eq!(operations.len(), 1);
    let SearchIndexOperation::Upsert(document) = &operations[0] else {
        panic!("targeted document is upserted");
    };
    assert_eq!(document.source_id, second_id);
    assert_eq!(document.document_number.as_deref(), Some("2026D00009"));

    let cursor_skiptoken: i64 = sqlx::query_scalar(
        "SELECT latest_skiptoken
         FROM search_index_cursor
         WHERE index_name = 'opentk_entities' AND source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor_skiptoken, 7);

    Ok(())
}

#[tokio::test]
async fn index_records_reuses_search_sync_validation_errors() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_targeted_validation").await?;
    let client = MemoryIndexClient::default();
    let invalid_config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 0,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };

    let error = index_records(&pool, &client, &invalid_config, &[])
        .await
        .expect_err("invalid config is rejected");
    assert!(matches!(error, SearchSyncError::InvalidBatchSize));

    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };
    let error = index_records(
        &pool,
        &client,
        &config,
        &[SearchSyncRecordKey::new(
            "NotARealCategory".to_owned(),
            Uuid::new_v4(),
            1,
        )],
    )
    .await
    .expect_err("unknown targeted category is rejected");
    assert!(
        matches!(error, SearchSyncError::UnknownCategory(category) if category == "NotARealCategory")
    );

    Ok(())
}

#[tokio::test]
async fn index_records_skips_relation_target_placeholders_until_detail_exists(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_target_placeholder").await?;
    let commissie_id = Uuid::parse_str("44444444-4444-4444-8444-444444444444").expect("valid uuid");
    sqlx::query(
        "INSERT INTO sync_entity (
            source_category,
            source_id,
            latest_skiptoken,
            deleted,
            source_updated_at,
            atom_updated_at
         )
         VALUES ('Commissie', $1, 5, false, '2026-04-26T00:00:00Z', '2026-04-26T01:00:00Z')",
    )
    .bind(commissie_id)
    .execute(&pool)
    .await?;
    let client = MemoryIndexClient::default();
    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Activiteit".to_owned(), "Commissie".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };

    let report = index_records(
        &pool,
        &client,
        &config,
        &[SearchSyncRecordKey::new(
            "Commissie".to_owned(),
            commissie_id,
            5,
        )],
    )
    .await
    .expect("placeholder entity is skipped");

    assert_eq!(report.indexed, 0);
    assert_eq!(report.deleted, 0);
    assert!(client.operations().is_empty());
    assert!(list_failures(&pool).await?.is_empty());

    Ok(())
}

#[tokio::test]
async fn index_records_allows_documents_without_extracted_content() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_document_without_content").await?;
    let client = MemoryIndexClient::default();
    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };
    write_parsed_entity(
        &pool,
        "Document",
        11,
        &document_xml(document_id(), "2026D00011"),
    )
    .await;

    let report = index_records(
        &pool,
        &client,
        &config,
        &[SearchSyncRecordKey::new(
            "Document".to_owned(),
            document_id(),
            11,
        )],
    )
    .await
    .expect("metadata-only document indexes");

    assert_eq!(report.indexed, 1);
    assert_eq!(report.deleted, 0);
    let operations = client.operations();
    assert_eq!(operations.len(), 1);
    let SearchIndexOperation::Upsert(document) = &operations[0] else {
        panic!("document is upserted");
    };
    assert_eq!(document.source_id, document_id());
    assert_eq!(document.document_number.as_deref(), Some("2026D00011"));
    assert_eq!(document.extracted_text, None);
    assert_eq!(document.extracted_html, None);
    assert!(list_failures(&pool).await?.is_empty());

    Ok(())
}

#[tokio::test]
async fn full_reindex_indexes_person_activity_metadata_and_relation_labels(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_entity_metadata").await?;
    let person_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").expect("valid uuid");
    let activity_id = Uuid::parse_str("44444444-4444-4444-8444-444444444444").expect("valid uuid");
    let commissie_id = Uuid::parse_str("55555555-5555-4555-8555-555555555555").expect("valid uuid");
    write_parsed_entity(&pool, "Persoon", 1, &person_xml(person_id)).await;
    write_parsed_entity(
        &pool,
        "Activiteit",
        2,
        &activity_xml(activity_id, commissie_id),
    )
    .await;
    let client = MemoryIndexClient::default();

    let report = full_reindex(
        &pool,
        &client,
        &SearchSyncConfig {
            index_name: "opentk_entities".to_owned(),
            categories: vec!["Persoon".to_owned(), "Activiteit".to_owned()],
            batch_size: 10,
            max_payload_bytes: 80_000_000,
            retry_limit: 3,
        },
    )
    .await
    .expect("full reindex succeeds");

    assert_eq!(report.indexed, 2);
    let operations = client.operations();
    let person = operations
        .iter()
        .find_map(|operation| match operation {
            SearchIndexOperation::Upsert(document) if document.source_id == person_id => {
                Some(document)
            }
            _ => None,
        })
        .expect("person is indexed");
    assert_eq!(person.title, "Ada Lovelace");
    assert!(
        person
            .metadata_text
            .iter()
            .any(|value| value == "nummer: P1"),
        "person scalar metadata is indexed"
    );

    let activity = operations
        .iter()
        .find_map(|operation| match operation {
            SearchIndexOperation::Upsert(document) if document.source_id == activity_id => {
                Some(document)
            }
            _ => None,
        })
        .expect("activity is indexed");
    assert_eq!(activity.title, "Procedurevergadering");
    assert!(
        activity
            .relation_labels
            .iter()
            .any(|label| label.contains("voortouwcommissie Voortouwcommissie")),
        "activity relation label is indexed"
    );

    Ok(())
}

#[tokio::test]
async fn incoming_relation_labels_are_not_indexed_as_person_backreferences(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_relation_orientation").await?;
    let person_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").expect("valid uuid");
    let first_actor_id =
        Uuid::parse_str("66666666-6666-4666-8666-666666666666").expect("valid uuid");
    let second_actor_id =
        Uuid::parse_str("77777777-7777-4777-8777-777777777777").expect("valid uuid");
    write_parsed_entity(&pool, "Persoon", 1, &person_xml(person_id)).await;
    write_parsed_entity(
        &pool,
        "ActiviteitActor",
        2,
        &activity_actor_xml(first_actor_id, person_id),
    )
    .await;
    write_parsed_entity(
        &pool,
        "ActiviteitActor",
        3,
        &activity_actor_xml(second_actor_id, person_id),
    )
    .await;
    let client = MemoryIndexClient::default();

    full_reindex(
        &pool,
        &client,
        &SearchSyncConfig {
            index_name: "opentk_entities".to_owned(),
            categories: vec!["Persoon".to_owned()],
            batch_size: 10,
            max_payload_bytes: 80_000_000,
            retry_limit: 3,
        },
    )
    .await
    .expect("full reindex succeeds");

    let operations = client.operations();
    let person = operations
        .iter()
        .find_map(|operation| match operation {
            SearchIndexOperation::Upsert(document) if document.source_id == person_id => {
                Some(document)
            }
            _ => None,
        })
        .expect("person is indexed");
    assert!(
        person.relation_labels.is_empty(),
        "incoming person backreferences are noisy search fuel and must not be indexed"
    );

    Ok(())
}

#[tokio::test]
async fn transient_index_failure_retries_and_advances_cursor_after_recovery(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_transient_retry").await?;
    seed_document_with_content(&pool, 7, "2026D00007", "retry text", "<p>retry html</p>").await;
    let client = FlakyIndexClient::new(1);
    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };

    let report = full_reindex(&pool, &client, &config)
        .await
        .expect("transient failure recovers");

    assert_eq!(report.indexed, 1);
    assert_eq!(client.apply_attempts(), 2);
    let failures = list_failures(&pool).await?;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].source_category, "Document");
    assert_eq!(failures[0].source_id, document_id());
    assert_eq!(failures[0].latest_skiptoken, 7);
    assert_eq!(failures[0].operation, "upsert");
    assert_eq!(failures[0].attempt_count, 1);
    assert!(
        failures[0]
            .error
            .contains("transient search index failure attempt 1"),
        "failure stores retry evidence"
    );

    let cursor = sqlx::query(
        "SELECT latest_skiptoken, state, last_error
         FROM search_index_cursor
         WHERE index_name = 'opentk_entities' AND source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor.get::<i64, _>("latest_skiptoken"), 7);
    assert_eq!(cursor.get::<String, _>("state"), "caught_up");
    assert_eq!(cursor.get::<Option<String>, _>("last_error"), None);

    Ok(())
}

#[tokio::test]
async fn index_completeness_marks_materially_missing_category_degraded() -> Result<(), sqlx::Error>
{
    let pool = migrated_pool("search_sync_completeness").await?;
    let first_id = document_id();
    let second_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid");
    seed_document_with_content_for(&pool, first_id, 1, "2026D00001", "first", "<p>first</p>").await;
    seed_document_with_content_for(&pool, second_id, 2, "2026D00002", "second", "<p>second</p>")
        .await;
    sqlx::query(
        "INSERT INTO search_index_cursor (
            index_name,
            source_category,
            latest_skiptoken,
            last_indexed_at,
            state
         )
         VALUES ('opentk_entities', 'Document', 2, now() - interval '1 hour', 'caught_up')",
    )
    .execute(&pool)
    .await?;
    let client = CountIndexClient { count: 0 };
    let report = verify_index_completeness(
        &pool,
        &client,
        &SearchSyncConfig {
            index_name: "opentk_entities".to_owned(),
            categories: vec!["Document".to_owned()],
            batch_size: 10,
            max_payload_bytes: 80_000_000,
            retry_limit: 3,
        },
        &SearchCompletenessConfig {
            min_ratio_basis_points: 9_800,
            min_missing_documents: 1,
            grace_period: chrono::Duration::minutes(15),
        },
    )
    .await
    .expect("completeness verification succeeds");

    assert_eq!(report.len(), 1);
    assert_eq!(report[0].postgres_count, 2);
    assert_eq!(report[0].search_count, 0);
    assert_eq!(report[0].missing_count, 2);
    assert_eq!(report[0].state, SearchIndexCompletenessState::Degraded);
    assert!(
        report[0]
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("materially incomplete")),
        "degraded status contains actionable count evidence"
    );

    Ok(())
}

#[tokio::test]
async fn index_failure_is_persisted_retryable_and_does_not_advance_cursor(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("search_sync_failure").await?;
    seed_document_with_content(
        &pool,
        7,
        "2026D00007",
        "failure text",
        "<p>failure html</p>",
    )
    .await;
    let client = FailingIndexClient;
    let config = SearchSyncConfig {
        index_name: "opentk_entities".to_owned(),
        categories: vec!["Document".to_owned()],
        batch_size: 10,
        max_payload_bytes: 80_000_000,
        retry_limit: 3,
    };

    let error = full_reindex(&pool, &client, &config)
        .await
        .expect_err("index failure is returned");
    assert!(error.to_string().contains("search index failed"), "{error}");

    let failures = list_failures(&pool).await?;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].source_category, "Document");
    assert_eq!(failures[0].source_id, document_id());
    assert_eq!(failures[0].latest_skiptoken, 7);
    assert_eq!(failures[0].operation, "upsert");
    assert_eq!(failures[0].attempt_count, 1);
    assert!(
        failures[0].error.contains("fixture batch failure"),
        "failure stores explicit search error"
    );

    let cursor = sqlx::query(
        "SELECT latest_skiptoken, state, last_error
         FROM search_index_cursor
         WHERE index_name = 'opentk_entities' AND source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor.get::<i64, _>("latest_skiptoken"), 0);
    assert_eq!(cursor.get::<String, _>("state"), "error");
    assert!(cursor
        .get::<Option<String>, _>("last_error")
        .expect("last error")
        .contains("fixture batch failure"));

    Ok(())
}

async fn migrated_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL search sync tests");
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

async fn seed_document_with_content(
    pool: &PgPool,
    skiptoken: i64,
    document_number: &str,
    extracted_text: &str,
    extracted_html: &str,
) {
    seed_document_with_content_for(
        pool,
        document_id(),
        skiptoken,
        document_number,
        extracted_text,
        extracted_html,
    )
    .await;
}

async fn seed_document_with_content_for(
    pool: &PgPool,
    source_id: Uuid,
    skiptoken: i64,
    document_number: &str,
    extracted_text: &str,
    extracted_html: &str,
) {
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

    let retrieved_at: DateTime<Utc> = "2026-04-26T02:00:00Z".parse().expect("timestamp");
    let asset_url = Url::parse("https://example.test/document.html").expect("asset URL");
    let text_url = Url::parse("https://example.test/document.txt").expect("text URL");
    let outcome = record_document_asset_fetches(
        pool,
        &[DocumentAssetFetchReport {
            document_source_category: "Document".to_owned(),
            document_source_id: source_id,
            asset_url,
            upstream_url: Url::parse("https://example.test/document.html").expect("upstream URL"),
            upstream_content_type: Some("text/html".to_owned()),
            upstream_content_length: Some(82),
            upstream_last_modified_at: None,
            retrieval_status: RetrievalStatus::Fetched,
            retrieval_error: None,
            retrieved_at,
            source_hash: Some(format!("hash-{source_id}-{document_number}")),
            selected_source: Some(DocumentSelectedSource {
                url: text_url.clone(),
                content_type: Some("text/plain".to_owned()),
                content_length: Some(24),
                kind: DocumentAssetKind::OfficialText,
                official_source: true,
                source_rank: 0,
            }),
            selected_body: Some(extracted_text.as_bytes().to_vec()),
            discovered_sources: Vec::new(),
        }],
    )
    .await
    .expect("asset records");
    let document_asset_id: i64 = sqlx::query_scalar(
        "SELECT id
         FROM document_asset
         WHERE document_source_category = 'Document' AND document_source_id = $1",
    )
    .bind(source_id)
    .fetch_one(pool)
    .await
    .expect("asset id loads");
    assert_eq!(outcome.assets_recorded, 1);
    record_document_content_extractions(
        pool,
        &[DocumentContentExtractionReport {
            document_source_category: "Document".to_owned(),
            document_source_id: source_id,
            document_asset_id: Some(document_asset_id),
            selected_source_url: text_url,
            selected_source_content_type: Some("text/plain".to_owned()),
            selected_source_content_length: Some(24),
            official_source: true,
            source_rank: 0,
            extraction_status: ExtractionStatus::Extracted,
            validation_status: ValidationStatus::Valid,
            extraction_error: None,
            extraction_tool: "test".to_owned(),
            extraction_tool_version: "1".to_owned(),
            source_hash: format!("hash-{source_id}-{document_number}"),
            output_hash: Some(format!("output-{source_id}-{document_number}")),
            extracted_text: Some(extracted_text.to_owned()),
            extracted_html: Some(extracted_html.to_owned()),
            extracted_at: retrieved_at,
        }],
    )
    .await
    .expect("content records");
}

async fn write_document_delete(pool: &PgPool, source_id: Uuid, skiptoken: i64) {
    let deleted = parse_entity_xml(
        "Document",
        &format!(
            r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="{source_id}"
            verwijderd="true"
            bijgewerkt="2026-04-27T00:00:00Z"/>"#
        ),
    )
    .expect("delete marker parses");
    write_sync_page(
        pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: skiptoken,
            next_url: None,
            atom_updated_at: atom_updated_at(),
            entities: vec![deleted],
        },
    )
    .await
    .expect("delete writes");
}

async fn write_parsed_entity(pool: &PgPool, category: &str, skiptoken: i64, xml: &str) {
    let entity = parse_entity_xml(category, xml).expect("entity payload parses");
    write_sync_page(
        pool,
        SyncPageWrite {
            category: category.to_owned(),
            latest_skiptoken: skiptoken,
            next_url: None,
            atom_updated_at: atom_updated_at(),
            entities: vec![entity],
        },
    )
    .await
    .expect("entity writes");
}

fn person_xml(source_id: Uuid) -> String {
    format!(
        r#"<persoon xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="{source_id}"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <nummer>P1</nummer>
            <roepnaam>Ada</roepnaam>
            <achternaam>Lovelace</achternaam>
        </persoon>"#
    )
}

fn activity_xml(source_id: Uuid, commissie_id: Uuid) -> String {
    format!(
        r#"<activiteit xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="{source_id}"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <voortouwcommissie ref="{commissie_id}"/>
            <nummer>A1</nummer>
            <onderwerp>Procedurevergadering</onderwerp>
            <datum>2026-04-26T00:00:00Z</datum>
        </activiteit>"#
    )
}

fn activity_actor_xml(source_id: Uuid, person_id: Uuid) -> String {
    format!(
        r#"<activiteitActor xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="{source_id}"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z">
            <persoon ref="{person_id}"/>
            <actorNaam>Ada Lovelace</actorNaam>
        </activiteitActor>"#
    )
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

fn document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid")
}

#[derive(Default)]
struct MemoryIndexClient {
    reset_calls: Mutex<usize>,
    operations: Mutex<Vec<SearchIndexOperation>>,
}

impl MemoryIndexClient {
    fn reset_called(&self) -> bool {
        *self.reset_calls.lock().expect("reset mutex") > 0
    }

    fn operations(&self) -> Vec<SearchIndexOperation> {
        self.operations.lock().expect("operations mutex").clone()
    }
}

impl SearchIndexClient for MemoryIndexClient {
    async fn reset_index(&self, _schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        *self.reset_calls.lock().expect("reset mutex") += 1;
        self.operations.lock().expect("operations mutex").clear();
        Ok(())
    }

    async fn apply_batch(
        &self,
        operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError> {
        self.operations
            .lock()
            .expect("operations mutex")
            .extend_from_slice(operations);
        Ok(())
    }
}

struct FailingIndexClient;

impl SearchIndexClient for FailingIndexClient {
    async fn reset_index(&self, _schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        Ok(())
    }

    async fn apply_batch(
        &self,
        _operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError> {
        Err(SearchIndexError::InvalidResponse(
            "fixture batch failure".to_owned(),
        ))
    }
}

struct CountIndexClient {
    count: u64,
}

impl SearchCountClient for CountIndexClient {
    fn count<'a>(
        &'a self,
        _filter: SearchFilter,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<u64, SearchIndexError>> + Send + 'a>,
    > {
        Box::pin(async move { Ok(self.count) })
    }
}

struct FlakyIndexClient {
    remaining_failures: Mutex<usize>,
    apply_attempts: Mutex<usize>,
    operations: Mutex<Vec<SearchIndexOperation>>,
}

impl FlakyIndexClient {
    fn new(failures_before_success: usize) -> Self {
        Self {
            remaining_failures: Mutex::new(failures_before_success),
            apply_attempts: Mutex::new(0),
            operations: Mutex::new(Vec::new()),
        }
    }

    fn apply_attempts(&self) -> usize {
        *self.apply_attempts.lock().expect("attempts mutex")
    }
}

impl SearchIndexClient for FlakyIndexClient {
    async fn reset_index(&self, _schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        Ok(())
    }

    async fn apply_batch(
        &self,
        operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError> {
        *self.apply_attempts.lock().expect("attempts mutex") += 1;
        let mut remaining = self
            .remaining_failures
            .lock()
            .expect("remaining failures mutex");
        if *remaining > 0 {
            *remaining -= 1;
            return Err(SearchIndexError::Http {
                status: None,
                message: "fixture connection refused".to_owned(),
            });
        }
        self.operations
            .lock()
            .expect("operations mutex")
            .extend_from_slice(operations);
        Ok(())
    }
}
