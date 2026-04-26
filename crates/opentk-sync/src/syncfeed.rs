use std::{
    fmt,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{StatusCode, Url};
use roxmltree::{Document, Node};
use thiserror::Error;
use tokio::{
    sync::{Mutex, Semaphore},
    task::JoinSet,
    time::sleep,
};

const SYNCFEED_PATH: &str = "/SyncFeed/2.0/Feed";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncFeedContentMode {
    Internal,
    External,
}

impl SyncFeedContentMode {
    const fn query_value(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::External => "external",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyncFeedClientConfig {
    pub base_url: Url,
    pub content_mode: SyncFeedContentMode,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub max_retries: u32,
    pub initial_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub max_concurrent_requests: NonZeroUsize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CategoryCursor {
    Start,
    FromNextUrl(Url),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncFeedCursor {
    pub category: String,
    pub url: Url,
}

impl SyncFeedCursor {
    #[must_use]
    pub fn first_page(
        base_url: &Url,
        category: impl Into<String>,
        content_mode: SyncFeedContentMode,
    ) -> Self {
        let category = category.into();
        let mut url = base_url.clone();
        url.set_path(SYNCFEED_PATH);
        url.set_query(None);
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("category", &category);
            pairs.append_pair("content", content_mode.query_value());
        }
        Self { category, url }
    }

    /// Build a cursor from a `SyncFeed` `next` or `resume` URL.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL is outside the configured `SyncFeed` origin,
    /// has the wrong path, omits required cursor state, or names another
    /// category/content mode.
    pub fn from_url(
        base_url: &Url,
        category: impl Into<String>,
        url: Url,
        content_mode: SyncFeedContentMode,
    ) -> Result<Self, SyncFeedClientError> {
        let category = category.into();
        validate_cursor_url(base_url, &category, &url, content_mode, true)?;
        Ok(Self { category, url })
    }
}

#[derive(Clone, Debug)]
pub struct SyncFeedPage {
    pub category: String,
    pub request_url: Url,
    pub entries: Vec<SyncFeedEntry>,
    pub next_request: Option<SyncFeedCursor>,
    pub resume: Option<SyncFeedCursor>,
    pub outcome: SyncFeedRequestOutcome,
    pub raw_xml: String,
}

#[derive(Clone, Debug)]
pub struct SyncFeedEntry {
    pub id: String,
    pub category: String,
    pub updated: String,
    pub next: SyncFeedCursor,
    pub content_xml: Option<String>,
    pub enclosure_url: Option<Url>,
}

#[derive(Clone, Debug)]
pub struct SyncFeedRequestOutcome {
    pub status: StatusCode,
    pub latency: Duration,
    pub attempts: u32,
    pub rate_limited: bool,
}

#[derive(Clone, Debug)]
struct SharedLimiter {
    semaphore: Arc<Semaphore>,
    adaptive: Arc<Mutex<AdaptiveLimiterState>>,
}

#[derive(Clone, Debug)]
struct AdaptiveLimiterState {
    pacing_delay: Duration,
}

#[derive(Clone, Debug)]
pub struct SyncFeedClient {
    http: reqwest::Client,
    config: SyncFeedClientConfig,
    limiter: SharedLimiter,
}

impl SyncFeedClient {
    /// Create a `SyncFeed` client with shared concurrency and pacing state.
    ///
    /// # Errors
    ///
    /// Returns an error when the base URL cannot be used as a base URL or the
    /// underlying HTTP client cannot be constructed.
    pub fn new(config: SyncFeedClientConfig) -> Result<Self, SyncFeedClientError> {
        if config.base_url.cannot_be_a_base() {
            return Err(SyncFeedClientError::InvalidBaseUrl {
                url: config.base_url.to_string(),
            });
        }
        let http = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|source| SyncFeedClientError::RequestBuild {
                message: source.to_string(),
            })?;
        let limiter = SharedLimiter {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests.get())),
            adaptive: Arc::new(Mutex::new(AdaptiveLimiterState {
                pacing_delay: Duration::ZERO,
            })),
        };
        Ok(Self {
            http,
            config,
            limiter,
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.config.base_url
    }

    #[must_use]
    pub const fn content_mode(&self) -> SyncFeedContentMode {
        self.config.content_mode
    }

    /// Fetch and parse one `SyncFeed` page.
    ///
    /// # Errors
    ///
    /// Returns explicit errors for transport failures, timeouts, non-success
    /// statuses, invalid XML/content type, malformed cursor links, and retry
    /// exhaustion.
    pub async fn fetch_page(
        &self,
        cursor: SyncFeedCursor,
    ) -> Result<SyncFeedPage, SyncFeedClientError> {
        self.fetch_page_with_retries(cursor).await
    }

    /// Fetch one category sequentially until an empty page with `resume`.
    ///
    /// # Errors
    ///
    /// Returns any page-fetching error and also fails when an empty page omits
    /// the required `resume` cursor.
    pub async fn fetch_category_until_resume(
        &self,
        category: impl Into<String>,
        start: CategoryCursor,
    ) -> Result<Vec<SyncFeedPage>, SyncFeedClientError> {
        let category = category.into();
        let mut cursor = match start {
            CategoryCursor::Start => SyncFeedCursor::first_page(
                &self.config.base_url,
                category.clone(),
                self.config.content_mode,
            ),
            CategoryCursor::FromNextUrl(url) => SyncFeedCursor::from_url(
                &self.config.base_url,
                category.clone(),
                url,
                self.config.content_mode,
            )?,
        };
        let mut pages = Vec::new();

        loop {
            let page = self.fetch_page(cursor).await?;
            if page.entries.is_empty() {
                if page.resume.is_none() {
                    return Err(SyncFeedClientError::InvalidResumePage {
                        category,
                        request_url: page.request_url.to_string(),
                    });
                }
                pages.push(page);
                return Ok(pages);
            }

            let next = page.next_request.clone().ok_or_else(|| {
                SyncFeedClientError::MissingCursorState {
                    category: category.clone(),
                    request_url: page.request_url.to_string(),
                }
            })?;
            pages.push(page);
            cursor = next;
        }
    }

    /// Fetch several categories concurrently while preserving order per category.
    ///
    /// # Errors
    ///
    /// Returns any worker page-fetching error or a worker join failure.
    pub async fn fetch_categories_until_resume<I, S>(
        &self,
        categories: I,
    ) -> Result<Vec<(String, Vec<SyncFeedPage>)>, SyncFeedClientError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut workers = JoinSet::new();
        for category in categories {
            let category = category.into();
            let client = self.clone();
            workers.spawn(async move {
                let pages = client
                    .fetch_category_until_resume(category.clone(), CategoryCursor::Start)
                    .await?;
                Ok::<_, SyncFeedClientError>((category, pages))
            });
        }

        let mut pages_by_category = Vec::new();
        while let Some(result) = workers.join_next().await {
            let category_pages = result.map_err(|source| SyncFeedClientError::WorkerJoin {
                message: source.to_string(),
            })??;
            pages_by_category.push(category_pages);
        }
        Ok(pages_by_category)
    }

    pub async fn current_pacing_delay(&self) -> Duration {
        self.limiter.adaptive.lock().await.pacing_delay
    }

    async fn fetch_page_with_retries(
        &self,
        cursor: SyncFeedCursor,
    ) -> Result<SyncFeedPage, SyncFeedClientError> {
        let mut attempt = 1;
        let mut delay = self.config.initial_retry_delay;

        loop {
            match self.fetch_page_once(cursor.clone(), attempt).await {
                Ok(FetchAttempt::RetryableStatus(outcome)) => {
                    if attempt > self.config.max_retries {
                        return Err(SyncFeedClientError::RetryExhausted {
                            attempts: attempt,
                            last_status: Some(outcome.status),
                            last_error: None,
                        });
                    }
                }
                Ok(FetchAttempt::Page(page)) => return Ok(*page),
                Err(error @ SyncFeedClientError::Timeout { .. }) => {
                    if attempt > self.config.max_retries {
                        return Err(error);
                    }
                }
                Err(error) => return Err(error),
            }

            sleep(delay).await;
            delay = delay.saturating_mul(2).min(self.config.max_retry_delay);
            attempt += 1;
        }
    }

    async fn fetch_page_once(
        &self,
        cursor: SyncFeedCursor,
        attempts: u32,
    ) -> Result<FetchAttempt, SyncFeedClientError> {
        let _permit = self
            .limiter
            .semaphore
            .acquire()
            .await
            .map_err(|_| SyncFeedClientError::LimiterClosed)?;
        let pacing_delay = self.current_pacing_delay().await;
        if !pacing_delay.is_zero() {
            sleep(pacing_delay).await;
        }
        let started = Instant::now();
        let response = self
            .http
            .get(cursor.url.clone())
            .timeout(self.config.request_timeout)
            .send()
            .await
            .map_err(|source| classify_reqwest_error(&source, &cursor.url))?;
        let latency = started.elapsed();
        let status = response.status();
        let rate_limited = status == StatusCode::TOO_MANY_REQUESTS;
        let outcome = SyncFeedRequestOutcome {
            status,
            latency,
            attempts,
            rate_limited,
        };
        self.record_limiter_observation(status).await;

        if !status.is_success() {
            if should_retry_status(status) {
                return Ok(FetchAttempt::RetryableStatus(outcome));
            }
            return Err(SyncFeedClientError::NonSuccessStatus {
                status,
                request_url: cursor.url.to_string(),
                latency,
            });
        }

        validate_content_type(
            response.headers().get(reqwest::header::CONTENT_TYPE),
            &cursor.url,
        )?;
        let raw_xml =
            response
                .text()
                .await
                .map_err(|source| SyncFeedClientError::HttpTransport {
                    request_url: cursor.url.to_string(),
                    message: source.to_string(),
                })?;
        parse_page(
            &self.config.base_url,
            self.config.content_mode,
            cursor,
            outcome,
            raw_xml,
        )
        .map(Box::new)
        .map(FetchAttempt::Page)
    }
}

#[derive(Debug)]
enum FetchAttempt {
    Page(Box<SyncFeedPage>),
    RetryableStatus(SyncFeedRequestOutcome),
}

impl SyncFeedClient {
    async fn record_limiter_observation(&self, status: StatusCode) {
        let mut state = self.limiter.adaptive.lock().await;
        if status == StatusCode::TOO_MANY_REQUESTS {
            state.pacing_delay = state
                .pacing_delay
                .saturating_add(self.config.initial_retry_delay)
                .max(self.config.initial_retry_delay)
                .min(self.config.max_retry_delay);
        } else if status.is_success() && !state.pacing_delay.is_zero() {
            state.pacing_delay /= 2;
        }
    }
}

fn should_retry_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn classify_reqwest_error(error: &reqwest::Error, request_url: &Url) -> SyncFeedClientError {
    if error.is_timeout() {
        SyncFeedClientError::Timeout {
            request_url: request_url.to_string(),
            message: error.to_string(),
        }
    } else {
        SyncFeedClientError::HttpTransport {
            request_url: request_url.to_string(),
            message: error.to_string(),
        }
    }
}

fn validate_content_type(
    content_type: Option<&reqwest::header::HeaderValue>,
    request_url: &Url,
) -> Result<(), SyncFeedClientError> {
    let Some(content_type) = content_type else {
        return Err(SyncFeedClientError::InvalidXmlContentType {
            request_url: request_url.to_string(),
            content_type: None,
        });
    };
    let content_type =
        content_type
            .to_str()
            .map_err(|_| SyncFeedClientError::InvalidXmlContentType {
                request_url: request_url.to_string(),
                content_type: Some("<non-utf8>".to_owned()),
            })?;
    if content_type.contains("xml") {
        Ok(())
    } else {
        Err(SyncFeedClientError::InvalidXmlContentType {
            request_url: request_url.to_string(),
            content_type: Some(content_type.to_owned()),
        })
    }
}

fn parse_page(
    base_url: &Url,
    content_mode: SyncFeedContentMode,
    cursor: SyncFeedCursor,
    outcome: SyncFeedRequestOutcome,
    raw_xml: String,
) -> Result<SyncFeedPage, SyncFeedClientError> {
    let document = Document::parse(&raw_xml).map_err(|source| SyncFeedClientError::InvalidXml {
        request_url: cursor.url.to_string(),
        message: source.to_string(),
    })?;
    let feed = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "feed")
        .ok_or_else(|| SyncFeedClientError::InvalidXml {
            request_url: cursor.url.to_string(),
            message: "missing Atom feed element".to_owned(),
        })?;

    let mut entries = Vec::new();
    for entry in feed
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "entry")
    {
        entries.push(parse_entry(
            base_url,
            content_mode,
            &cursor.category,
            &cursor.url,
            &raw_xml,
            entry,
        )?);
    }

    let feed_next = link_href(feed, "next")
        .map(|href| {
            parse_link_cursor(
                base_url,
                &cursor.category,
                &cursor.url,
                href,
                content_mode,
                "feed next",
            )
        })
        .transpose()?;
    let resume = link_href(feed, "resume")
        .map(|href| {
            parse_link_cursor(
                base_url,
                &cursor.category,
                &cursor.url,
                href,
                content_mode,
                "resume",
            )
        })
        .transpose()?;
    let next_request = entries.last().map(|entry| entry.next.clone()).or(feed_next);

    Ok(SyncFeedPage {
        category: cursor.category,
        request_url: cursor.url,
        entries,
        next_request,
        resume,
        outcome,
        raw_xml,
    })
}

fn parse_entry(
    base_url: &Url,
    content_mode: SyncFeedContentMode,
    expected_category: &str,
    request_url: &Url,
    raw_xml: &str,
    entry: Node<'_, '_>,
) -> Result<SyncFeedEntry, SyncFeedClientError> {
    let id = child_text(entry, "id").ok_or_else(|| SyncFeedClientError::InvalidXml {
        request_url: request_url.to_string(),
        message: "entry missing id".to_owned(),
    })?;
    let category = entry
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "category")
        .and_then(|node| node.attribute("term"))
        .ok_or_else(|| SyncFeedClientError::MissingCursorState {
            category: expected_category.to_owned(),
            request_url: request_url.to_string(),
        })?
        .to_owned();
    if !categories_match(&category, expected_category) {
        return Err(SyncFeedClientError::CategoryMismatch {
            expected: expected_category.to_owned(),
            actual: category,
            cursor_url: request_url.to_string(),
        });
    }
    let updated = child_text(entry, "updated").ok_or_else(|| SyncFeedClientError::InvalidXml {
        request_url: request_url.to_string(),
        message: "entry missing updated".to_owned(),
    })?;
    let next_href =
        link_href(entry, "next").ok_or_else(|| SyncFeedClientError::MissingEntryNext {
            category: expected_category.to_owned(),
            request_url: request_url.to_string(),
            entry_id: id.clone(),
        })?;
    let next = parse_link_cursor(
        base_url,
        expected_category,
        request_url,
        next_href,
        content_mode,
        "entry next",
    )?;
    let content_xml = entry
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "content")
        .and_then(|node| content_payload_xml(raw_xml, node));
    let enclosure_url = link_href(entry, "enclosure")
        .map(|href| {
            request_url
                .join(href)
                .map_err(|source| SyncFeedClientError::MalformedCursorUrl {
                    context: "entry enclosure".to_owned(),
                    cursor_url: href.to_owned(),
                    message: source.to_string(),
                })
        })
        .transpose()?;

    Ok(SyncFeedEntry {
        id,
        category: expected_category.to_owned(),
        updated,
        next,
        content_xml,
        enclosure_url,
    })
}

fn content_payload_xml(raw_xml: &str, content: Node<'_, '_>) -> Option<String> {
    content
        .children()
        .find(Node::is_element)
        .map(|node| raw_xml[node.range()].to_owned())
        .or_else(|| content.text().map(str::to_owned))
}

fn child_text(node: Node<'_, '_>, child_name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == child_name)
        .and_then(|child| child.text())
        .map(str::to_owned)
}

fn link_href<'a>(node: Node<'a, 'a>, rel: &str) -> Option<&'a str> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name() == "link")
        .find(|child| child.attribute("rel") == Some(rel))
        .and_then(|child| child.attribute("href"))
}

fn parse_link_cursor(
    base_url: &Url,
    category: &str,
    request_url: &Url,
    href: &str,
    content_mode: SyncFeedContentMode,
    context: &str,
) -> Result<SyncFeedCursor, SyncFeedClientError> {
    let url = request_url
        .join(href)
        .map_err(|source| SyncFeedClientError::MalformedCursorUrl {
            context: context.to_owned(),
            cursor_url: href.to_owned(),
            message: source.to_string(),
        })?;
    SyncFeedCursor::from_url(base_url, category.to_owned(), url, content_mode)
}

fn validate_cursor_url(
    base_url: &Url,
    category: &str,
    url: &Url,
    content_mode: SyncFeedContentMode,
    require_skiptoken: bool,
) -> Result<(), SyncFeedClientError> {
    if url.scheme() != base_url.scheme()
        || url.host_str() != base_url.host_str()
        || url.port_or_known_default() != base_url.port_or_known_default()
    {
        return Err(SyncFeedClientError::MalformedCursorUrl {
            context: "cursor origin".to_owned(),
            cursor_url: url.to_string(),
            message: "cursor URL does not match SyncFeed base URL".to_owned(),
        });
    }
    if url.path() != SYNCFEED_PATH {
        return Err(SyncFeedClientError::MalformedCursorUrl {
            context: "cursor path".to_owned(),
            cursor_url: url.to_string(),
            message: format!("expected path {SYNCFEED_PATH}"),
        });
    }

    let mut found_category = None;
    let mut found_skiptoken = None;
    let mut found_content = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "category" => found_category = Some(value.into_owned()),
            "skiptoken" => found_skiptoken = Some(value.into_owned()),
            "content" => found_content = Some(value.into_owned()),
            _ => {}
        }
    }

    let actual_category =
        found_category.ok_or_else(|| SyncFeedClientError::MissingCursorState {
            category: category.to_owned(),
            request_url: url.to_string(),
        })?;
    if !categories_match(&actual_category, category) {
        return Err(SyncFeedClientError::CategoryMismatch {
            expected: category.to_owned(),
            actual: actual_category,
            cursor_url: url.to_string(),
        });
    }
    if require_skiptoken && found_skiptoken.is_none() {
        return Err(SyncFeedClientError::MissingCursorState {
            category: category.to_owned(),
            request_url: url.to_string(),
        });
    }
    if let Some(content) = found_content {
        let expected = content_mode.query_value();
        if content != expected {
            return Err(SyncFeedClientError::MalformedCursorUrl {
                context: "cursor content mode".to_owned(),
                cursor_url: url.to_string(),
                message: format!("expected content={expected}, got content={content}"),
            });
        }
    }
    Ok(())
}

fn categories_match(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

#[derive(Debug, Error)]
pub enum SyncFeedClientError {
    #[error("invalid SyncFeed base URL: {url}")]
    InvalidBaseUrl { url: String },
    #[error("failed to build SyncFeed request client: {message}")]
    RequestBuild { message: String },
    #[error("SyncFeed request timed out for {request_url}: {message}")]
    Timeout {
        request_url: String,
        message: String,
    },
    #[error("SyncFeed HTTP transport failed for {request_url}: {message}")]
    HttpTransport {
        request_url: String,
        message: String,
    },
    #[error("SyncFeed returned non-success status {status} for {request_url} after {latency:?}")]
    NonSuccessStatus {
        status: StatusCode,
        request_url: String,
        latency: Duration,
    },
    #[error("SyncFeed XML content type was invalid for {request_url}: {content_type:?}")]
    InvalidXmlContentType {
        request_url: String,
        content_type: Option<String>,
    },
    #[error("SyncFeed XML was invalid for {request_url}: {message}")]
    InvalidXml {
        request_url: String,
        message: String,
    },
    #[error("SyncFeed cursor state is missing for category {category} at {request_url}")]
    MissingCursorState {
        category: String,
        request_url: String,
    },
    #[error("SyncFeed cursor URL is malformed in {context}: {cursor_url}: {message}")]
    MalformedCursorUrl {
        context: String,
        cursor_url: String,
        message: String,
    },
    #[error(
        "SyncFeed cursor category mismatch: expected {expected}, got {actual} at {cursor_url}"
    )]
    CategoryMismatch {
        expected: String,
        actual: String,
        cursor_url: String,
    },
    #[error(
        "SyncFeed entry {entry_id} in category {category} is missing a next link at {request_url}"
    )]
    MissingEntryNext {
        category: String,
        request_url: String,
        entry_id: String,
    },
    #[error("SyncFeed empty page for category {category} at {request_url} did not include resume")]
    InvalidResumePage {
        category: String,
        request_url: String,
    },
    #[error("SyncFeed retry policy exhausted after {attempts} attempts; last status={last_status:?}; last error={last_error:?}")]
    RetryExhausted {
        attempts: u32,
        last_status: Option<StatusCode>,
        last_error: Option<String>,
    },
    #[error("SyncFeed concurrency limiter was closed")]
    LimiterClosed,
    #[error("SyncFeed category worker failed to join: {message}")]
    WorkerJoin { message: String },
}

impl fmt::Display for SyncFeedContentMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.query_value())
    }
}
