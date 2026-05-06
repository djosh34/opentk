use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use opentk_db::{connect, schema_lifecycle::ensure_schema, DatabaseConfig};
use opentk_search::{
    SearchCountClient, SearchFilter, SearchHealthClient, SearchIndexError, SearchQueryClient,
    SearchRequest, SearchResponse,
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::{collections::VecDeque, future::Future, pin::Pin, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
};
use tower::ServiceExt;
use uuid::Uuid;

pub struct TestServer {
    pub base_url: String,
    pub client: reqwest::Client,
    task: JoinHandle<Result<(), std::io::Error>>,
}

#[derive(Clone, Debug)]
pub struct FixtureResponse {
    target: String,
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl FixtureResponse {
    pub fn ok(target: &str, content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            target: target.to_owned(),
            status: 200,
            content_type,
            body: body.into(),
        }
    }

    pub fn status(target: &str, status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            target: target.to_owned(),
            status,
            content_type: "text/plain",
            body: body.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FixtureAssetServer {
    pub base_url: String,
    responses: Arc<Mutex<VecDeque<FixtureResponse>>>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FixtureAssetServer {
    pub async fn start(responses: Vec<FixtureResponse>) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_responses = Arc::clone(&responses);
        let server_requests = Arc::clone(&requests);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    return;
                };
                let responses = Arc::clone(&server_responses);
                let requests = Arc::clone(&server_requests);
                tokio::spawn(async move {
                    let mut buffer = [0_u8; 4096];
                    let bytes_read = stream.read(&mut buffer).await.expect("read request");
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let target = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .expect("request target")
                        .to_owned();
                    requests.lock().await.push(target.clone());
                    let response = {
                        let mut responses = responses.lock().await;
                        let index = responses
                            .iter()
                            .position(|response| response.target == target)
                            .expect("matching fixture response");
                        responses.remove(index).expect("response exists")
                    };
                    let header = format!(
                        "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {content_length}\r\nconnection: close\r\n\r\n",
                        status = response.status,
                        content_type = response.content_type,
                        content_length = response.body.len(),
                    );
                    stream
                        .write_all(header.as_bytes())
                        .await
                        .expect("write response header");
                    stream
                        .write_all(&response.body)
                        .await
                        .expect("write response body");
                });
            }
        });
        Ok(Self {
            base_url: format!("http://{address}"),
            responses,
            requests,
        })
    }

    pub async fn requests(&self) -> Vec<String> {
        self.requests.lock().await.clone()
    }

    pub async fn assert_consumed(&self) {
        let remaining = self.responses.lock().await.len();
        assert_eq!(remaining, 0, "all fixture responses must be consumed");
    }
}

impl TestServer {
    pub async fn get_json(
        &self,
        path: &str,
        expected_status: StatusCode,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await?;
        assert_eq!(response.status(), expected_status);
        Ok(serde_json::from_str(&response.text().await?)?)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn start_server(pool: PgPool) -> Result<TestServer, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let task =
        tokio::spawn(async move { axum::serve(listener, router_without_search(pool)).await });

    Ok(TestServer {
        base_url: format!("http://{address}"),
        client: reqwest::Client::new(),
        task,
    })
}

pub async fn router_json(
    pool: PgPool,
    path: &str,
    expected_status: StatusCode,
) -> Result<Value, Box<dyn std::error::Error>> {
    let response = router_without_search(pool)
        .oneshot(Request::get(path).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), expected_status);
    response_json(response).await
}

pub async fn router_json_with_public_limit(
    pool: PgPool,
    path: &str,
    expected_status: StatusCode,
    max_public_query_limit: u32,
) -> Result<Value, Box<dyn std::error::Error>> {
    let response = router_without_search_with_public_limit(pool, max_public_query_limit)
        .oneshot(Request::get(path).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), expected_status);
    response_json(response).await
}

pub fn router_without_search(pool: PgPool) -> Router {
    opentk_api::router_with_search(pool, Arc::new(UnavailableSearchClient))
}

pub fn router_without_search_with_public_limit(
    pool: PgPool,
    max_public_query_limit: u32,
) -> Router {
    opentk_api::router_with_search_and_public_limit(
        pool,
        Arc::new(UnavailableSearchClient),
        max_public_query_limit,
    )
}

struct UnavailableSearchClient;

impl SearchQueryClient for UnavailableSearchClient {
    fn search<'a>(
        &'a self,
        _request: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search unavailable in this test".to_owned(),
            })
        })
    }
}

impl SearchHealthClient for UnavailableSearchClient {
    fn health<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<(), SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search unavailable in this test".to_owned(),
            })
        })
    }
}

impl SearchCountClient for UnavailableSearchClient {
    fn count<'a>(
        &'a self,
        _filter: SearchFilter,
    ) -> Pin<Box<dyn Future<Output = Result<u64, SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search unavailable in this test".to_owned(),
            })
        })
    }
}

pub async fn migrated_pool(test_name: &str) -> Result<PgPool, Box<dyn std::error::Error>> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run API tests");
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

    let pool = connect(&DatabaseConfig {
        url: with_search_path(&database_url, &schema_name),
        max_connections: 4,
    })
    .await?;
    ensure_schema(&pool).await?;
    Ok(pool)
}

pub async fn seed_deep_fixture(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sync_category
         (source_category, latest_skiptoken, state, last_fetch_at, last_synced_at, caught_up_at, next_url, resume_url)
         VALUES
         ('Document', 43, 'running', '2026-04-26T10:00:00Z', '2026-04-26T10:01:00Z', NULL, 'https://example.test/document/next', 'https://example.test/document/resume'),
         ('Activiteit', 41, 'caught_up', '2026-04-26T11:00:00Z', '2026-04-26T11:01:00Z', '2026-04-26T11:02:00Z', NULL, NULL),
         ('Persoon', 42, 'caught_up', '2026-04-26T12:00:00Z', '2026-04-26T12:01:00Z', '2026-04-26T12:02:00Z', NULL, NULL)",
    )
    .execute(pool)
    .await?;

    insert_document(pool).await?;
    insert_second_document(pool).await?;
    insert_third_document(pool).await?;
    insert_activity(pool).await?;
    insert_person(pool).await?;
    insert_document_version(pool).await?;
    insert_document_activity_relation(pool).await?;
    insert_document_version_relation(pool).await?;
    Ok(())
}

pub async fn insert_document_with_asset_metadata(
    pool: &PgPool,
    source_id: Uuid,
    skiptoken: i64,
    content_type: &str,
    content_length: i32,
    enclosure_url: &str,
) -> Result<(), sqlx::Error> {
    insert_sync_entity(pool, "Document", source_id, skiptoken).await?;
    sqlx::query(
        "INSERT INTO document
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at,
          content_type, content_length, enclosure_url, document_nummer, titel)
         VALUES
         ('Document', $1, $2, false, '2026-04-26T12:00:00Z', '2026-04-26T12:01:00Z',
          $3, $4, $5, $6, 'Deep content fixture')",
    )
    .bind(source_id)
    .bind(skiptoken)
    .bind(content_type)
    .bind(content_length)
    .bind(enclosure_url)
    .bind(format!("2026D{skiptoken:05}"))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_sync_entity(
    pool: &PgPool,
    category: &str,
    source_id: Uuid,
    skiptoken: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sync_entity
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at)
         VALUES ($1, $2, $3, false, '2026-04-26T12:00:00Z', '2026-04-26T12:01:00Z')",
    )
    .bind(category)
    .bind(source_id)
    .bind(skiptoken)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_document(pool: &PgPool) -> Result<(), sqlx::Error> {
    insert_sync_entity(pool, "Document", document_id(), 40).await?;
    sqlx::query(
        "INSERT INTO document
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at,
          content_type, content_length, enclosure_url, document_nummer, titel, onderwerp, datum)
         VALUES
         ('Document', $1, 40, false, '2026-04-26T12:00:00Z', '2026-04-26T12:01:00Z',
          'application/pdf', 12345, 'https://example.test/document.pdf', '2026D00001',
          'Fixture document', 'Read endpoint', '2026-04-26T00:00:00Z')",
    )
    .bind(document_id())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_activity(pool: &PgPool) -> Result<(), sqlx::Error> {
    insert_sync_entity(pool, "Activiteit", activity_id(), 41).await?;
    sqlx::query(
        "INSERT INTO activiteit
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at,
          soort, nummer, onderwerp, datum, aanvangstijd, locatie, status)
         VALUES
         ('Activiteit', $1, 41, false, '2026-04-26T12:00:00Z', '2026-04-26T12:01:00Z',
          'Debat', 'A-1', 'Fixture activity', '2026-04-26T13:00:00Z',
          '2026-04-26T13:30:00Z', 'Plenaire zaal', 'Gepland')",
    )
    .bind(activity_id())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_person(pool: &PgPool) -> Result<(), sqlx::Error> {
    insert_sync_entity(pool, "Persoon", person_id(), 42).await?;
    sqlx::query(
        "INSERT INTO persoon
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at,
          nummer, titels, initialen, achternaam, tussenvoegsel, roepnaam)
         VALUES
         ('Persoon', $1, 42, false, '2026-04-26T12:00:00Z', '2026-04-26T12:01:00Z',
          'P-1', 'dr.', 'J.', 'Jansen', 'van', 'Jan')",
    )
    .bind(person_id())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn document_source_row(pool: &PgPool, id: Uuid) -> Result<Value, sqlx::Error> {
    let row = sqlx::query(
        "SELECT source_category, source_id, latest_skiptoken, deleted,
                to_char(source_updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"+00:00\"') AS source_updated_at,
                to_char(atom_updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"+00:00\"') AS atom_updated_at,
                content_type, content_length, enclosure_url, document_nummer, titel
         FROM document
         WHERE source_category = 'Document' AND source_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(serde_json::json!({
        "metadata": {
            "category": row.get::<String, _>("source_category"),
            "source_id": row.get::<Uuid, _>("source_id").to_string(),
            "latest_skiptoken": row.get::<i64, _>("latest_skiptoken"),
            "deleted": row.get::<bool, _>("deleted"),
            "source_updated_at": row.get::<String, _>("source_updated_at"),
            "atom_updated_at": row.get::<String, _>("atom_updated_at"),
        },
        "fields": {
            "content_type": row.get::<Option<String>, _>("content_type"),
            "content_length": row.get::<Option<i32>, _>("content_length"),
            "enclosure_url": row.get::<Option<String>, _>("enclosure_url"),
            "document_nummer": row.get::<Option<String>, _>("document_nummer"),
            "titel": row.get::<Option<String>, _>("titel"),
        }
    }))
}

pub fn category<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["categories"]
        .as_array()
        .expect("categories")
        .iter()
        .find(|category| category["category"] == name)
        .unwrap_or_else(|| panic!("missing category {name}"))
}

pub fn document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("valid uuid")
}

pub fn second_document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111112").expect("valid uuid")
}

pub fn third_document_id() -> Uuid {
    Uuid::parse_str("11111111-1111-4111-8111-111111111113").expect("valid uuid")
}

pub fn activity_id() -> Uuid {
    Uuid::parse_str("22222222-2222-4222-8222-222222222222").expect("valid uuid")
}

pub fn person_id() -> Uuid {
    Uuid::parse_str("33333333-3333-4333-8333-333333333333").expect("valid uuid")
}

pub fn document_version_id() -> Uuid {
    Uuid::parse_str("44444444-4444-4444-8444-444444444444").expect("valid uuid")
}

async fn response_json(
    response: axum::response::Response,
) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX).await?,
    )?)
}

async fn insert_second_document(pool: &PgPool) -> Result<(), sqlx::Error> {
    insert_sync_entity(pool, "Document", second_document_id(), 41).await?;
    sqlx::query(
        "INSERT INTO document
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at,
          document_nummer, titel)
         VALUES
         ('Document', $1, 41, false, '2026-04-26T12:10:00Z', '2026-04-26T12:11:00Z',
          '2026D00002', 'Second fixture document')",
    )
    .bind(second_document_id())
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_third_document(pool: &PgPool) -> Result<(), sqlx::Error> {
    insert_sync_entity(pool, "Document", third_document_id(), 43).await?;
    sqlx::query(
        "INSERT INTO document
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at,
          document_nummer, titel)
         VALUES
         ('Document', $1, 43, false, '2026-04-26T12:20:00Z', '2026-04-26T12:21:00Z',
          '2026D00003', 'Third fixture document')",
    )
    .bind(third_document_id())
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_document_version(pool: &PgPool) -> Result<(), sqlx::Error> {
    insert_sync_entity(pool, "DocumentVersie", document_version_id(), 44).await?;
    sqlx::query(
        "INSERT INTO document_versie
         (source_category, source_id, latest_skiptoken, deleted, source_updated_at, atom_updated_at,
          status, versienummer)
         VALUES
         ('DocumentVersie', $1, 44, false, '2026-04-26T12:30:00Z', '2026-04-26T12:31:00Z',
          'Vastgesteld', '1')",
    )
    .bind(document_version_id())
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_document_activity_relation(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO document__activiteit
         (source_category, source_id, relation_name, target_category, target_id, ordinal, source_updated_at)
         VALUES ('Document', $1, 'activiteit', 'Activiteit', $2, 0, '2026-04-26T12:02:00Z')",
    )
    .bind(document_id())
    .bind(activity_id())
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_document_version_relation(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO document_versie__document
         (source_category, source_id, relation_name, target_category, target_id, ordinal, source_updated_at)
         VALUES ('DocumentVersie', $1, 'document', 'Document', $2, 0, '2026-04-26T12:32:00Z')",
    )
    .bind(document_version_id())
    .bind(document_id())
    .execute(pool)
    .await?;
    Ok(())
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
