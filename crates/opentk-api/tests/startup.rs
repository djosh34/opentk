use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::{
    net::SocketAddr,
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tower::ServiceExt;

#[tokio::test]
async fn api_config_builds_router_connected_to_configured_database(
) -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run API startup tests");
    let bind_address = SocketAddr::from_str("127.0.0.1:0")?;

    let server = opentk_api::build_app(opentk_api::ApiConfig {
        bind_address,
        database: opentk_db::DatabaseConfig {
            url: database_url,
            max_connections: 5,
        },
        search: opentk_api::SearchBackendConfig {
            url: "http://127.0.0.1:1".to_owned(),
            api_key: Some("startup-key".to_owned()),
            index_name: "startup_index".to_owned(),
        },
        max_public_query_limit: 1000,
    })
    .await?;

    assert_eq!(server.bind_address, bind_address);
    let response = server
        .router
        .oneshot(Request::get("/categories").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    assert!(body["categories"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));

    Ok(())
}

#[tokio::test]
async fn api_startup_marks_search_unavailable_when_probe_fails(
) -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("OPENTK_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set OPENTK_TEST_DATABASE_URL or DATABASE_URL to run API startup tests");
    let search = FailingSearchServer::start().await?;

    let server = opentk_api::build_app(opentk_api::ApiConfig {
        bind_address: SocketAddr::from_str("127.0.0.1:0")?,
        database: opentk_db::DatabaseConfig {
            url: database_url,
            max_connections: 5,
        },
        search: opentk_api::SearchBackendConfig {
            url: search.base_url.clone(),
            api_key: Some("startup-key".to_owned()),
            index_name: "startup_index".to_owned(),
        },
        max_public_query_limit: 1000,
    })
    .await?;

    assert_eq!(search.request_count(), 1, "startup must probe search once");
    assert!(
        search.first_request().starts_with("GET /stats "),
        "{}",
        search.first_request()
    );

    let health = server
        .router
        .clone()
        .oneshot(Request::get("/health").body(Body::empty())?)
        .await?;
    assert_eq!(health.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&to_bytes(health.into_body(), usize::MAX).await?)?;
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["postgres"], "ok");
    assert_eq!(body["meilisearch"], "unavailable");

    let search_response = server
        .router
        .oneshot(Request::get("/search?q=test").body(Body::empty())?)
        .await?;
    assert_eq!(search_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value =
        serde_json::from_slice(&to_bytes(search_response.into_body(), usize::MAX).await?)?;
    assert_eq!(
        body,
        serde_json::json!({
            "code": "search_unavailable",
            "message": "search unavailable"
        })
    );
    assert_eq!(
        search.request_count(),
        1,
        "unavailable search client must avoid repeated backend calls"
    );

    Ok(())
}

struct FailingSearchServer {
    base_url: String,
    requests: Arc<AtomicUsize>,
    first_request: Arc<Mutex<Option<String>>>,
}

impl FailingSearchServer {
    async fn start() -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let requests = Arc::new(AtomicUsize::new(0));
        let first_request = Arc::new(Mutex::new(None));
        let server_requests = Arc::clone(&requests);
        let server_first_request = Arc::clone(&first_request);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    return;
                };
                server_requests.fetch_add(1, Ordering::SeqCst);
                let first_request = Arc::clone(&server_first_request);
                tokio::spawn(async move {
                    let mut buffer = [0_u8; 4096];
                    let bytes_read = stream.read(&mut buffer).await.expect("read request");
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
                    {
                        let mut first = first_request.lock().expect("first request mutex");
                        if first.is_none() {
                            *first = Some(request);
                        }
                    }
                    stream
                        .write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\ncontent-type: text/plain\r\ncontent-length: 18\r\nconnection: close\r\n\r\nsearch unavailable",
                        )
                        .await
                        .expect("write response");
                });
            }
        });

        Ok(Self {
            base_url: format!("http://{address}"),
            requests,
            first_request,
        })
    }

    fn request_count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    fn first_request(&self) -> String {
        self.first_request
            .lock()
            .expect("first request mutex")
            .clone()
            .unwrap_or_default()
    }
}
