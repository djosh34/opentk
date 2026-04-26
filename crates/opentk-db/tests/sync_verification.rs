use std::{num::NonZeroUsize, time::Duration};

use chrono::{DateTime, Utc};
use opentk_core::official_schema;
use opentk_db::{
    sync_verification::{verify_sync_database, SyncVerificationConfig},
    sync_writer::{write_sync_page, SyncPageWrite},
};
use opentk_sync::{
    payload::{parse_entity_xml, parse_entry_payload},
    runner::{skiptoken_from_url, CategorySyncState},
    syncfeed::{SyncFeedClient, SyncFeedClientConfig, SyncFeedContentMode, SyncFeedCursor},
};
use reqwest::Url;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[tokio::test]
async fn verifies_seeded_document_category_and_direct_query_evidence() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("verify_document").await?;
    write_document(&pool, 42, document_xml("2026D00001", kamerstukdossier_id())).await;

    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: vec!["Document".to_owned()],
            required_relation_samples: 1,
        },
    )
    .await
    .expect("database verifies");

    let document = report
        .categories
        .iter()
        .find(|category| category.category == "Document")
        .expect("Document category is reported");
    assert_eq!(document.table_name, "document");
    assert_eq!(document.current_rows, 1);
    assert_eq!(document.registry_rows, 1);
    assert_eq!(document.latest_skiptoken, Some(42));
    assert_eq!(document.state, CategorySyncState::Running);

    let document_number = report
        .direct_queries
        .iter()
        .find(|query| query.name == "document_by_document_nummer")
        .expect("document number query evidence is reported");
    assert_eq!(document_number.rows, 1);
    Ok(())
}

#[tokio::test]
async fn verifies_every_official_category_has_schema_and_table_evidence() -> Result<(), sqlx::Error>
{
    let pool = migrated_pool("verify_all_categories").await?;
    seed_minimal_official_entities(&pool).await;

    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: Vec::new(),
            required_relation_samples: 0,
        },
    )
    .await
    .expect("database verifies");

    assert_eq!(
        report.categories.len(),
        official_schema::entity_types().len(),
        "every official entity category is reported"
    );
    for entity in official_schema::entity_types() {
        let category = report
            .categories
            .iter()
            .find(|category| category.category == entity.category)
            .unwrap_or_else(|| panic!("{} is reported", entity.category));
        assert_eq!(category.current_rows, 1, "{} current row", entity.category);
        assert_eq!(
            category.registry_rows, 1,
            "{} registry row",
            entity.category
        );
        assert!(
            category.latest_skiptoken.is_some(),
            "{} cursor",
            entity.category
        );
    }
    Ok(())
}

#[tokio::test]
async fn verifies_every_official_relation_table_is_queryable() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("verify_relations").await?;
    write_document(&pool, 42, document_xml("2026D00001", kamerstukdossier_id())).await;

    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: Vec::new(),
            required_relation_samples: 0,
        },
    )
    .await
    .expect("database verifies");

    let official_relation_count = official_schema::entity_types()
        .iter()
        .flat_map(|entity| entity.fields)
        .filter(|field| {
            matches!(
                field.kind,
                opentk_core::official_schema::FieldKind::Relation
            )
        })
        .count();
    assert_eq!(report.relation_tables.len(), official_relation_count);
    for relation in &report.relation_tables {
        assert!(
            relation.queryable_from_source,
            "{} source lookup is indexed",
            relation.table_name
        );
        assert!(
            relation.queryable_from_target,
            "{} target lookup is indexed",
            relation.table_name
        );
    }
    let document_relation = report
        .relation_tables
        .iter()
        .find(|relation| relation.table_name == "document__kamerstukdossier")
        .expect("Document Kamerstukdossier relation is reported");
    assert_eq!(document_relation.rows, 1);
    Ok(())
}

#[tokio::test]
async fn verification_snapshots_prove_reapplying_pages_is_idempotent() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("verify_idempotent").await?;
    let xml = document_xml("2026D00001", kamerstukdossier_id());
    write_document(&pool, 42, xml.clone()).await;
    let first = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: vec!["Document".to_owned()],
            required_relation_samples: 1,
        },
    )
    .await
    .expect("first verification succeeds");

    write_document(&pool, 42, xml).await;
    let second = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: vec!["Document".to_owned()],
            required_relation_samples: 1,
        },
    )
    .await
    .expect("second verification succeeds");

    assert_eq!(
        snapshot_rows(&first, "document"),
        snapshot_rows(&second, "document")
    );
    assert_eq!(
        snapshot_rows(&first, "document__kamerstukdossier"),
        snapshot_rows(&second, "document__kamerstukdossier")
    );
    assert_eq!(
        snapshot_rows(&first, "sync_entity"),
        snapshot_rows(&second, "sync_entity")
    );
    assert_eq!(
        snapshot_rows(&first, "sync_category"),
        snapshot_rows(&second, "sync_category")
    );
    Ok(())
}

#[tokio::test]
async fn update_replacement_verification_uses_latest_current_rows() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("verify_update").await?;
    write_document(&pool, 1, document_xml("2026D00001", kamerstukdossier_id())).await;
    let replacement_relation =
        Uuid::parse_str("22222222-2222-4222-8222-222222222223").expect("valid uuid");
    write_document(&pool, 2, document_xml("2026D00002", replacement_relation)).await;

    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: vec!["Document".to_owned()],
            required_relation_samples: 1,
        },
    )
    .await
    .expect("database verifies");
    assert_eq!(snapshot_rows(&report, "document"), 1);
    assert_eq!(snapshot_rows(&report, "document__kamerstukdossier"), 1);

    let document_number: Option<String> = sqlx::query_scalar(
        "SELECT document_nummer FROM document WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(document_number.as_deref(), Some("2026D00002"));
    let target_id: Uuid = sqlx::query_scalar(
        "SELECT target_id FROM document__kamerstukdossier WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(target_id, replacement_relation);
    Ok(())
}

#[tokio::test]
async fn verification_records_storage_and_asset_metadata_evidence() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("verify_storage").await?;
    write_document(&pool, 42, document_xml("2026D00001", kamerstukdossier_id())).await;

    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories: vec!["Document".to_owned()],
            required_relation_samples: 1,
        },
    )
    .await
    .expect("database verifies");

    assert!(report.storage.table_bytes > 0);
    assert!(report.storage.index_bytes > 0);
    assert_eq!(report.storage.html_asset_bytes, 0);
    assert_eq!(report.storage.binary_asset_metadata_rows, 1);
    Ok(())
}

#[tokio::test]
#[ignore = "live SyncFeed smoke for make test-long"]
async fn live_syncfeed_page_can_be_written_and_verified() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = migrated_pool("verify_live").await?;
    let base_url = std::env::var("OPENTK_LIVE_SYNC_BASE_URL")
        .unwrap_or_else(|_| "https://gegevensmagazijn.tweedekamer.nl".to_owned())
        .parse::<Url>()?;
    let categories = std::env::var("OPENTK_LIVE_SYNC_CATEGORIES")
        .unwrap_or_else(|_| "Document".to_owned())
        .split(',')
        .map(str::trim)
        .filter(|category| !category.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    assert!(
        !categories.is_empty(),
        "OPENTK_LIVE_SYNC_CATEGORIES must name at least one category"
    );

    let client = SyncFeedClient::new(SyncFeedClientConfig {
        base_url: base_url.clone(),
        content_mode: SyncFeedContentMode::Internal,
        request_timeout: Duration::from_secs(30),
        connect_timeout: Duration::from_secs(10),
        max_retries: 2,
        initial_retry_delay: Duration::from_millis(500),
        max_retry_delay: Duration::from_secs(5),
        max_concurrent_requests: NonZeroUsize::new(2).expect("non-zero"),
    })?;

    for category in &categories {
        let page = client
            .fetch_page(SyncFeedCursor::first_page(
                &base_url,
                category.clone(),
                SyncFeedContentMode::Internal,
            ))
            .await?;
        let next = page
            .next_request
            .as_ref()
            .or(page.resume.as_ref())
            .ok_or_else(|| format!("live {category} page had no next or resume cursor"))?;
        let latest_skiptoken = skiptoken_from_url(&next.url)
            .ok_or_else(|| format!("live {category} cursor had no skiptoken"))?;
        let entities = page
            .entries
            .iter()
            .map(parse_entry_payload)
            .collect::<Result<Vec<_>, _>>()?;
        assert!(
            !entities.is_empty(),
            "live bounded smoke expected at least one {category} entity"
        );
        write_sync_page(
            &pool,
            SyncPageWrite {
                category: category.clone(),
                latest_skiptoken,
                next_url: Some(next.url.to_string()),
                atom_updated_at: Utc::now(),
                entities,
            },
        )
        .await?;
    }

    let report = verify_sync_database(
        &pool,
        SyncVerificationConfig {
            categories,
            required_relation_samples: 0,
        },
    )
    .await?;
    assert!(report
        .categories
        .iter()
        .all(|category| category.registry_rows > 0 && category.latest_skiptoken.is_some()));
    Ok(())
}

async fn write_document(pool: &PgPool, skiptoken: i64, xml: String) {
    let document = parse_entity_xml("Document", &xml).expect("document payload parses");
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

async fn seed_minimal_official_entities(pool: &PgPool) {
    for (index, entity) in official_schema::entity_types().iter().enumerate() {
        let source_id = Uuid::from_u128(0xaaaaaaaa_aaaa_4aaa_8aaa_000000000000 + index as u128);
        let xml = format!(
            r#"<{root} xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0" id="{source_id}" verwijderd="false" bijgewerkt="2026-04-26T00:00:00Z"/>"#,
            root = entity.xml_element
        );
        let parsed = parse_entity_xml(entity.category, &xml)
            .unwrap_or_else(|error| panic!("{} payload parses: {error}", entity.category));
        write_sync_page(
            pool,
            SyncPageWrite {
                category: entity.category.to_owned(),
                latest_skiptoken: i64::try_from(index + 1).expect("category index fits i64"),
                next_url: None,
                atom_updated_at: atom_updated_at(),
                entities: vec![parsed],
            },
        )
        .await
        .unwrap_or_else(|error| panic!("{} page writes: {error}", entity.category));
    }
}

fn snapshot_rows(
    report: &opentk_db::sync_verification::SyncVerificationReport,
    table_name: &str,
) -> i64 {
    report
        .table_snapshots
        .iter()
        .find(|snapshot| snapshot.table_name == table_name)
        .unwrap_or_else(|| panic!("{table_name} snapshot exists"))
        .rows
}

async fn migrated_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect(
            "set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL verification tests",
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

fn document_xml(document_nummer: &str, relation_id: Uuid) -> String {
    format!(
        r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="11111111-1111-4111-8111-111111111111"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z"
            contentType="application/pdf"
            contentLength="12345">
            <kamerstukdossier ref="{relation_id}"/>
            <documentNummer>{document_nummer}</documentNummer>
            <onderwerp>Verification task</onderwerp>
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

fn kamerstukdossier_id() -> Uuid {
    Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid")
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
