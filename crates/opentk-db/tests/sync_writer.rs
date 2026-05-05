use chrono::{DateTime, Utc};
use opentk_core::official_schema;
use opentk_db::{
    schema_lifecycle::ensure_schema,
    sync_writer::{write_sync_page, SyncPageWrite},
};
use opentk_sync::payload::{parse_entity_xml, ParsedEntity, ParsedRelation};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::sync::Arc;
use tokio::sync::Barrier;
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
            next_url: Some("https://example.test/SyncFeed/2.0/Feed?category=Document&skiptoken=42&content=internal".to_owned()),
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
async fn repeated_identical_page_does_not_rewrite_existing_sync_entity_rows(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("writer_idempotent_sync_entity").await?;
    let xml = document_xml("2026D00001", kamerstukdossier_id());

    write_document(&pool, 42, xml.clone()).await;

    let source_before = sync_entity_xmin(&pool, "Document", document_id()).await?;
    let target_before = sync_entity_xmin(&pool, "Kamerstukdossier", kamerstukdossier_id()).await?;

    write_document(&pool, 42, xml).await;

    let source_after = sync_entity_xmin(&pool, "Document", document_id()).await?;
    let target_after = sync_entity_xmin(&pool, "Kamerstukdossier", kamerstukdossier_id()).await?;
    assert_eq!(source_after, source_before);
    assert_eq!(target_after, target_before);

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
            next_url: None,
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
            next_url: None,
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
async fn concurrent_pages_with_reciprocal_relation_targets_do_not_deadlock(
) -> Result<(), sqlx::Error> {
    let pool = migrated_pool("writer_concurrent_relation_locks").await?;
    install_sync_entity_lock_delay(&pool, &[document_id(), related_document_id()]).await?;
    let first = related_document(document_id(), related_document_id(), "2026D00001");
    let second = related_document(related_document_id(), document_id(), "2026D00002");
    let start = Arc::new(Barrier::new(2));

    let first_write = tokio::spawn(write_after_barrier(pool.clone(), start.clone(), first, 101));
    let second_write = tokio::spawn(write_after_barrier(pool.clone(), start, second, 102));

    let first_result = first_write.await.expect("first writer task joins");
    let second_result = second_write.await.expect("second writer task joins");

    first_result.expect("first reciprocal page writes without deadlock");
    second_result.expect("second reciprocal page writes without deadlock");
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
                next_url: None,
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
            next_url: None,
            atom_updated_at: atom_updated_at(),
            entities: vec![document],
        },
    )
    .await
    .expect("document writes");
}

async fn write_after_barrier(
    pool: PgPool,
    start: Arc<Barrier>,
    entity: ParsedEntity,
    skiptoken: i64,
) -> Result<(), String> {
    start.wait().await;
    write_sync_page(
        &pool,
        SyncPageWrite {
            category: "Document".to_owned(),
            latest_skiptoken: skiptoken,
            next_url: None,
            atom_updated_at: atom_updated_at(),
            entities: vec![entity],
        },
    )
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

async fn sync_entity_xmin(
    pool: &PgPool,
    source_category: &str,
    source_id: Uuid,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT xmin::text
         FROM sync_entity
         WHERE source_category = $1 AND source_id = $2",
    )
    .bind(source_category)
    .bind(source_id)
    .fetch_one(pool)
    .await
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
        .max_connections(4)
        .connect(&with_search_path(&database_url, &schema_name))
        .await?;
    ensure_schema(&pool)
        .await
        .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
    Ok(pool)
}

async fn install_sync_entity_lock_delay(
    pool: &PgPool,
    source_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    let id_list = source_ids
        .iter()
        .map(|source_id| format!("'{source_id}'::uuid"))
        .collect::<Vec<_>>()
        .join(", ");
    let function_sql = format!(
        r"
        CREATE OR REPLACE FUNCTION delay_selected_sync_entity_locks()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
          IF NEW.source_category = 'Document'
             AND NEW.source_id IN ({id_list}) THEN
            PERFORM pg_sleep(0.1);
          END IF;
          RETURN NEW;
        END;
        $$;
        "
    );
    sqlx::query(&function_sql).execute(pool).await?;
    sqlx::query(
        r"
        CREATE TRIGGER delay_selected_sync_entity_locks
        BEFORE INSERT OR UPDATE ON sync_entity
        FOR EACH ROW EXECUTE FUNCTION delay_selected_sync_entity_locks()
        ",
    )
    .execute(pool)
    .await?;
    Ok(())
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

fn related_document(source_id: Uuid, relation_id: Uuid, document_nummer: &str) -> ParsedEntity {
    let mut entity = parse_entity_xml(
        "Document",
        &document_xml_with_id(source_id, document_nummer),
    )
    .expect("document payload parses");
    entity.relations.push(ParsedRelation {
        name: "bronDocument".to_owned(),
        target_category: "Document".to_owned(),
        target_id: relation_id,
        target_updated_at: None,
        ordinal: 0,
    });
    entity
}

fn document_xml_with_id(source_id: Uuid, document_nummer: &str) -> String {
    format!(
        r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
            id="{source_id}"
            verwijderd="false"
            bijgewerkt="2026-04-26T00:00:00Z"
            contentType="application/pdf"
            contentLength="12345">
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

fn related_document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111112").expect("valid uuid")
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
