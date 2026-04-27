use std::{
    net::SocketAddr,
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use opentk_config::Config;
use opentk_db::{
    schema_lifecycle::ensure_schema,
    startup_validation::{
        redact_database_url, validate_api_dependencies, validate_search_sync_dependencies,
        validate_sync_dependencies, DependencyCheckStatus, DependencyKind, SearchRequirement,
    },
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::test]
async fn database_validation_runs_select_one_against_reachable_database(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("api_valid_schema").await?;
    ensure_schema(&pool).await?;
    let database_url = current_database_url(&pool).await?;
    pool.close().await;
    let config = config_with_database(database_url, "http://127.0.0.1:1", None)?;

    let report = validate_api_dependencies(&config, SearchRequirement::Optional).await?;

    assert_eq!(
        report.database,
        DependencyCheckStatus::Reachable {
            dependency: DependencyKind::Database,
            target: config.database.url.clone(),
        }
    );
    assert!(matches!(
        report.search,
        Some(DependencyCheckStatus::Degraded {
            dependency: DependencyKind::Meilisearch,
            ..
        })
    ));
    Ok(())
}

#[tokio::test]
async fn api_validation_fails_when_database_schema_is_missing(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("api_missing_schema").await?;
    let database_url = current_database_url(&pool).await?;
    pool.close().await;
    let config = config_with_database(database_url, "http://127.0.0.1:1", None)?;

    let error = validate_api_dependencies(&config, SearchRequirement::Optional)
        .await
        .expect_err("missing schema is fatal for API startup");

    assert!(
        error.to_string().contains("has incompatible schema"),
        "{error}"
    );
    assert!(error.to_string().contains("missing table"), "{error}");
    Ok(())
}

#[tokio::test]
async fn api_validation_fails_when_database_schema_is_incompatible(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = empty_pool("api_incompatible_schema").await?;
    sqlx::query("CREATE TABLE sync_category (source_category integer NOT NULL)")
        .execute(&pool)
        .await?;
    let database_url = current_database_url(&pool).await?;
    pool.close().await;
    let config = config_with_database(database_url, "http://127.0.0.1:1", None)?;

    let error = validate_api_dependencies(&config, SearchRequirement::Optional)
        .await
        .expect_err("incompatible schema is fatal for API startup");

    assert!(
        error.to_string().contains("has incompatible schema"),
        "{error}"
    );
    assert!(error.to_string().contains("sync_category"), "{error}");
    assert!(error.to_string().contains("source_category"), "{error}");
    Ok(())
}

#[test]
fn database_url_redaction_masks_password() -> Result<(), Box<dyn std::error::Error>> {
    let redacted = redact_database_url("postgres://postgres:secret@127.0.0.1:1/opentk")?;

    assert_eq!(redacted, "postgres://postgres:***@127.0.0.1:1/opentk");
    Ok(())
}

#[tokio::test]
async fn database_validation_returns_human_readable_unreachable_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let config = config_with_database(
        "postgres://postgres@127.0.0.1:1/opentk",
        "http://127.0.0.1:1",
        None,
    )?;

    let error = validate_api_dependencies(&config, SearchRequirement::Optional)
        .await
        .expect_err("database failure is fatal");

    assert!(
        error
            .to_string()
            .starts_with("Database at postgres://postgres@127.0.0.1:1/opentk is unreachable:"),
        "{error}"
    );
    Ok(())
}

#[tokio::test]
async fn search_sync_validation_requires_meilisearch_stats(
) -> Result<(), Box<dyn std::error::Error>> {
    let meili = HttpFixture::start(vec![ResponseSpec::ok(r#"{"databaseSize":1}"#)]).await?;
    let config = config_with_database(test_database_url()?, &meili.base_url, Some("secret"))?;

    let report = validate_search_sync_dependencies(&config).await?;

    assert_eq!(
        report.search,
        Some(DependencyCheckStatus::Reachable {
            dependency: DependencyKind::Meilisearch,
            target: meili.base_url.clone(),
        })
    );
    assert_eq!(meili.request_count(), 1);
    assert!(
        meili.first_request().contains("GET /stats "),
        "{}",
        meili.first_request()
    );
    assert!(meili
        .first_request()
        .to_ascii_lowercase()
        .contains("authorization: bearer secret"));
    Ok(())
}

#[tokio::test]
async fn required_meilisearch_validation_formats_non_success_status(
) -> Result<(), Box<dyn std::error::Error>> {
    let meili = HttpFixture::start(vec![ResponseSpec::status(
        "401 Unauthorized",
        "invalid API key",
    )])
    .await?;
    let config = config_with_database(
        schema_prepared_database_url("api_required_search").await?,
        &meili.base_url,
        Some("bad"),
    )?;

    let error = validate_api_dependencies(&config, SearchRequirement::Required)
        .await
        .expect_err("required search is fatal");

    assert_eq!(
        error.to_string(),
        format!(
            "Meilisearch at {} returned 401 Unauthorized: invalid API key",
            meili.base_url
        )
    );
    Ok(())
}

#[tokio::test]
async fn optional_api_search_validation_reports_degraded_search(
) -> Result<(), Box<dyn std::error::Error>> {
    let meili = HttpFixture::start(vec![ResponseSpec::status(
        "401 Unauthorized",
        "invalid API key",
    )])
    .await?;
    let config = config_with_database(
        schema_prepared_database_url("api_optional_search").await?,
        &meili.base_url,
        Some("bad"),
    )?;

    let report = validate_api_dependencies(&config, SearchRequirement::Optional).await?;

    assert!(matches!(
        report.search,
        Some(DependencyCheckStatus::Degraded {
            dependency: DependencyKind::Meilisearch,
            ..
        })
    ));
    assert!(report.to_string().contains("degraded"));
    assert!(report.to_string().contains("invalid API key"));
    Ok(())
}

#[tokio::test]
async fn sync_validation_requires_syncfeed_base_url() -> Result<(), Box<dyn std::error::Error>> {
    let syncfeed = HttpFixture::start(vec![ResponseSpec::ok("ok")]).await?;
    let config = config_with_syncfeed(test_database_url()?, &syncfeed.base_url)?;

    let report = validate_sync_dependencies(&config).await?;

    assert_eq!(
        report.syncfeed,
        Some(DependencyCheckStatus::Reachable {
            dependency: DependencyKind::SyncFeed,
            target: format!("{}/", syncfeed.base_url),
        })
    );
    assert_eq!(syncfeed.request_count(), 1);
    Ok(())
}

#[tokio::test]
async fn sync_validation_formats_syncfeed_unreachable_error(
) -> Result<(), Box<dyn std::error::Error>> {
    let syncfeed = HttpFixture::start(vec![ResponseSpec::status(
        "503 Service Unavailable",
        "maintenance",
    )])
    .await?;
    let config = config_with_syncfeed(test_database_url()?, &syncfeed.base_url)?;

    let error = validate_sync_dependencies(&config)
        .await
        .expect_err("syncfeed failure is fatal");

    assert_eq!(
        error.to_string(),
        format!(
            "SyncFeed base URL {}/ returned 503 Service Unavailable: maintenance",
            syncfeed.base_url
        )
    );
    Ok(())
}

#[tokio::test]
async fn sync_validation_reports_syncfeed_error_body_read_failures(
) -> Result<(), Box<dyn std::error::Error>> {
    let syncfeed = HttpFixture::start(vec![ResponseSpec::truncated(
        "503 Service Unavailable",
        "main",
    )])
    .await?;
    let config = config_with_syncfeed(test_database_url()?, &syncfeed.base_url)?;

    let error = validate_sync_dependencies(&config)
        .await
        .expect_err("syncfeed failure is fatal");
    let error_text = error.to_string();

    assert!(
        error_text.starts_with(&format!(
            "SyncFeed base URL {}/ returned 503 Service Unavailable: ",
            syncfeed.base_url
        )),
        "{error_text}"
    );
    assert!(
        error_text.contains("body")
            || error_text.contains("end of file")
            || error_text.contains("content length"),
        "{error_text}"
    );
    assert!(
        !error_text.ends_with("503 Service Unavailable: "),
        "{error_text}"
    );
    Ok(())
}

fn test_database_url() -> Result<String, std::env::VarError> {
    std::env::var("OPENTK_TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL"))
}

async fn empty_pool(test_name: &str) -> Result<PgPool, sqlx::Error> {
    let database_url = test_database_url()
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL startup tests");
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

    PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_search_path(&database_url, &schema_name))
        .await
}

async fn schema_prepared_database_url(
    test_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let pool = empty_pool(test_name).await?;
    ensure_schema(&pool).await?;
    let database_url = current_database_url(&pool).await?;
    pool.close().await;
    Ok(database_url)
}

async fn current_database_url(pool: &PgPool) -> Result<String, sqlx::Error> {
    let database_url = test_database_url()
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run PostgreSQL startup tests");
    let schema_name: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await?;
    Ok(with_search_path(&database_url, &schema_name))
}

fn with_search_path(database_url: &str, schema_name: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options=-csearch_path%3D{schema_name}")
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn config_with_database(
    database_url: impl AsRef<str>,
    search_url: &str,
    search_api_key: Option<&str>,
) -> Result<Config, Box<dyn std::error::Error>> {
    let api_key = search_api_key.map_or_else(String::new, |key| format!(r#"api_key = "{key}""#));
    Ok(Config::from_toml_str(&format!(
        r#"
        [database]
        url = "{}"

        [search]
        url = "{search_url}"
        {api_key}
        "#,
        database_url.as_ref()
    ))?)
}

fn config_with_syncfeed(
    database_url: impl AsRef<str>,
    syncfeed_url: &str,
) -> Result<Config, Box<dyn std::error::Error>> {
    Ok(Config::from_toml_str(&format!(
        r#"
        [database]
        url = "{}"

        [sync]
        base_url = "{syncfeed_url}"
        "#,
        database_url.as_ref()
    ))?)
}

struct HttpFixture {
    base_url: String,
    requests: Arc<std::sync::Mutex<Vec<String>>>,
}

impl HttpFixture {
    async fn start(responses: Vec<ResponseSpec>) -> Result<Self, std::io::Error> {
        let listener =
            TcpListener::bind(SocketAddr::from_str("127.0.0.1:0").expect("socket")).await?;
        let address = listener.local_addr()?;
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let served = Arc::new(AtomicUsize::new(0));
        let server_requests = Arc::clone(&requests);
        let server_served = Arc::clone(&served);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let mut buffer = vec![0_u8; 8 * 1024];
                let Ok(read) = stream.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                server_requests.lock().expect("request mutex").push(request);
                let index = server_served.fetch_add(1, Ordering::Relaxed);
                let response = responses
                    .get(index)
                    .or_else(|| responses.last())
                    .expect("fixture has at least one response");
                let raw = response.render();
                if stream.write_all(raw.as_bytes()).await.is_err() {
                    return;
                }
            }
        });
        Ok(Self {
            base_url: format!("http://{address}"),
            requests,
        })
    }

    fn request_count(&self) -> usize {
        self.requests.lock().expect("request mutex").len()
    }

    fn first_request(&self) -> String {
        self.requests
            .lock()
            .expect("request mutex")
            .first()
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone)]
struct ResponseSpec {
    status: &'static str,
    body: &'static str,
    content_length: usize,
}

impl ResponseSpec {
    const fn ok(body: &'static str) -> Self {
        Self {
            status: "200 OK",
            body,
            content_length: body.len(),
        }
    }

    const fn status(status: &'static str, body: &'static str) -> Self {
        Self {
            status,
            body,
            content_length: body.len(),
        }
    }

    const fn truncated(status: &'static str, body: &'static str) -> Self {
        Self {
            status,
            body,
            content_length: body.len() + 10,
        }
    }

    fn render(&self) -> String {
        format!(
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.status,
            self.content_length,
            self.body
        )
    }
}
