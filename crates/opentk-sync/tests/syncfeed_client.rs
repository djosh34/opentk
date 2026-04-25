use std::{num::NonZeroUsize, sync::Arc, time::Duration};

use opentk_sync::syncfeed::{
    CategoryCursor, SyncFeedClient, SyncFeedClientConfig, SyncFeedClientError, SyncFeedContentMode,
    SyncFeedCursor,
};
use reqwest::Url;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};

fn config(base_url: Url) -> SyncFeedClientConfig {
    SyncFeedClientConfig {
        base_url,
        content_mode: SyncFeedContentMode::Internal,
        request_timeout: Duration::from_secs(2),
        connect_timeout: Duration::from_secs(2),
        max_retries: 0,
        initial_retry_delay: Duration::from_millis(1),
        max_retry_delay: Duration::from_millis(1),
        max_concurrent_requests: NonZeroUsize::new(4).expect("non-zero"),
    }
}

#[tokio::test]
async fn fetch_page_uses_last_entry_next_as_next_request() {
    let server = TestServer::start(vec![TestResponse::atom(
        "/SyncFeed/2.0/Feed?category=Document&content=internal",
        "",
    )])
    .await;
    let entry_next = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
        server.base_url
    );
    let unrelated_feed_next = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=999&content=internal",
        server.base_url
    );
    let entry_next_xml = entry_next.replace('&', "&amp;");
    let unrelated_feed_next_xml = unrelated_feed_next.replace('&', "&amp;");
    server
        .replace_responses(vec![TestResponse::atom(
            "/SyncFeed/2.0/Feed?category=Document&content=internal",
            &format!(
                r#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <link rel="next" href="{unrelated_feed_next_xml}" />
              <entry>
                <id>document-1</id>
                <category term="Document" />
                <updated>2026-04-26T00:00:00Z</updated>
                <link rel="next" href="{entry_next_xml}" />
                <content type="application/xml"><Document><Id>document-1</Id></Document></content>
              </entry>
            </feed>
            "#
            ),
        )])
        .await;

    let base_url = Url::parse(&server.base_url).expect("mock server URL");
    let client = SyncFeedClient::new(config(base_url.clone())).expect("valid client config");
    let page = client
        .fetch_page(SyncFeedCursor::first_page(
            &base_url,
            "Document",
            SyncFeedContentMode::Internal,
        ))
        .await
        .expect("page fetch succeeds");

    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].id, "document-1");
    assert_eq!(
        page.next_request.expect("entry next cursor").url.as_str(),
        entry_next
    );
    assert_eq!(page.outcome.status.as_u16(), 200);
    assert_eq!(page.outcome.attempts, 1);
    assert!(page.outcome.latency > Duration::ZERO);
}

#[tokio::test]
async fn category_fetch_follows_entry_next_until_resume() {
    let server = TestServer::start(Vec::new()).await;
    let first_next = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
        server.base_url
    );
    let resume = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=2&content=internal",
        server.base_url
    );
    server
        .replace_responses(vec![
            TestResponse::atom(
                "/SyncFeed/2.0/Feed?category=Document&content=internal",
                &format!(
                    r#"
                    <feed xmlns="http://www.w3.org/2005/Atom">
                      <entry>
                        <id>document-1</id>
                        <category term="Document" />
                        <updated>2026-04-26T00:00:00Z</updated>
                        <link rel="next" href="{}" />
                      </entry>
                    </feed>
                    "#,
                    xml_url(&first_next)
                ),
            ),
            TestResponse::atom(
                "/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
                &format!(
                    r#"
                    <feed xmlns="http://www.w3.org/2005/Atom">
                      <link rel="resume" href="{}" />
                    </feed>
                    "#,
                    xml_url(&resume)
                ),
            ),
        ])
        .await;

    let base_url = Url::parse(&server.base_url).expect("mock server URL");
    let client = SyncFeedClient::new(config(base_url)).expect("valid client config");
    let pages = client
        .fetch_category_until_resume("Document", CategoryCursor::Start)
        .await
        .expect("category fetch succeeds");

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].entries[0].next.url.as_str(), first_next);
    assert_eq!(pages[1].entries.len(), 0);
    assert_eq!(
        pages[1].resume.as_ref().expect("resume").url.as_str(),
        resume
    );
}

#[test]
fn resumed_cursor_urls_are_strictly_validated() {
    let base_url = Url::parse("https://example.test").expect("base URL");

    let wrong_path =
        Url::parse("https://example.test/not-feed?category=Document&skiptoken=1").expect("URL");
    assert!(matches!(
        SyncFeedCursor::from_url(
            &base_url,
            "Document",
            wrong_path,
            SyncFeedContentMode::Internal
        ),
        Err(SyncFeedClientError::MalformedCursorUrl { .. })
    ));

    let missing_category =
        Url::parse("https://example.test/SyncFeed/2.0/Feed?skiptoken=1").expect("URL");
    assert!(matches!(
        SyncFeedCursor::from_url(
            &base_url,
            "Document",
            missing_category,
            SyncFeedContentMode::Internal
        ),
        Err(SyncFeedClientError::MissingCursorState { .. })
    ));

    let wrong_category =
        Url::parse("https://example.test/SyncFeed/2.0/Feed?category=Zaak&skiptoken=1")
            .expect("URL");
    assert!(matches!(
        SyncFeedCursor::from_url(
            &base_url,
            "Document",
            wrong_category,
            SyncFeedContentMode::Internal
        ),
        Err(SyncFeedClientError::CategoryMismatch { .. })
    ));

    let missing_skiptoken =
        Url::parse("https://example.test/SyncFeed/2.0/Feed?category=Document").expect("URL");
    assert!(matches!(
        SyncFeedCursor::from_url(
            &base_url,
            "Document",
            missing_skiptoken,
            SyncFeedContentMode::Internal
        ),
        Err(SyncFeedClientError::MissingCursorState { .. })
    ));
}

#[tokio::test]
async fn response_failures_are_explicit_errors() {
    let server = TestServer::start(vec![
        TestResponse::status(
            "/SyncFeed/2.0/Feed?category=Document&content=internal",
            400,
            "bad request",
        ),
        TestResponse::plain(
            "/SyncFeed/2.0/Feed?category=Document&content=internal",
            "<feed />",
        ),
        TestResponse::atom(
            "/SyncFeed/2.0/Feed?category=Document&content=internal",
            "<feed>",
        ),
        TestResponse::atom(
            "/SyncFeed/2.0/Feed?category=Document&content=internal",
            r#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <entry>
                <id>document-1</id>
                <category term="Document" />
                <updated>2026-04-26T00:00:00Z</updated>
              </entry>
            </feed>
            "#,
        ),
    ])
    .await;
    let base_url = Url::parse(&server.base_url).expect("mock server URL");
    let client = SyncFeedClient::new(config(base_url.clone())).expect("valid client config");
    let cursor =
        || SyncFeedCursor::first_page(&base_url, "Document", SyncFeedContentMode::Internal);

    assert!(matches!(
        client.fetch_page(cursor()).await,
        Err(SyncFeedClientError::NonSuccessStatus { status, .. }) if status.as_u16() == 400
    ));
    assert!(matches!(
        client.fetch_page(cursor()).await,
        Err(SyncFeedClientError::InvalidXmlContentType { .. })
    ));
    assert!(matches!(
        client.fetch_page(cursor()).await,
        Err(SyncFeedClientError::InvalidXml { .. })
    ));
    assert!(matches!(
        client.fetch_page(cursor()).await,
        Err(SyncFeedClientError::MissingEntryNext { .. })
    ));

    let timeout_server = TestServer::start(vec![TestResponse::atom(
        "/SyncFeed/2.0/Feed?category=Document&content=internal",
        "<feed />",
    )
    .with_delay(Duration::from_millis(100))])
    .await;
    let timeout_base_url = Url::parse(&timeout_server.base_url).expect("mock server URL");
    let mut timeout_config = config(timeout_base_url.clone());
    timeout_config.request_timeout = Duration::from_millis(10);
    let timeout_client = SyncFeedClient::new(timeout_config).expect("valid client config");
    let timeout_cursor =
        SyncFeedCursor::first_page(&timeout_base_url, "Document", SyncFeedContentMode::Internal);
    assert!(matches!(
        timeout_client.fetch_page(timeout_cursor).await,
        Err(SyncFeedClientError::Timeout { .. })
    ));
}

#[tokio::test]
async fn retryable_statuses_are_retried_and_exhaustion_is_explicit() {
    let server = TestServer::start(Vec::new()).await;
    let next = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
        server.base_url
    );
    server
        .replace_responses(vec![
            TestResponse::status(
                "/SyncFeed/2.0/Feed?category=Document&content=internal",
                429,
                "rate limited",
            ),
            TestResponse::atom(
                "/SyncFeed/2.0/Feed?category=Document&content=internal",
                &format!(
                    r#"
                    <feed xmlns="http://www.w3.org/2005/Atom">
                      <entry>
                        <id>document-1</id>
                        <category term="Document" />
                        <updated>2026-04-26T00:00:00Z</updated>
                        <link rel="next" href="{}" />
                      </entry>
                    </feed>
                    "#,
                    xml_url(&next)
                ),
            ),
        ])
        .await;
    let base_url = Url::parse(&server.base_url).expect("mock server URL");
    let mut retry_config = config(base_url.clone());
    retry_config.max_retries = 1;
    let client = SyncFeedClient::new(retry_config).expect("valid client config");
    let page = client
        .fetch_page(SyncFeedCursor::first_page(
            &base_url,
            "Document",
            SyncFeedContentMode::Internal,
        ))
        .await
        .expect("retry succeeds");

    assert_eq!(page.outcome.status.as_u16(), 200);
    assert_eq!(page.outcome.attempts, 2);

    server
        .replace_responses(vec![
            TestResponse::status(
                "/SyncFeed/2.0/Feed?category=Document&content=internal",
                503,
                "unavailable",
            ),
            TestResponse::status(
                "/SyncFeed/2.0/Feed?category=Document&content=internal",
                503,
                "unavailable",
            ),
        ])
        .await;
    let error = client
        .fetch_page(SyncFeedCursor::first_page(
            &base_url,
            "Document",
            SyncFeedContentMode::Internal,
        ))
        .await
        .expect_err("retry exhaustion");
    assert!(matches!(
        error,
        SyncFeedClientError::RetryExhausted {
            attempts: 2,
            last_status: Some(status),
            ..
        } if status.as_u16() == 503
    ));
}

#[tokio::test]
async fn category_workers_fetch_in_parallel_while_each_category_preserves_cursor_order() {
    let server = TestServer::start(Vec::new()).await;
    let document_next = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
        server.base_url
    );
    let document_resume = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=2&content=internal",
        server.base_url
    );
    let zaak_next = format!(
        "{}/SyncFeed/2.0/Feed?category=Zaak&skiptoken=1&content=internal",
        server.base_url
    );
    let zaak_resume = format!(
        "{}/SyncFeed/2.0/Feed?category=Zaak&skiptoken=2&content=internal",
        server.base_url
    );
    server
        .replace_responses(vec![
            entry_page("Document", &document_next)
                .for_target("/SyncFeed/2.0/Feed?category=Document&content=internal")
                .with_delay(Duration::from_millis(80)),
            entry_page("Zaak", &zaak_next)
                .for_target("/SyncFeed/2.0/Feed?category=Zaak&content=internal")
                .with_delay(Duration::from_millis(80)),
            resume_page(&document_resume)
                .for_target("/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal"),
            resume_page(&zaak_resume)
                .for_target("/SyncFeed/2.0/Feed?category=Zaak&skiptoken=1&content=internal"),
        ])
        .await;

    let base_url = Url::parse(&server.base_url).expect("mock server URL");
    let mut parallel_config = config(base_url);
    parallel_config.max_concurrent_requests = NonZeroUsize::new(2).expect("non-zero");
    let client = SyncFeedClient::new(parallel_config).expect("valid client config");
    let pages_by_category = client
        .fetch_categories_until_resume(["Document", "Zaak"])
        .await
        .expect("parallel category fetch succeeds");

    assert_eq!(pages_by_category.len(), 2);
    assert_eq!(server.max_active_requests().await, 2);
    let requests = server.requests().await;
    assert_category_order(
        &requests,
        "/SyncFeed/2.0/Feed?category=Document&content=internal",
        "/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
    );
    assert_category_order(
        &requests,
        "/SyncFeed/2.0/Feed?category=Zaak&content=internal",
        "/SyncFeed/2.0/Feed?category=Zaak&skiptoken=1&content=internal",
    );
}

#[tokio::test]
async fn rate_limit_observations_adjust_adaptive_pacing() {
    let server = TestServer::start(Vec::new()).await;
    let next = format!(
        "{}/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
        server.base_url
    );
    server
        .replace_responses(vec![
            TestResponse::status(
                "/SyncFeed/2.0/Feed?category=Document&content=internal",
                429,
                "rate limited",
            ),
            entry_page("Document", &next)
                .for_target("/SyncFeed/2.0/Feed?category=Document&content=internal"),
        ])
        .await;

    let base_url = Url::parse(&server.base_url).expect("mock server URL");
    let mut limiter_config = config(base_url.clone());
    limiter_config.initial_retry_delay = Duration::from_millis(20);
    limiter_config.max_retry_delay = Duration::from_millis(100);
    limiter_config.max_retries = 0;
    let client = SyncFeedClient::new(limiter_config).expect("valid client config");

    let first_error = client
        .fetch_page(SyncFeedCursor::first_page(
            &base_url,
            "Document",
            SyncFeedContentMode::Internal,
        ))
        .await
        .expect_err("429 exhausts without retry");
    assert!(matches!(
        first_error,
        SyncFeedClientError::RetryExhausted {
            last_status: Some(status),
            ..
        } if status.as_u16() == 429
    ));
    let delayed_after_429 = client.current_pacing_delay().await;
    assert!(delayed_after_429 >= Duration::from_millis(20));

    let page = client
        .fetch_page(SyncFeedCursor::first_page(
            &base_url,
            "Document",
            SyncFeedContentMode::Internal,
        ))
        .await
        .expect("success after pacing");
    assert_eq!(page.outcome.status.as_u16(), 200);
    assert!(client.current_pacing_delay().await < delayed_after_429);
}

#[derive(Clone, Debug)]
struct TestResponse {
    expected_target: String,
    status: u16,
    content_type: Option<&'static str>,
    body: String,
    delay: Duration,
}

impl TestResponse {
    fn atom(expected_target: &str, body: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status: 200,
            content_type: Some("application/atom+xml"),
            body: body.to_owned(),
            delay: Duration::ZERO,
        }
    }

    fn plain(expected_target: &str, body: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status: 200,
            content_type: Some("text/plain"),
            body: body.to_owned(),
            delay: Duration::ZERO,
        }
    }

    fn status(expected_target: &str, status: u16, body: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status,
            content_type: Some("application/atom+xml"),
            body: body.to_owned(),
            delay: Duration::ZERO,
        }
    }

    const fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    fn for_target(mut self, expected_target: &str) -> Self {
        expected_target.clone_into(&mut self.expected_target);
        self
    }
}

fn xml_url(url: &str) -> String {
    url.replace('&', "&amp;")
}

fn entry_page(category: &str, next: &str) -> TestResponse {
    TestResponse::atom(
        "",
        &format!(
            r#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <entry>
                <id>{category}-1</id>
                <category term="{category}" />
                <updated>2026-04-26T00:00:00Z</updated>
                <link rel="next" href="{}" />
              </entry>
            </feed>
            "#,
            xml_url(next)
        ),
    )
}

fn resume_page(resume: &str) -> TestResponse {
    TestResponse::atom(
        "",
        &format!(
            r#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <link rel="resume" href="{}" />
            </feed>
            "#,
            xml_url(resume)
        ),
    )
}

fn assert_category_order(requests: &[String], first: &str, second: &str) {
    let first_index = requests
        .iter()
        .position(|request| request == first)
        .expect("first category request");
    let second_index = requests
        .iter()
        .position(|request| request == second)
        .expect("second category request");
    assert!(first_index < second_index);
}

#[derive(Clone, Debug)]
struct TestServer {
    base_url: String,
    responses: Arc<Mutex<Vec<TestResponse>>>,
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
                    let wire_response = format!(
                        "HTTP/1.1 {status} OK\r\n{content_type_header}content-length: {content_length}\r\nconnection: close\r\n\r\n{body}",
                        status = response.status,
                        content_length = body.len(),
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
            responses,
            requests,
            max_active_requests,
        }
    }

    async fn replace_responses(&self, responses: Vec<TestResponse>) {
        *self.responses.lock().await = responses;
    }

    async fn requests(&self) -> Vec<String> {
        self.requests.lock().await.clone()
    }

    async fn max_active_requests(&self) -> usize {
        *self.max_active_requests.lock().await
    }
}
