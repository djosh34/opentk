use chrono::{DateTime, Utc};
use opentk_core::official_schema;
use opentk_db::sync_writer::{write_sync_page, SyncPageWrite};
use opentk_sync::payload::parse_entity_xml;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

#[tokio::test]
async fn writes_document_page_with_scalar_relation_asset_and_cursor() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("writer_document").await?;
    let document = parse_entity_xml(
        "Document",
        include_str!("../../opentk-sync/tests/fixtures/syncfeed_payloads/document.xml"),
    )
    .expect("document payload parses");

    let outcome = write_sync_page(
        &pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: 42,
            atom_updated_at: atom_updated_at(),
            entities: vec![document],
        },
    )
    .await
    .expect("page writes");

    assert_eq!(outcome.entities_written, 1);
    assert_eq!(outcome.relations_written, 1);
    let sync = sqlx::query(
        "SELECT deleted, latest_skiptoken FROM sync_entity
         WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert!(!sync.get::<bool, _>("deleted"));
    assert_eq!(sync.get::<i64, _>("latest_skiptoken"), 42);

    let document = sqlx::query(
        "SELECT document_nummer, content_type, content_length, enclosure_url FROM document
         WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        document
            .get::<Option<String>, _>("document_nummer")
            .as_deref(),
        Some("2026D00001")
    );
    assert_eq!(
        document.get::<Option<String>, _>("content_type").as_deref(),
        Some("application/pdf")
    );
    assert_eq!(
        document.get::<Option<i32>, _>("content_length"),
        Some(12345)
    );

    let relation_count: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM document__kamerstukdossier
         WHERE source_category = 'Document'
           AND source_id = $1
           AND target_category = 'Kamerstukdossier'
           AND target_id = $2",
    )
    .bind(document_id())
    .bind(kamerstukdossier_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(relation_count, 1);

    let cursor: i64 = sqlx::query_scalar(
        "SELECT latest_skiptoken FROM sync_category WHERE source_category = 'Document'",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(cursor, 42);
    Ok(())
}

#[tokio::test]
async fn updated_document_replaces_current_scalar_and_relations() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("writer_update").await?;
    write_document(&pool, 1, document_xml("2026D00001", kamerstukdossier_id())).await;
    let replacement_relation =
        Uuid::parse_str("22222222-2222-4222-8222-222222222223").expect("valid uuid");
    write_document(&pool, 2, document_xml("2026D00002", replacement_relation)).await;

    let document_number: Option<String> = sqlx::query_scalar(
        "SELECT document_nummer FROM document WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(document_number.as_deref(), Some("2026D00002"));

    let rows = sqlx::query(
        "SELECT target_id FROM document__kamerstukdossier
         WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_all(&pool)
    .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<Uuid, _>("target_id"), replacement_relation);
    Ok(())
}

#[tokio::test]
async fn delete_marker_records_delete_and_removes_current_rows() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("writer_delete").await?;
    write_document(&pool, 1, document_xml("2026D00001", kamerstukdossier_id())).await;
    let deleted = parse_entity_xml(
        "Document",
        r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="11111111-1111-4111-8111-111111111111"
            verwijderd="true"
            bijgewerkt="2026-04-27T00:00:00Z"/>"#,
    )
    .expect("delete marker parses");

    write_sync_page(
        &pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: 2,
            atom_updated_at: atom_updated_at(),
            entities: vec![deleted],
        },
    )
    .await
    .expect("delete writes");

    let deleted: bool = sqlx::query_scalar(
        "SELECT deleted FROM sync_entity WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert!(deleted);
    let current_rows: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM document WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(current_rows, 0);
    let relation_rows: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM document__kamerstukdossier WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(document_id())
    .fetch_one(&pool)
    .await?;
    assert_eq!(relation_rows, 0);
    Ok(())
}

#[tokio::test]
async fn page_write_rolls_back_when_later_entity_fails() -> Result<(), sqlx::Error> {
    let pool = migrated_pool("writer_rollback").await?;
    let good = parse_entity_xml(
        "Document",
        &document_xml("2026D00001", kamerstukdossier_id()),
    )
    .expect("document payload parses");
    let mut bad = parse_entity_xml(
        "Document",
        &document_xml("2026D00002", kamerstukdossier_id()),
    )
    .expect("document payload parses");
    bad.category = "Zaak".to_owned();

    let error = write_sync_page(
        &pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: 99,
            atom_updated_at: atom_updated_at(),
            entities: vec![good, bad],
        },
    )
    .await
    .expect_err("page fails");
    assert!(error.to_string().contains("category mismatch"), "{error}");

    let rows: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM sync_entity")
        .fetch_one(&pool)
        .await?;
    assert_eq!(rows, 0);
    let cursors: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM sync_category")
        .fetch_one(&pool)
        .await?;
    assert_eq!(cursors, 0);
    Ok(())
}

#[tokio::test]
async fn every_official_category_can_round_trip_a_minimal_current_entity() -> Result<(), sqlx::Error>
{
    let pool = migrated_pool("writer_all_categories").await?;

    for (index, entity) in official_schema::entity_types().iter().enumerate() {
        let source_id = Uuid::from_u128(0xaaaaaaaa_aaaa_4aaa_8aaa_000000000000 + index as u128);
        let xml = format!(
            r#"<{root} xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0" id="{source_id}" verwijderd="false" bijgewerkt="2026-04-26T00:00:00Z"/>"#,
            root = entity.xml_element
        );
        let parsed = parse_entity_xml(entity.category, &xml)
            .unwrap_or_else(|error| panic!("{} payload parses: {error}", entity.category));

        write_sync_page(
            &pool,
            SyncPageWrite {
                category: entity.category.to_owned(),
                latest_skiptoken: i64::try_from(index + 1).expect("category index fits i64"),
                atom_updated_at: atom_updated_at(),
                entities: vec![parsed],
            },
        )
        .await
        .unwrap_or_else(|error| panic!("{} page writes: {error}", entity.category));
    }

    let current_rows: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM sync_entity")
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        current_rows,
        i64::try_from(official_schema::entity_types().len()).expect("entity count fits i64")
    );
    Ok(())
}

async fn write_document(pool: &PgPool, skiptoken: i64, xml: String) {
    let document = parse_entity_xml("Document", &xml).expect("document payload parses");
    write_sync_page(
        pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: skiptoken,
            atom_updated_at: atom_updated_at(),
            entities: vec![document],
        },
    )
    .await
    .expect("document writes");
}

async fn migrated_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL writer tests");
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
            <onderwerp>Writer task</onderwerp>
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
