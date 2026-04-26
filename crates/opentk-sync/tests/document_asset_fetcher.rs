use std::{
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};

use opentk_sync::document_asset::{
    DocumentAssetFetchConfig, DocumentAssetFetchRequest, DocumentAssetFetcher, DocumentAssetKind,
    RetrievalStatus,
};
use reqwest::Url;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};
use uuid::Uuid;

fn config() -> DocumentAssetFetchConfig {
    DocumentAssetFetchConfig {
        request_timeout: Duration::from_secs(2),
        connect_timeout: Duration::from_secs(2),
        max_retries: 0,
        initial_retry_delay: Duration::from_millis(1),
        max_retry_delay: Duration::from_millis(1),
        max_concurrent_requests: NonZeroUsize::new(4).expect("non-zero"),
        max_asset_bytes: 1024 * 1024,
    }
}

#[tokio::test]
async fn html_asset_prefers_official_text_alternative() {
    let server = TestServer::start(vec![
        TestResponse::html(
            "/document",
            r#"
            <html>
              <head>
                <link rel="alternate" type="text/plain" href="/document.txt">
              </head>
              <body><a href="/document.pdf">PDF</a></body>
            </html>
            "#,
        ),
        TestResponse::plain("/document.txt", "official transcript text"),
    ])
    .await;
    let asset_url = Url::parse(&format!("{}/document", server.base_url)).expect("asset URL");
    let text_url = Url::parse(&format!("{}/document.txt", server.base_url)).expect("text URL");
    let fetcher = DocumentAssetFetcher::new(config()).expect("fetcher config");

    let report = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: asset_url.clone(),
            expected_content_type: Some("text/html".to_owned()),
            expected_content_length: None,
        })
        .await;

    assert_eq!(report.retrieval_status, RetrievalStatus::Fetched);
    assert_eq!(report.retrieval_error, None);
    assert_eq!(report.asset_url, asset_url);
    assert_eq!(report.upstream_url, asset_url);
    assert_eq!(report.upstream_content_type.as_deref(), Some("text/html"));
    assert!(report.upstream_content_length.is_some());

    let selected = report.selected_source.expect("selected source");
    assert_eq!(selected.url, text_url);
    assert_eq!(selected.content_type.as_deref(), Some("text/plain"));
    assert_eq!(selected.content_length, Some(24));
    assert_eq!(selected.kind, DocumentAssetKind::OfficialText);
    assert!(selected.official_source);
    assert_eq!(selected.source_rank, 0);
    assert_eq!(
        report.content_hash.as_deref(),
        Some("0ae0f2625510c92b5b792ec6103b96fbfa40d52940ace95fe7ce1e02d93a3aa3")
    );
    assert_eq!(
        report.selected_body.as_deref(),
        Some("official transcript text".as_bytes())
    );
    assert_eq!(report.discovered_sources.len(), 1);
    assert_eq!(report.discovered_sources[0].url, text_url);
}

#[tokio::test]
async fn pdf_and_docx_assets_are_binary_extraction_inputs() {
    let server = TestServer::start(vec![
        TestResponse::ok("/document.pdf", "application/pdf", "%PDF fixture"),
        TestResponse::ok(
            "/document.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "PK docx fixture",
        ),
    ])
    .await;
    let fetcher = DocumentAssetFetcher::new(config()).expect("fetcher config");

    let pdf_url = Url::parse(&format!("{}/document.pdf", server.base_url)).expect("PDF URL");
    let pdf = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: pdf_url.clone(),
            expected_content_type: Some("application/pdf".to_owned()),
            expected_content_length: Some(12),
        })
        .await;

    assert_eq!(pdf.retrieval_status, RetrievalStatus::Fetched);
    let pdf_selected = pdf.selected_source.expect("PDF selected source");
    assert_eq!(pdf_selected.url, pdf_url);
    assert_eq!(pdf_selected.kind, DocumentAssetKind::Pdf);
    assert_eq!(
        pdf_selected.content_type.as_deref(),
        Some("application/pdf")
    );
    assert_eq!(pdf_selected.content_length, Some(12));
    assert!(!pdf_selected.official_source);
    assert_eq!(pdf_selected.source_rank, 10);
    assert_eq!(
        pdf.content_hash.as_deref(),
        Some("9fabd57eb6ff1bc32d6a31eb86c7f493e6ba94b62fa5277f24e8bb4bd7429f0a")
    );
    assert_eq!(
        pdf.selected_body.as_deref(),
        Some("%PDF fixture".as_bytes())
    );

    let docx_url = Url::parse(&format!("{}/document.docx", server.base_url)).expect("DOCX URL");
    let docx = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: docx_url.clone(),
            expected_content_type: Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_owned(),
            ),
            expected_content_length: Some(15),
        })
        .await;

    assert_eq!(docx.retrieval_status, RetrievalStatus::Fetched);
    let docx_selected = docx.selected_source.expect("DOCX selected source");
    assert_eq!(docx_selected.url, docx_url);
    assert_eq!(docx_selected.kind, DocumentAssetKind::Docx);
    assert_eq!(
        docx_selected.content_type.as_deref(),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    );
    assert_eq!(docx_selected.content_length, Some(15));
    assert!(!docx_selected.official_source);
    assert_eq!(docx_selected.source_rank, 10);
    assert_eq!(
        docx.content_hash.as_deref(),
        Some("7af448b8f1053dc147679949648b9145f66d40ce42d359d3b83505737b043e06")
    );
    assert_eq!(
        docx.selected_body.as_deref(),
        Some("PK docx fixture".as_bytes())
    );
}

#[tokio::test]
async fn missing_length_succeeds_but_wrong_expected_length_fails() {
    let server = TestServer::start(vec![
        TestResponse::plain("/no-length.txt", "no length").without_content_length(),
        TestResponse::plain("/wrong-length.txt", "wrong length"),
        TestResponse::plain("/wrong-type.txt", "wrong type"),
    ])
    .await;
    let fetcher = DocumentAssetFetcher::new(config()).expect("fetcher config");

    let no_length_url =
        Url::parse(&format!("{}/no-length.txt", server.base_url)).expect("no-length URL");
    let no_length = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: no_length_url,
            expected_content_type: Some("text/plain".to_owned()),
            expected_content_length: None,
        })
        .await;

    assert_eq!(no_length.retrieval_status, RetrievalStatus::Fetched);
    assert_eq!(no_length.upstream_content_length, None);
    assert_eq!(
        no_length
            .selected_source
            .expect("selected source")
            .content_length,
        None
    );

    let wrong_length_url =
        Url::parse(&format!("{}/wrong-length.txt", server.base_url)).expect("wrong-length URL");
    let wrong_length = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: wrong_length_url,
            expected_content_type: Some("text/plain".to_owned()),
            expected_content_length: Some(999),
        })
        .await;

    assert_eq!(wrong_length.retrieval_status, RetrievalStatus::Failed);
    assert!(
        wrong_length
            .retrieval_error
            .as_deref()
            .is_some_and(|error| error.contains("content length")),
        "expected explicit content-length error, got {:?}",
        wrong_length.retrieval_error
    );
    assert!(wrong_length.selected_source.is_none());
    assert!(wrong_length.content_hash.is_none());

    let wrong_type_url =
        Url::parse(&format!("{}/wrong-type.txt", server.base_url)).expect("wrong-type URL");
    let wrong_type = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: wrong_type_url,
            expected_content_type: Some("application/pdf".to_owned()),
            expected_content_length: None,
        })
        .await;

    assert_eq!(wrong_type.retrieval_status, RetrievalStatus::Failed);
    assert!(
        wrong_type
            .retrieval_error
            .as_deref()
            .is_some_and(|error| error.contains("content type")),
        "expected explicit content-type error, got {:?}",
        wrong_type.retrieval_error
    );
    assert!(wrong_type.selected_source.is_none());
    assert!(wrong_type.content_hash.is_none());
}

#[tokio::test]
async fn unsupported_not_found_and_retry_exhaustion_are_status_reports() {
    let server = TestServer::start(vec![
        TestResponse::ok("/image.png", "image/png", "png"),
        TestResponse::status("/missing.pdf", 404, "missing"),
        TestResponse::status("/busy.pdf", 503, "busy"),
        TestResponse::status("/busy.pdf", 503, "still busy"),
    ])
    .await;
    let mut retry_config = config();
    retry_config.max_retries = 1;
    let fetcher = DocumentAssetFetcher::new(retry_config).expect("fetcher config");

    let unsupported = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: Url::parse(&format!("{}/image.png", server.base_url))
                .expect("unsupported URL"),
            expected_content_type: Some("image/png".to_owned()),
            expected_content_length: None,
        })
        .await;
    assert_eq!(
        unsupported.retrieval_status,
        RetrievalStatus::UnsupportedContentType
    );
    assert!(unsupported.selected_source.is_none());
    assert!(unsupported.content_hash.is_none());

    let not_found = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: Url::parse(&format!("{}/missing.pdf", server.base_url))
                .expect("missing URL"),
            expected_content_type: Some("application/pdf".to_owned()),
            expected_content_length: None,
        })
        .await;
    assert_eq!(not_found.retrieval_status, RetrievalStatus::NotFound);
    assert!(not_found.selected_source.is_none());

    let busy_url = Url::parse(&format!("{}/busy.pdf", server.base_url)).expect("busy URL");
    let failed = fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: busy_url,
            expected_content_type: Some("application/pdf".to_owned()),
            expected_content_length: None,
        })
        .await;
    assert_eq!(failed.retrieval_status, RetrievalStatus::Failed);
    assert!(
        failed
            .retrieval_error
            .as_deref()
            .is_some_and(|error| error.contains("503")),
        "expected retry exhaustion to include 503, got {:?}",
        failed.retrieval_error
    );

    let requests = server.requests().await;
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.as_str() == "/busy.pdf")
            .count(),
        2
    );
}

#[tokio::test]
async fn fetch_many_honors_concurrency_limit_and_timeout() {
    let server = TestServer::start(vec![
        TestResponse::plain("/slow-1.txt", "slow one").with_delay(Duration::from_millis(60)),
        TestResponse::plain("/slow-2.txt", "slow two").with_delay(Duration::from_millis(60)),
        TestResponse::plain("/timeout.txt", "timeout").with_delay(Duration::from_millis(120)),
    ])
    .await;
    let mut limited_config = config();
    limited_config.max_concurrent_requests = NonZeroUsize::new(1).expect("non-zero");
    limited_config.request_timeout = Duration::from_millis(500);
    let fetcher = DocumentAssetFetcher::new(limited_config).expect("fetcher config");
    let requests =
        ["/slow-1.txt", "/slow-2.txt"]
            .into_iter()
            .map(|path| DocumentAssetFetchRequest {
                document_source_category: "Document".to_owned(),
                document_source_id: Uuid::new_v4(),
                asset_url: Url::parse(&format!("{}{path}", server.base_url)).expect("asset URL"),
                expected_content_type: Some("text/plain".to_owned()),
                expected_content_length: None,
            });

    let started_at = Instant::now();
    let reports = fetcher.fetch_many(requests).await;

    assert_eq!(reports.len(), 2);
    assert_eq!(server.max_active_requests().await, 1);
    assert!(
        started_at.elapsed() >= Duration::from_millis(100),
        "sequential delayed requests should not overlap"
    );
    assert!(reports
        .iter()
        .all(|report| report.retrieval_status == RetrievalStatus::Fetched));

    let mut timeout_config = config();
    timeout_config.request_timeout = Duration::from_millis(20);
    let timeout_fetcher = DocumentAssetFetcher::new(timeout_config).expect("fetcher config");
    let timeout_report = timeout_fetcher
        .fetch_one(DocumentAssetFetchRequest {
            document_source_category: "Document".to_owned(),
            document_source_id: Uuid::new_v4(),
            asset_url: Url::parse(&format!("{}/timeout.txt", server.base_url))
                .expect("timeout URL"),
            expected_content_type: Some("text/plain".to_owned()),
            expected_content_length: None,
        })
        .await;
    assert_eq!(timeout_report.retrieval_status, RetrievalStatus::Failed);
    assert!(
        timeout_report
            .retrieval_error
            .as_deref()
            .is_some_and(|error| error.contains("timed out")),
        "expected timeout error, got {:?}",
        timeout_report.retrieval_error
    );
}

#[derive(Clone, Debug)]
struct TestResponse {
    expected_target: String,
    status: u16,
    content_type: Option<&'static str>,
    body: String,
    delay: Duration,
    omit_content_length: bool,
    content_length_override: Option<usize>,
}

impl TestResponse {
    fn html(expected_target: &str, body: &str) -> Self {
        Self::ok(expected_target, "text/html", body)
    }

    fn plain(expected_target: &str, body: &str) -> Self {
        Self::ok(expected_target, "text/plain", body)
    }

    fn ok(expected_target: &str, content_type: &'static str, body: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status: 200,
            content_type: Some(content_type),
            body: body.to_owned(),
            delay: Duration::ZERO,
            omit_content_length: false,
            content_length_override: None,
        }
    }

    const fn without_content_length(mut self) -> Self {
        self.omit_content_length = true;
        self
    }

    fn status(expected_target: &str, status: u16, body: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status,
            content_type: Some("text/plain"),
            body: body.to_owned(),
            delay: Duration::ZERO,
            omit_content_length: false,
            content_length_override: None,
        }
    }

    const fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

#[derive(Clone, Debug)]
struct TestServer {
    base_url: String,
    _responses: Arc<Mutex<Vec<TestResponse>>>,
    requests: Arc<Mutex<Vec<String>>>,
    max_active_requests: Arc<Mutex<usize>>,
}

impl TestServer {
    async fn start(responses: Vec<TestResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("local address");
        let responses = Arc::new(Mutex::new(responses));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let active_requests = Arc::new(Mutex::new(0_usize));
        let max_active_requests = Arc::new(Mutex::new(0_usize));
        let server_responses = Arc::clone(&responses);
        let server_requests = Arc::clone(&requests);
        let server_active_requests = Arc::clone(&active_requests);
        let server_max_active_requests = Arc::clone(&max_active_requests);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    return;
                };
                let responses = Arc::clone(&server_responses);
                let requests = Arc::clone(&server_requests);
                let active_requests = Arc::clone(&server_active_requests);
                let max_active_requests = Arc::clone(&server_max_active_requests);
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
                            .position(|response| response.expected_target == target)
                            .expect("matching response");
                        responses.remove(index)
                    };
                    {
                        let mut active = active_requests.lock().await;
                        *active += 1;
                        let mut max_active = max_active_requests.lock().await;
                        *max_active = (*max_active).max(*active);
                    }
                    if !response.delay.is_zero() {
                        tokio::time::sleep(response.delay).await;
                    }
                    let body = response.body;
                    let content_type_header = response
                        .content_type
                        .map_or_else(String::new, |content_type| {
                            format!("content-type: {content_type}\r\n")
                        });
                    let content_length_header = if response.omit_content_length {
                        String::new()
                    } else {
                        let content_length = response.content_length_override.unwrap_or(body.len());
                        format!("content-length: {content_length}\r\n")
                    };
                    let wire_response = format!(
                        "HTTP/1.1 {status} OK\r\n{content_type_header}{content_length_header}connection: close\r\n\r\n{body}",
                        status = response.status,
                    );
                    stream
                        .write_all(wire_response.as_bytes())
                        .await
                        .expect("write response");
                    *active_requests.lock().await -= 1;
                });
            }
        });
        Self {
            base_url: format!("http://{address}"),
            _responses: responses,
            requests,
            max_active_requests,
        }
    }

    async fn requests(&self) -> Vec<String> {
        self.requests.lock().await.clone()
    }

    async fn max_active_requests(&self) -> usize {
        *self.max_active_requests.lock().await
    }
}
