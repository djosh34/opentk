use std::{fmt::Write as _, num::NonZeroUsize, time::Duration};

use chrono::{DateTime, Utc};
use reqwest::{header, StatusCode, Url};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{sync::Semaphore, task::JoinSet, time::sleep};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct DocumentAssetFetchConfig {
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub max_retries: u32,
    pub initial_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub max_concurrent_requests: NonZeroUsize,
    pub max_asset_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct DocumentAssetFetcher {
    http: reqwest::Client,
    config: DocumentAssetFetchConfig,
}

impl DocumentAssetFetcher {
    /// Create a document asset fetcher.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client configuration is invalid.
    pub fn new(config: DocumentAssetFetchConfig) -> Result<Self, DocumentAssetFetchError> {
        let http = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|source| DocumentAssetFetchError::RequestBuild {
                message: source.to_string(),
            })?;
        Ok(Self { http, config })
    }

    pub async fn fetch_one(&self, request: DocumentAssetFetchRequest) -> DocumentAssetFetchReport {
        let retrieved_at = Utc::now();
        let upstream = match self.fetch_bytes(request.asset_url.clone()).await {
            Ok(response) => response,
            Err(error) => {
                return failed_report(request, retrieved_at, error);
            }
        };

        if upstream.status == StatusCode::NOT_FOUND || upstream.status == StatusCode::GONE {
            return status_report(request, retrieved_at, RetrievalStatus::NotFound, upstream);
        }

        if !upstream.status.is_success() {
            let error = format!("HTTP status {}", upstream.status.as_u16());
            return status_report_with_error(
                request,
                retrieved_at,
                RetrievalStatus::Failed,
                upstream,
                error,
            );
        }

        if let Err(error) = validate_lengths(&request, &upstream) {
            return status_report_with_error(
                request,
                retrieved_at,
                RetrievalStatus::Failed,
                upstream,
                error,
            );
        }
        if let Err(error) = validate_content_type(&request, &upstream) {
            return status_report_with_error(
                request,
                retrieved_at,
                RetrievalStatus::Failed,
                upstream,
                error,
            );
        }

        let Some(upstream_kind) = classify_content_type(upstream.content_type.as_deref()) else {
            return status_report_with_error(
                request,
                retrieved_at,
                RetrievalStatus::UnsupportedContentType,
                upstream,
                "unsupported content type".to_owned(),
            );
        };

        if matches!(
            upstream_kind,
            DocumentAssetKind::OfficialText | DocumentAssetKind::OfficialTranscript
        ) {
            return fetched_report_for_response(request, retrieved_at, upstream, upstream_kind, 0);
        }

        if let Some(alternative) = self
            .fetch_best_official_alternative(&request.asset_url, &upstream)
            .await
        {
            return match alternative {
                Ok((candidate, candidate_kind, discovered)) => fetched_report_for_alternative(
                    request,
                    retrieved_at,
                    upstream,
                    candidate,
                    candidate_kind,
                    discovered,
                ),
                Err(error) => failed_report(request, retrieved_at, error),
            };
        }

        fetched_report_for_response(request, retrieved_at, upstream, upstream_kind, 10)
    }

    /// Fetch several document assets with the configured concurrency bound.
    ///
    /// # Panics
    ///
    /// Panics if an internal worker task panics or if the owned semaphore is
    /// closed while worker tasks are still running.
    pub async fn fetch_many<I>(&self, requests: I) -> Vec<DocumentAssetFetchReport>
    where
        I: IntoIterator<Item = DocumentAssetFetchRequest>,
    {
        let requests: Vec<_> = requests.into_iter().enumerate().collect();
        let limiter =
            std::sync::Arc::new(Semaphore::new(self.config.max_concurrent_requests.get()));
        let mut workers = JoinSet::new();
        for (index, request) in requests {
            let fetcher = self.clone();
            let limiter = std::sync::Arc::clone(&limiter);
            workers.spawn(async move {
                let _permit = limiter.acquire_owned().await.expect("semaphore is open");
                (index, fetcher.fetch_one(request).await)
            });
        }

        let mut reports = Vec::new();
        while let Some(joined) = workers.join_next().await {
            let (index, report) = joined.expect("document asset worker did not panic");
            reports.push((index, report));
        }
        reports.sort_by_key(|(index, _report)| *index);
        reports.into_iter().map(|(_index, report)| report).collect()
    }

    async fn fetch_best_official_alternative(
        &self,
        base_url: &Url,
        upstream: &FetchedBytes,
    ) -> Option<
        Result<
            (
                FetchedBytes,
                DocumentAssetKind,
                Vec<DocumentDiscoveredSource>,
            ),
            String,
        >,
    > {
        if classify_content_type(upstream.content_type.as_deref())
            != Some(DocumentAssetKind::OfficialHtml)
        {
            return None;
        }
        let html = String::from_utf8_lossy(&upstream.body);
        let discovered = discover_official_sources(base_url, &html);
        let candidate = discovered.first()?;
        let fetched = match self.fetch_bytes(candidate.url.clone()).await {
            Ok(fetched) => fetched,
            Err(error) => return Some(Err(error)),
        };
        if !fetched.status.is_success() {
            return Some(Err(format!(
                "official alternative returned HTTP status {}",
                fetched.status.as_u16()
            )));
        }
        let Some(kind) = classify_content_type(fetched.content_type.as_deref()) else {
            return Some(Err(
                "official alternative has unsupported content type".to_owned()
            ));
        };
        Some(Ok((fetched, kind, discovered)))
    }

    async fn fetch_bytes(&self, url: Url) -> Result<FetchedBytes, String> {
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            match self.fetch_bytes_once(url.clone()).await {
                Ok(response) if !is_retryable_status(response.status) => return Ok(response),
                Ok(response) => {
                    if attempt == self.config.max_retries {
                        return Ok(response);
                    }
                    last_error = Some(format!("HTTP status {}", response.status.as_u16()));
                }
                Err(error) => last_error = Some(error),
            }
            if attempt < self.config.max_retries {
                sleep(
                    self.config
                        .initial_retry_delay
                        .min(self.config.max_retry_delay),
                )
                .await;
            }
        }
        Err(last_error.unwrap_or_else(|| "request failed".to_owned()))
    }

    async fn fetch_bytes_once(&self, url: Url) -> Result<FetchedBytes, String> {
        let response = self
            .http
            .get(url.clone())
            .timeout(self.config.request_timeout)
            .send()
            .await
            .map_err(|source| format_reqwest_error(&source))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(normalize_content_type);
        let content_length = response.content_length();
        let last_modified = None;
        let body = response
            .bytes()
            .await
            .map_err(|source| format_reqwest_error(&source))?
            .to_vec();
        Ok(FetchedBytes {
            url,
            status,
            content_type,
            content_length,
            last_modified,
            body,
        })
    }
}

fn format_reqwest_error(source: &reqwest::Error) -> String {
    if source.is_timeout() {
        format!("request timed out: {source}")
    } else {
        source.to_string()
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

#[derive(Clone, Debug)]
struct FetchedBytes {
    url: Url,
    status: StatusCode,
    content_type: Option<String>,
    content_length: Option<u64>,
    last_modified: Option<DateTime<Utc>>,
    body: Vec<u8>,
}

fn failed_report(
    request: DocumentAssetFetchRequest,
    retrieved_at: DateTime<Utc>,
    error: String,
) -> DocumentAssetFetchReport {
    DocumentAssetFetchReport {
        document_source_category: request.document_source_category,
        document_source_id: request.document_source_id,
        asset_url: request.asset_url.clone(),
        upstream_url: request.asset_url,
        upstream_content_type: None,
        upstream_content_length: None,
        upstream_last_modified_at: None,
        retrieval_status: RetrievalStatus::Failed,
        retrieval_error: Some(error),
        retrieved_at,
        source_hash: None,
        selected_source: None,
        selected_body: None,
        discovered_sources: Vec::new(),
    }
}

fn status_report(
    request: DocumentAssetFetchRequest,
    retrieved_at: DateTime<Utc>,
    status: RetrievalStatus,
    upstream: FetchedBytes,
) -> DocumentAssetFetchReport {
    DocumentAssetFetchReport {
        document_source_category: request.document_source_category,
        document_source_id: request.document_source_id,
        asset_url: request.asset_url,
        upstream_url: upstream.url,
        upstream_content_type: upstream.content_type,
        upstream_content_length: upstream.content_length,
        upstream_last_modified_at: upstream.last_modified,
        retrieval_status: status,
        retrieval_error: None,
        retrieved_at,
        source_hash: None,
        selected_source: None,
        selected_body: None,
        discovered_sources: Vec::new(),
    }
}

fn status_report_with_error(
    request: DocumentAssetFetchRequest,
    retrieved_at: DateTime<Utc>,
    status: RetrievalStatus,
    upstream: FetchedBytes,
    error: String,
) -> DocumentAssetFetchReport {
    let mut report = status_report(request, retrieved_at, status, upstream);
    report.retrieval_error = Some(error);
    report
}

fn fetched_report_for_response(
    request: DocumentAssetFetchRequest,
    retrieved_at: DateTime<Utc>,
    upstream: FetchedBytes,
    kind: DocumentAssetKind,
    source_rank: i32,
) -> DocumentAssetFetchReport {
    let selected = DocumentSelectedSource {
        url: upstream.url.clone(),
        content_type: upstream.content_type.clone(),
        content_length: upstream.content_length,
        official_source: kind.is_official_content(),
        kind,
        source_rank,
    };
    let source_hash = sha256_hex(&upstream.body);
    DocumentAssetFetchReport {
        document_source_category: request.document_source_category,
        document_source_id: request.document_source_id,
        asset_url: request.asset_url,
        upstream_url: upstream.url,
        upstream_content_type: upstream.content_type,
        upstream_content_length: upstream.content_length,
        upstream_last_modified_at: upstream.last_modified,
        retrieval_status: RetrievalStatus::Fetched,
        retrieval_error: None,
        retrieved_at,
        source_hash: Some(source_hash),
        selected_source: Some(selected),
        selected_body: Some(upstream.body),
        discovered_sources: Vec::new(),
    }
}

fn fetched_report_for_alternative(
    request: DocumentAssetFetchRequest,
    retrieved_at: DateTime<Utc>,
    upstream: FetchedBytes,
    candidate: FetchedBytes,
    candidate_kind: DocumentAssetKind,
    discovered_sources: Vec<DocumentDiscoveredSource>,
) -> DocumentAssetFetchReport {
    let selected = DocumentSelectedSource {
        url: candidate.url,
        content_type: candidate.content_type,
        content_length: candidate.content_length,
        kind: candidate_kind,
        official_source: true,
        source_rank: 0,
    };
    DocumentAssetFetchReport {
        document_source_category: request.document_source_category,
        document_source_id: request.document_source_id,
        asset_url: request.asset_url,
        upstream_url: upstream.url,
        upstream_content_type: upstream.content_type,
        upstream_content_length: upstream.content_length,
        upstream_last_modified_at: upstream.last_modified,
        retrieval_status: RetrievalStatus::Fetched,
        retrieval_error: None,
        retrieved_at,
        source_hash: Some(sha256_hex(&candidate.body)),
        selected_source: Some(selected),
        selected_body: Some(candidate.body),
        discovered_sources,
    }
}

fn normalize_content_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}

fn classify_content_type(content_type: Option<&str>) -> Option<DocumentAssetKind> {
    match content_type.map(normalize_content_type).as_deref() {
        Some("text/plain") => Some(DocumentAssetKind::OfficialText),
        Some("text/html" | "application/xhtml+xml") => Some(DocumentAssetKind::OfficialHtml),
        Some("application/pdf") => Some(DocumentAssetKind::Pdf),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document") => {
            Some(DocumentAssetKind::Docx)
        }
        _ => None,
    }
}

fn validate_lengths(
    request: &DocumentAssetFetchRequest,
    response: &FetchedBytes,
) -> Result<(), String> {
    let actual_length = u64::try_from(response.body.len())
        .map_err(|_| "content length exceeds u64 range".to_owned())?;
    if let Some(header_length) = response.content_length {
        if header_length != actual_length {
            return Err(format!(
                "content length header {header_length} did not match actual body length {actual_length}"
            ));
        }
    }
    if let Some(expected_length) = request.expected_content_length {
        if response
            .content_length
            .is_some_and(|header_length| header_length != expected_length)
        {
            return Err(format!(
                "expected content length {expected_length} did not match upstream content length {}",
                response.content_length.expect("checked present")
            ));
        }
        if actual_length != expected_length {
            return Err(format!(
                "expected content length {expected_length} did not match actual body length {actual_length}"
            ));
        }
    }
    Ok(())
}

fn validate_content_type(
    request: &DocumentAssetFetchRequest,
    response: &FetchedBytes,
) -> Result<(), String> {
    let Some(expected) = request.expected_content_type.as_deref() else {
        return Ok(());
    };
    let expected = normalize_content_type(expected);
    let Some(actual) = response.content_type.as_deref() else {
        return Err(format!(
            "expected content type {expected} but upstream omitted content type"
        ));
    };
    if actual != expected {
        return Err(format!(
            "expected content type {expected} did not match upstream content type {actual}"
        ));
    }
    Ok(())
}

fn discover_official_sources(base_url: &Url, html: &str) -> Vec<DocumentDiscoveredSource> {
    html.split('<')
        .filter(|fragment| fragment.trim_start().starts_with("link"))
        .filter(|fragment| has_attribute_value(fragment, "rel", "alternate"))
        .filter_map(|fragment| {
            let href = attribute_value(fragment, "href")?;
            let url = base_url.join(&href).ok()?;
            let content_type = attribute_value(fragment, "type");
            let kind = classify_content_type(content_type.as_deref())?;
            Some(DocumentDiscoveredSource {
                url,
                kind,
                source_rank: 0,
            })
        })
        .filter(|source| source.kind.is_official_content())
        .collect()
}

fn has_attribute_value(fragment: &str, name: &str, expected: &str) -> bool {
    attribute_value(fragment, name).is_some_and(|value| {
        value
            .split_ascii_whitespace()
            .any(|part| part.eq_ignore_ascii_case(expected))
    })
}

fn attribute_value(fragment: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let start = fragment.find(&needle)? + needle.len();
    let quote = fragment[start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = fragment[value_start..].find(quote)? + value_start;
    Some(fragment[value_start..value_end].to_owned())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().fold(String::new(), |mut output, byte| {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        output
    })
}

impl DocumentAssetKind {
    const fn is_official_content(&self) -> bool {
        matches!(
            self,
            Self::OfficialText | Self::OfficialHtml | Self::OfficialTranscript
        )
    }
}

#[derive(Clone, Debug)]
pub struct DocumentAssetFetchRequest {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub asset_url: Url,
    pub expected_content_type: Option<String>,
    pub expected_content_length: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct DocumentAssetFetchReport {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub asset_url: Url,
    pub upstream_url: Url,
    pub upstream_content_type: Option<String>,
    pub upstream_content_length: Option<u64>,
    pub upstream_last_modified_at: Option<DateTime<Utc>>,
    pub retrieval_status: RetrievalStatus,
    pub retrieval_error: Option<String>,
    pub retrieved_at: DateTime<Utc>,
    pub source_hash: Option<String>,
    pub selected_source: Option<DocumentSelectedSource>,
    pub selected_body: Option<Vec<u8>>,
    pub discovered_sources: Vec<DocumentDiscoveredSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetrievalStatus {
    Fetched,
    NotFound,
    UnsupportedContentType,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentAssetKind {
    OfficialText,
    OfficialHtml,
    OfficialTranscript,
    Pdf,
    Docx,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSelectedSource {
    pub url: Url,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub kind: DocumentAssetKind,
    pub official_source: bool,
    pub source_rank: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentDiscoveredSource {
    pub url: Url,
    pub kind: DocumentAssetKind,
    pub source_rank: i32,
}

#[derive(Debug, Error)]
pub enum DocumentAssetFetchError {
    #[error("failed to build HTTP client: {message}")]
    RequestBuild { message: String },
}
