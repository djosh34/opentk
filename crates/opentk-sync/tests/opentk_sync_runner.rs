use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use opentk_sync::{
    payload::ParsedEntity,
    runner::{
        CategoryStatus, CategorySyncState, CompleteSyncConfig, CompleteSyncError,
        CompleteSyncRunner, DurableSyncError, PreparedSyncPage, StoredCategoryCursor, SyncPhase,
        SyncRunMode, SyncStore, SyncStoreError, SyncStoreFuture, SyncStoreWriteOutcome,
    },
    syncfeed::{SyncFeedClient, SyncFeedClientConfig, SyncFeedContentMode},
};
use reqwest::Url;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};
use uuid::Uuid;

#[tokio::test]
async fn crash_before_cursor_commit_refetches_page_and_applies_entity_once() {
    let server = TestServer::start(Vec::new()).await;
    let next = server.cursor("Document", 1);
    let resume = server.cursor("Document", 2);
    let first = "/SyncFeed/2.0/Feed?category=Document&content=internal";
    server
        .replace_responses(vec![
            document_page("Document", &next).for_target(first),
            document_page("Document", &next).for_target(first),
            resume_page(&resume)
                .for_target("/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal"),
        ])
        .await;
    let store = MemoryStore::default();
    store.fail_next_write_before_commit().await;
    let runner = runner(&server, store.clone(), ["Document"]);

    assert!(matches!(
        runner.run_once().await,
        Err(CompleteSyncError::Store {
            phase: SyncPhase::Write,
            ..
        })
    ));
    assert!(store.cursor("Document").await.is_none());

    let report = runner.run_once().await.expect("restart completes");

    assert_eq!(report.categories[0].entities_seen, 1);
    assert_eq!(store.entity_count("Document").await, 1);
    assert_eq!(
        server.requests().await,
        vec![
            first.to_owned(),
            first.to_owned(),
            "/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal".to_owned(),
        ]
    );
}

#[tokio::test]
async fn crash_after_cursor_commit_restarts_from_advanced_cursor() {
    let server = TestServer::start(Vec::new()).await;
    let next = server.cursor("Document", 1);
    let resume = server.cursor("Document", 2);
    server
        .replace_responses(vec![
            document_page("Document", &next)
                .for_target("/SyncFeed/2.0/Feed?category=Document&content=internal"),
            resume_page(&resume)
                .for_target("/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal"),
        ])
        .await;
    let store = MemoryStore::default();
    store.fail_next_write_after_commit().await;
    let runner = runner(&server, store.clone(), ["Document"]);

    assert!(matches!(
        runner.run_once().await,
        Err(CompleteSyncError::Store {
            phase: SyncPhase::Write,
            ..
        })
    ));
    assert_eq!(
        store
            .cursor("Document")
            .await
            .expect("cursor")
            .latest_skiptoken,
        1
    );

    let report = runner.run_once().await.expect("restart completes");

    assert!(report.categories[0].caught_up);
    assert_eq!(
        server.requests().await,
        vec![
            "/SyncFeed/2.0/Feed?category=Document&content=internal".to_owned(),
            "/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal".to_owned(),
        ]
    );
}

#[tokio::test]
async fn category_workers_overlap_while_each_category_preserves_cursor_order() {
    let server = TestServer::start(Vec::new()).await;
    let document_next = server.cursor("Document", 1);
    let document_resume = server.cursor("Document", 2);
    let zaak_next = server.cursor("Zaak", 1);
    let zaak_resume = server.cursor("Zaak", 2);
    server
        .replace_responses(vec![
            document_page("Document", &document_next)
                .for_target("/SyncFeed/2.0/Feed?category=Document&content=internal")
                .with_delay(Duration::from_millis(80)),
            document_page("Zaak", &zaak_next)
                .for_target("/SyncFeed/2.0/Feed?category=Zaak&content=internal")
                .with_delay(Duration::from_millis(80)),
            resume_page(&document_resume)
                .for_target("/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal"),
            resume_page(&zaak_resume)
                .for_target("/SyncFeed/2.0/Feed?category=Zaak&skiptoken=1&content=internal"),
        ])
        .await;
    let runner = runner(&server, MemoryStore::default(), ["Document", "Zaak"]);

    let report = runner.run_once().await.expect("sync completes");

    assert_eq!(report.categories.len(), 2);
    assert_eq!(server.max_active_requests(), 2);
    let requests = server.requests().await;
    assert_before(
        &requests,
        "/SyncFeed/2.0/Feed?category=Document&content=internal",
        "/SyncFeed/2.0/Feed?category=Document&skiptoken=1&content=internal",
    );
    assert_before(
        &requests,
        "/SyncFeed/2.0/Feed?category=Zaak&content=internal",
        "/SyncFeed/2.0/Feed?category=Zaak&skiptoken=1&content=internal",
    );
}

#[tokio::test]
async fn empty_resume_page_marks_category_caught_up() {
    let server = TestServer::start(Vec::new()).await;
    let resume = server.cursor("Document", 9);
    server
        .replace_responses(vec![resume_page(&resume)
            .for_target("/SyncFeed/2.0/Feed?category=Document&content=internal")])
        .await;
    let store = MemoryStore::default();
    let runner = runner(&server, store.clone(), ["Document"]);

    let report = runner.run_once().await.expect("sync completes");

    assert!(report.categories[0].caught_up);
    let cursor = store.cursor("Document").await.expect("stored cursor");
    assert!(cursor.caught_up);
    assert_eq!(cursor.latest_skiptoken, 9);
    assert_eq!(cursor.next_url.as_str(), resume);
}

#[tokio::test]
async fn fractie_zetel_vacature_initial_fetch_timeouts_retry_same_cursor_without_durable_error() {
    let server = TestServer::start(Vec::new()).await;
    let resume = server.cursor("FractieZetelVacature", 11);
    let initial = "/SyncFeed/2.0/Feed?category=FractieZetelVacature&content=internal";
    server
        .replace_responses(vec![
            resume_page(&resume)
                .for_target(initial)
                .with_delay(Duration::from_millis(80)),
            resume_page(&resume)
                .for_target(initial)
                .with_delay(Duration::from_millis(80)),
            resume_page(&resume)
                .for_target(initial)
                .with_delay(Duration::from_millis(80)),
            resume_page(&resume).for_target(initial),
        ])
        .await;
    let store = MemoryStore::default();
    let mut client_config = syncfeed_client_config(&server);
    client_config.request_timeout = Duration::from_millis(10);
    client_config.max_retries = 0;
    let runner = runner_with_client_config(
        client_config,
        store.clone(),
        ["FractieZetelVacature"],
        Duration::from_millis(1),
    );

    let report = runner.run_once().await.expect("sync recovers");

    assert!(report.categories[0].caught_up);
    let cursor = store
        .cursor("FractieZetelVacature")
        .await
        .expect("stored cursor");
    assert!(cursor.caught_up);
    assert_eq!(cursor.latest_skiptoken, 11);
    assert_eq!(cursor.next_url.as_str(), resume);
    assert!(store.errors().await.is_empty());
    assert_eq!(
        server.requests().await,
        vec![
            initial.to_owned(),
            initial.to_owned(),
            initial.to_owned(),
            initial.to_owned()
        ]
    );
}

#[tokio::test]
async fn toezegging_initial_body_decode_error_retries_same_cursor_without_durable_error() {
    let server = TestServer::start(Vec::new()).await;
    let resume = server.cursor("Toezegging", 17);
    let initial = "/SyncFeed/2.0/Feed?category=Toezegging&content=internal";
    server
        .replace_responses(vec![
            TestResponse::truncated_atom(initial),
            resume_page(&resume).for_target(initial),
        ])
        .await;
    let store = MemoryStore::default();
    let mut client_config = syncfeed_client_config(&server);
    client_config.max_retries = 0;
    let runner = runner_with_client_config(
        client_config,
        store.clone(),
        ["Toezegging"],
        Duration::from_millis(1),
    );

    let report = runner.run_once().await.expect("sync recovers");

    assert!(report.categories[0].caught_up);
    let cursor = store.cursor("Toezegging").await.expect("stored cursor");
    assert!(cursor.caught_up);
    assert_eq!(cursor.latest_skiptoken, 17);
    assert_eq!(cursor.next_url.as_str(), resume);
    assert!(store.errors().await.is_empty());
    assert_eq!(
        server.requests().await,
        vec![initial.to_owned(), initial.to_owned()]
    );
}

#[tokio::test]
async fn fetch_and_parse_errors_are_recorded_durably() {
    let server = TestServer::start(Vec::new()).await;
    server
        .replace_responses(vec![TestResponse::status(
            "/SyncFeed/2.0/Feed?category=Document&content=internal",
            500,
            "failed",
        )])
        .await;
    let store = MemoryStore::default();
    let fetch_runner = runner(&server, store.clone(), ["Document"]);
    assert!(matches!(
        fetch_runner.run_once().await,
        Err(CompleteSyncError::Fetch { .. })
    ));
    assert_eq!(store.errors().await[0].phase, SyncPhase::Fetch);

    let parse_server = TestServer::start(Vec::new()).await;
    let next = parse_server.cursor("Document", 1);
    parse_server
        .replace_responses(vec![invalid_document_page(&next)
            .for_target("/SyncFeed/2.0/Feed?category=Document&content=internal")])
        .await;
    let parse_store = MemoryStore::default();
    let parse_runner = runner(&parse_server, parse_store.clone(), ["Document"]);
    assert!(matches!(
        parse_runner.run_once().await,
        Err(CompleteSyncError::Parse { .. })
    ));
    let errors = parse_store.errors().await;
    assert_eq!(errors[0].phase, SyncPhase::Parse);
    assert_eq!(errors[0].category, "Document");
    assert_eq!(errors[0].skiptoken, Some(1));
    assert!(!errors[0].message.is_empty());
}

fn runner<const N: usize>(
    server: &TestServer,
    store: MemoryStore,
    categories: [&str; N],
) -> CompleteSyncRunner<MemoryStore> {
    runner_with_client_config(
        syncfeed_client_config(server),
        store,
        categories,
        Duration::from_millis(1),
    )
}

fn runner_with_client_config<const N: usize>(
    client_config: SyncFeedClientConfig,
    store: MemoryStore,
    categories: [&str; N],
    poll_interval: Duration,
) -> CompleteSyncRunner<MemoryStore> {
    let client = SyncFeedClient::new(client_config).expect("valid client config");
    CompleteSyncRunner {
        client,
        store,
        config: CompleteSyncConfig {
            categories: categories.into_iter().map(str::to_owned).collect(),
            mode: SyncRunMode::UntilCaughtUp,
            poll_interval,
        },
    }
}

fn syncfeed_client_config(server: &TestServer) -> SyncFeedClientConfig {
    SyncFeedClientConfig {
        base_url: Url::parse(&server.base_url).expect("mock server URL"),
        content_mode: SyncFeedContentMode::Internal,
        request_timeout: Duration::from_secs(2),
        connect_timeout: Duration::from_secs(2),
        max_retries: 0,
        initial_retry_delay: Duration::from_millis(1),
        max_retry_delay: Duration::from_millis(1),
        max_concurrent_requests: NonZeroUsize::new(2).expect("non-zero"),
    }
}

#[derive(Clone, Default)]
struct MemoryStore {
    inner: Arc<Mutex<MemoryStoreState>>,
}

#[derive(Default)]
struct MemoryStoreState {
    cursors: HashMap<String, StoredCategoryCursor>,
    entities: HashMap<(String, Uuid), ParsedEntity>,
    errors: Vec<DurableSyncError>,
    fail_next_write: Option<WriteFailure>,
}

#[derive(Clone, Copy)]
enum WriteFailure {
    BeforeCommit,
    AfterCommit,
}

impl MemoryStore {
    async fn fail_next_write_before_commit(&self) {
        self.inner.lock().await.fail_next_write = Some(WriteFailure::BeforeCommit);
    }

    async fn fail_next_write_after_commit(&self) {
        self.inner.lock().await.fail_next_write = Some(WriteFailure::AfterCommit);
    }

    async fn cursor(&self, category: &str) -> Option<StoredCategoryCursor> {
        self.inner.lock().await.cursors.get(category).cloned()
    }

    async fn entity_count(&self, category: &str) -> usize {
        self.inner
            .lock()
            .await
            .entities
            .keys()
            .filter(|(entity_category, _)| entity_category == category)
            .count()
    }

    async fn errors(&self) -> Vec<DurableSyncError> {
        self.inner.lock().await.errors.clone()
    }
}

impl SyncStore for MemoryStore {
    fn load_category_cursor<'a>(
        &'a self,
        category: &'a str,
    ) -> SyncStoreFuture<'a, Option<StoredCategoryCursor>> {
        Box::pin(async move { Ok(self.inner.lock().await.cursors.get(category).cloned()) })
    }

    fn write_page(&self, page: PreparedSyncPage) -> SyncStoreFuture<'_, SyncStoreWriteOutcome> {
        Box::pin(async move {
            let mut state = self.inner.lock().await;
            let failure = state.fail_next_write.take();
            if matches!(failure, Some(WriteFailure::BeforeCommit)) {
                return Err(SyncStoreError {
                    message: "crash before commit".to_owned(),
                });
            }
            for entity in &page.entities {
                state
                    .entities
                    .insert((entity.category.clone(), entity.source_id), entity.clone());
            }
            state.cursors.insert(
                page.category.clone(),
                StoredCategoryCursor {
                    category: page.category.clone(),
                    latest_skiptoken: page.latest_skiptoken,
                    next_url: page.next_url,
                    caught_up: false,
                },
            );
            if matches!(failure, Some(WriteFailure::AfterCommit)) {
                return Err(SyncStoreError {
                    message: "crash after commit".to_owned(),
                });
            }
            Ok(SyncStoreWriteOutcome {
                entities_seen: page.entities.len(),
                entities_written: page
                    .entities
                    .iter()
                    .filter(|entity| !entity.deleted)
                    .count(),
                entities_deleted: page.entities.iter().filter(|entity| entity.deleted).count(),
            })
        })
    }

    fn mark_caught_up<'a>(
        &'a self,
        category: &'a str,
        resume_url: Url,
        _observed_at: DateTime<Utc>,
    ) -> SyncStoreFuture<'a, ()> {
        Box::pin(async move {
            let latest_skiptoken = opentk_sync::runner::skiptoken_from_url(&resume_url)
                .ok_or_else(|| SyncStoreError {
                    message: "resume URL missing skiptoken".to_owned(),
                })?;
            self.inner.lock().await.cursors.insert(
                category.to_owned(),
                StoredCategoryCursor {
                    category: category.to_owned(),
                    latest_skiptoken,
                    next_url: resume_url,
                    caught_up: true,
                },
            );
            Ok(())
        })
    }

    fn record_error(&self, error: DurableSyncError) -> SyncStoreFuture<'_, ()> {
        Box::pin(async move {
            self.inner.lock().await.errors.push(error);
            Ok(())
        })
    }

    fn status<'a>(&'a self, categories: &'a [String]) -> SyncStoreFuture<'a, Vec<CategoryStatus>> {
        Box::pin(async move {
            let state = self.inner.lock().await;
            Ok(categories
                .iter()
                .map(|category| {
                    let cursor = state.cursors.get(category);
                    CategoryStatus {
                        category: category.clone(),
                        latest_skiptoken: cursor.map(|cursor| cursor.latest_skiptoken),
                        state: if cursor.is_some_and(|cursor| cursor.caught_up) {
                            CategorySyncState::CaughtUp
                        } else {
                            CategorySyncState::NotStarted
                        },
                        lag: None,
                        last_fetch_at: None,
                        last_error: state
                            .errors
                            .iter()
                            .rev()
                            .find(|error| error.category == *category)
                            .cloned(),
                    }
                })
                .collect())
        })
    }
}

#[derive(Clone, Debug)]
struct TestResponse {
    expected_target: String,
    status: u16,
    content_type: Option<&'static str>,
    body: String,
    delay: Duration,
    declared_content_length: Option<usize>,
}

impl TestResponse {
    fn atom(expected_target: &str, body: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status: 200,
            content_type: Some("application/atom+xml"),
            body: body.to_owned(),
            delay: Duration::ZERO,
            declared_content_length: None,
        }
    }

    fn status(expected_target: &str, status: u16, body: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status,
            content_type: Some("application/atom+xml"),
            body: body.to_owned(),
            delay: Duration::ZERO,
            declared_content_length: None,
        }
    }

    fn truncated_atom(expected_target: &str) -> Self {
        Self {
            expected_target: expected_target.to_owned(),
            status: 200,
            content_type: Some("application/atom+xml"),
            body: "<feed xmlns=\"http://www.w3.org/2005/Atom\">".to_owned(),
            delay: Duration::ZERO,
            declared_content_length: Some(512),
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

#[derive(Clone, Debug)]
struct TestServer {
    base_url: String,
    responses: Arc<Mutex<VecDeque<TestResponse>>>,
    requests: Arc<Mutex<Vec<String>>>,
    active_requests: Arc<AtomicUsize>,
    max_active_requests: Arc<AtomicUsize>,
}

impl TestServer {
    async fn start(responses: Vec<TestResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let local_addr = listener.local_addr().expect("local address");
        let server = Self {
            base_url: format!("http://{local_addr}"),
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            requests: Arc::new(Mutex::new(Vec::new())),
            active_requests: Arc::new(AtomicUsize::new(0)),
            max_active_requests: Arc::new(AtomicUsize::new(0)),
        };
        let server_task = server.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let server = server_task.clone();
                tokio::spawn(async move {
                    let mut buffer = vec![0; 8192];
                    let read = stream.read(&mut buffer).await.expect("read request");
                    let request = String::from_utf8_lossy(&buffer[..read]);
                    let target = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .expect("request target")
                        .to_owned();
                    server.requests.lock().await.push(target.clone());
                    let active = server.active_requests.fetch_add(1, Ordering::SeqCst) + 1;
                    server
                        .max_active_requests
                        .fetch_max(active, Ordering::SeqCst);
                    let response = server
                        .responses
                        .lock()
                        .await
                        .pop_front()
                        .unwrap_or_else(|| panic!("unexpected request {target}"));
                    assert_eq!(target, response.expected_target);
                    if !response.delay.is_zero() {
                        tokio::time::sleep(response.delay).await;
                    }
                    server.active_requests.fetch_sub(1, Ordering::SeqCst);
                    let content_type = response.content_type.unwrap_or("text/plain");
                    let status_text = if response.status == 200 {
                        "OK"
                    } else {
                        "ERROR"
                    };
                    let http_response = format!(
                        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response.status,
                        status_text,
                        content_type,
                        response.declared_content_length.unwrap_or(response.body.len()),
                        response.body
                    );
                    stream
                        .write_all(http_response.as_bytes())
                        .await
                        .expect("write response");
                });
            }
        });
        server
    }

    async fn replace_responses(&self, responses: Vec<TestResponse>) {
        *self.responses.lock().await = VecDeque::from(responses);
        self.requests.lock().await.clear();
        self.active_requests.store(0, Ordering::SeqCst);
        self.max_active_requests.store(0, Ordering::SeqCst);
    }

    fn cursor(&self, category: &str, skiptoken: i64) -> String {
        format!(
            "{}/SyncFeed/2.0/Feed?category={category}&skiptoken={skiptoken}&content=internal",
            self.base_url
        )
    }

    async fn requests(&self) -> Vec<String> {
        self.requests.lock().await.clone()
    }

    fn max_active_requests(&self) -> usize {
        self.max_active_requests.load(Ordering::SeqCst)
    }
}

fn document_page(category: &str, next: &str) -> TestResponse {
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
                <content type="application/xml"><![CDATA[{}]]></content>
              </entry>
            </feed>
            "#,
            xml_url(next),
            entity_xml(category)
        ),
    )
}

fn invalid_document_page(next: &str) -> TestResponse {
    TestResponse::atom(
        "",
        &format!(
            r#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <entry>
                <id>Document-1</id>
                <category term="Document" />
                <updated>2026-04-26T00:00:00Z</updated>
                <link rel="next" href="{}" />
                <content type="application/xml"><![CDATA[<document></document>]]></content>
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

fn entity_xml(category: &str) -> &'static str {
    match category {
        "Document" => {
            r#"<document xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
                id="11111111-1111-4111-8111-111111111111"
                verwijderd="false"
                bijgewerkt="2026-04-26T00:00:00Z"
                contentType="application/pdf"
                contentLength="12345">
                <documentNummer>2026D00001</documentNummer>
                <onderwerp>Runner task</onderwerp>
                <datum>2026-04-26T00:00:00Z</datum>
                <volgnummer>1</volgnummer>
                <vergaderjaar>2025-2026</vergaderjaar>
                <kamer>2</kamer>
            </document>"#
        }
        "Zaak" => {
            r#"<zaak xmlns="http://www.tweedekamer.nl/xsd/tkData/v1-0"
                id="22222222-2222-4222-8222-222222222222"
                verwijderd="false"
                bijgewerkt="2026-04-26T00:00:00Z"/>"#
        }
        _ => panic!("unsupported test category {category}"),
    }
}

fn xml_url(url: &str) -> String {
    url.replace('&', "&amp;")
}

fn assert_before(requests: &[String], first: &str, second: &str) {
    let first_index = requests
        .iter()
        .position(|request| request == first)
        .unwrap_or_else(|| panic!("missing request {first}"));
    let second_index = requests
        .iter()
        .position(|request| request == second)
        .unwrap_or_else(|| panic!("missing request {second}"));
    assert!(
        first_index < second_index,
        "{first} must happen before {second}; got {requests:?}"
    );
}
