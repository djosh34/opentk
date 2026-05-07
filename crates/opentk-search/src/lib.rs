use std::{collections::BTreeSet, future::Future, pin::Pin};

use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SearchMappingError {
    #[error("search source record is missing required field {field}")]
    MissingRequiredField { field: &'static str },
    #[error("search source record field {field} has unsupported value type {value_type}")]
    UnsupportedFieldType {
        field: String,
        value_type: &'static str,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SearchIndexError {
    #[error("search index HTTP request failed with status {status:?}: {message}")]
    Http {
        status: Option<u16>,
        message: String,
    },
    #[error("search index returned an invalid response: {0}")]
    InvalidResponse(String),
    #[error("search mapping failed")]
    Mapping(#[from] SearchMappingError),
    #[error("search payload item is larger than configured payload limit: item_bytes={item_bytes} max_payload_bytes={max_payload_bytes}")]
    PayloadTooLarge {
        item_bytes: usize,
        max_payload_bytes: usize,
    },
}

#[allow(async_fn_in_trait)]
pub trait SearchIndexClient {
    /// Delete and recreate the target index, then apply schema settings.
    ///
    /// # Errors
    ///
    /// Returns [`SearchIndexError`] when the backing search engine rejects any
    /// index reset or schema operation.
    async fn reset_index(&self, schema: &SearchIndexSchema) -> Result<(), SearchIndexError>;

    /// Apply a batch of upsert/delete operations to the target index.
    ///
    /// # Errors
    ///
    /// Returns [`SearchIndexError`] when the backing search engine rejects the
    /// batch or reports an asynchronous task failure.
    async fn apply_batch(
        &self,
        operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError>;

    /// Apply a batch while respecting a maximum JSON request payload size.
    ///
    /// Implementations that cannot inspect payload sizes may fall back to
    /// [`SearchIndexClient::apply_batch`]. The concrete Meilisearch client
    /// chunks by exact UTF-8 request-body bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SearchIndexError`] when the backing search engine rejects the
    /// batch, reports an asynchronous task failure, or one item cannot fit
    /// within `max_payload_bytes`.
    async fn apply_batch_with_payload_limit(
        &self,
        operations: &[SearchIndexOperation],
        max_payload_bytes: usize,
    ) -> Result<(), SearchIndexError> {
        let _ = max_payload_bytes;
        self.apply_batch(operations).await
    }
}

pub trait SearchQueryClient {
    /// Query indexed source records through a stable search boundary.
    ///
    /// # Errors
    ///
    /// Returns [`SearchIndexError`] when the backing search engine rejects the
    /// request or returns a response that cannot be mapped into stable search
    /// result types.
    fn search<'a>(
        &'a self,
        request: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchIndexError>> + Send + 'a>>;
}

pub trait SearchHealthClient {
    /// Check that the backing search service accepts lightweight authenticated requests.
    ///
    /// # Errors
    ///
    /// Returns [`SearchIndexError`] when the backing search service cannot be
    /// reached or rejects the health request.
    fn health<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<(), SearchIndexError>> + Send + 'a>>;
}

pub trait SearchCountClient {
    /// Count indexed source records matching a stable search filter.
    ///
    /// # Errors
    ///
    /// Returns [`SearchIndexError`] when the backing search service rejects the
    /// count request or returns a response without count metadata.
    fn count<'a>(
        &'a self,
        filter: SearchFilter,
    ) -> Pin<Box<dyn Future<Output = Result<u64, SearchIndexError>> + Send + 'a>>;
}

#[allow(async_fn_in_trait)]
pub trait SearchReconcilerClient {
    /// Create the target index if needed and apply schema settings without
    /// deleting existing documents.
    async fn ensure_index(&self, schema: &SearchIndexSchema) -> Result<(), SearchIndexError>;

    /// Return the highest indexed skiptoken for one source category using the
    /// filtered documents endpoint with `sort = latest_skiptoken:desc` and
    /// `limit = 1`.
    async fn highest_skiptoken(
        &self,
        source_category: &str,
    ) -> Result<Option<i64>, SearchIndexError>;

    /// Count indexed documents for a category, optionally bounded to the
    /// inclusive prefix `latest_skiptoken <= boundary`.
    async fn count_category_prefix(
        &self,
        source_category: &str,
        boundary: Option<i64>,
    ) -> Result<u64, SearchIndexError>;

    /// Delete indexed documents for `source_category` above an inclusive
    /// verified prefix boundary before idempotently rebuilding that suffix.
    async fn delete_category_after(
        &self,
        source_category: &str,
        boundary: i64,
    ) -> Result<(), SearchIndexError>;
}

pub trait SearchRuntimeClient: SearchQueryClient + SearchHealthClient + SearchCountClient {}

impl<T> SearchRuntimeClient for T where T: SearchQueryClient + SearchHealthClient + SearchCountClient
{}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    pub query: String,
    pub limit: u32,
    pub offset: u32,
    pub filter: Option<SearchFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilter {
    pub source_category: Option<String>,
    pub entity_kind: Option<SearchEntityKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResponse {
    pub query: String,
    pub limit: u32,
    pub offset: u32,
    pub estimated_total_hits: Option<u32>,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub key: String,
    pub source_category: String,
    pub source_id: Uuid,
    pub entity_kind: SearchEntityKind,
    pub title: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub date: Option<String>,
    pub document_number: Option<String>,
    pub snippets: Vec<SearchSnippet>,
    pub ranking_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSnippet {
    pub field: String,
    pub text: String,
    pub highlighted: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchSourceRecord {
    pub metadata: SearchEntityMetadata,
    pub fields: Map<String, Value>,
    pub document_content: Option<SearchDocumentContent>,
    pub relations: Vec<SearchRelationLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntityMetadata {
    pub category: String,
    pub source_id: Uuid,
    pub latest_skiptoken: i64,
    pub deleted: bool,
    pub source_updated_at: DateTime<Utc>,
    pub atom_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDocumentContent {
    pub selected_source_url: String,
    pub selected_source_content_type: Option<String>,
    pub official_source: bool,
    pub extraction_status: String,
    pub validation_status: String,
    pub output_hash: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRelationLabel {
    pub relation_name: String,
    pub target_category: String,
    pub target_id: Uuid,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchIndexOperation {
    Upsert(Box<SearchIndexDocument>),
    Delete(SearchDocumentId),
}

pub type SearchDocumentId = String;
pub type SearchDocumentKey = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndexDocument {
    pub id: SearchDocumentId,
    pub key: SearchDocumentKey,
    pub source_category: String,
    pub source_id: Uuid,
    pub entity_kind: SearchEntityKind,
    pub title: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub date: Option<String>,
    pub document_number: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub metadata_text: Vec<String>,
    pub relation_labels: Vec<String>,
    pub filter_categories: Vec<String>,
    pub latest_skiptoken: i64,
    pub source_updated_at: DateTime<Utc>,
    pub atom_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexSchema {
    pub index_name: &'static str,
    pub primary_key: &'static str,
    pub searchable_attributes: Vec<&'static str>,
    pub displayed_attributes: Vec<&'static str>,
    pub filterable_attributes: Vec<&'static str>,
    pub sortable_attributes: Vec<&'static str>,
    pub ranking_rules: Vec<&'static str>,
    pub max_total_hits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchEntityKind {
    Document,
    Person,
    Activity,
    Dossier,
    Other,
}

#[derive(Clone, Debug)]
pub struct MeilisearchClient {
    pub base_url: String,
    pub api_key: Option<String>,
    pub index_name: String,
    pub http: reqwest::Client,
}

impl MeilisearchClient {
    #[must_use]
    pub fn new(base_url: String, api_key: Option<String>, index_name: String) -> Self {
        Self {
            base_url,
            api_key,
            index_name,
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchApiResultShape {
    pub fields: Vec<&'static str>,
    pub snippet_fields: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchStorageSample {
    pub record_count: u64,
    pub extracted_text_bytes: u64,
    pub extracted_html_bytes: u64,
    pub relation_label_bytes: u64,
    pub engine_overhead_factor: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStorageEstimate {
    pub estimated_index_bytes: u64,
    pub bytes_per_record_from_task_1_fixture: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SearchStorageError {
    #[error("storage sample record count must be greater than zero")]
    EmptySample,
    #[error("storage overhead factor must be greater than zero")]
    InvalidOverheadFactor,
}

#[must_use]
pub fn meilisearch_schema() -> SearchIndexSchema {
    SearchIndexSchema {
        index_name: "opentk_entities",
        primary_key: "id",
        searchable_attributes: vec![
            "title",
            "summary",
            "document_number",
            "metadata_text",
            "relation_labels",
            "extracted_text",
            "extracted_html",
        ],
        displayed_attributes: vec![
            "key",
            "source_category",
            "source_id",
            "entity_kind",
            "title",
            "summary",
            "source_url",
            "date",
            "document_number",
            "metadata_text",
            "latest_skiptoken",
            "source_updated_at",
            "_formatted",
        ],
        filterable_attributes: vec![
            "source_category",
            "entity_kind",
            "date",
            "filter_categories",
            "latest_skiptoken",
        ],
        sortable_attributes: vec!["date", "latest_skiptoken", "source_updated_at"],
        ranking_rules: vec![
            "words",
            "typo",
            "proximity",
            "attribute",
            "sort",
            "exactness",
        ],
        max_total_hits: 10_000_000,
    }
}

#[must_use]
pub fn api_result_shape() -> SearchApiResultShape {
    SearchApiResultShape {
        fields: vec![
            "key",
            "source_category",
            "source_id",
            "entity_kind",
            "title",
            "summary",
            "source_url",
            "date",
            "document_number",
            "snippets",
            "ranking_score",
        ],
        snippet_fields: vec![
            "title",
            "summary",
            "metadata_text",
            "extracted_text",
            "extracted_html",
        ],
    }
}

/// Estimates search index storage from measured source content bytes.
///
/// # Errors
///
/// Returns an error when the sample has no records or uses an invalid overhead factor.
pub fn storage_estimate(
    sample: SearchStorageSample,
) -> Result<SearchStorageEstimate, SearchStorageError> {
    if sample.record_count == 0 {
        return Err(SearchStorageError::EmptySample);
    }
    if sample.engine_overhead_factor == 0 {
        return Err(SearchStorageError::InvalidOverheadFactor);
    }

    let source_bytes =
        sample.extracted_text_bytes + sample.extracted_html_bytes + sample.relation_label_bytes;
    Ok(SearchStorageEstimate {
        estimated_index_bytes: source_bytes * sample.engine_overhead_factor,
        bytes_per_record_from_task_1_fixture: 716,
    })
}

impl SearchIndexClient for MeilisearchClient {
    async fn reset_index(&self, schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        let delete = self
            .request(
                reqwest::Method::DELETE,
                &format!("/indexes/{}", self.index_name),
            )
            .send()
            .await
            .map_err(request_error)?;
        if !delete.status().is_success() && delete.status() != StatusCode::NOT_FOUND {
            return Err(response_error(delete).await);
        }

        let create_body = serde_json::json!({
            "uid": self.index_name,
            "primaryKey": schema.primary_key,
        });
        let create = self
            .request(reqwest::Method::POST, "/indexes")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(create_body.to_string())
            .send()
            .await
            .map_err(request_error)?;
        self.wait_for_response_task(create).await?;

        self.ensure_settings("searchable-attributes", &schema.searchable_attributes)
            .await?;
        self.ensure_settings("displayed-attributes", &schema.displayed_attributes)
            .await?;
        self.ensure_settings("filterable-attributes", &schema.filterable_attributes)
            .await?;
        self.ensure_settings("sortable-attributes", &schema.sortable_attributes)
            .await?;
        self.ensure_settings("ranking-rules", &schema.ranking_rules)
            .await?;
        self.ensure_settings(
            "pagination",
            &serde_json::json!({ "maxTotalHits": schema.max_total_hits }),
        )
        .await?;
        Ok(())
    }

    async fn apply_batch(
        &self,
        operations: &[SearchIndexOperation],
    ) -> Result<(), SearchIndexError> {
        self.apply_batch_with_payload_limit(operations, usize::MAX)
            .await
    }

    async fn apply_batch_with_payload_limit(
        &self,
        operations: &[SearchIndexOperation],
        max_payload_bytes: usize,
    ) -> Result<(), SearchIndexError> {
        let upserts = operations
            .iter()
            .filter_map(|operation| match operation {
                SearchIndexOperation::Upsert(document) => Some(document.as_ref()),
                SearchIndexOperation::Delete(_) => None,
            })
            .collect::<Vec<_>>();
        for chunk in json_array_chunks(&upserts, max_payload_bytes)? {
            tracing::info!(
                payload_bytes = chunk.body.len(),
                upsert_count = chunk.item_count,
                delete_count = 0_usize,
                "Meilisearch payload POST started"
            );
            let upload_started = std::time::Instant::now();
            let response = self
                .request(
                    reqwest::Method::POST,
                    &format!("/indexes/{}/documents", self.index_name),
                )
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(chunk.body)
                .send()
                .await
                .map_err(request_error)?;
            let upload_ms = upload_started.elapsed().as_millis();
            let wait_started = std::time::Instant::now();
            self.wait_for_response_task(response).await?;
            tracing::info!(
                upload_ms,
                meilisearch_wait_ms = wait_started.elapsed().as_millis(),
                "Meilisearch payload POST finished"
            );
        }

        let delete_keys = operations
            .iter()
            .filter_map(|operation| match operation {
                SearchIndexOperation::Delete(key) => Some(key.as_str()),
                SearchIndexOperation::Upsert(_) => None,
            })
            .collect::<Vec<_>>();
        for chunk in json_array_chunks(&delete_keys, max_payload_bytes)? {
            tracing::info!(
                payload_bytes = chunk.body.len(),
                upsert_count = 0_usize,
                delete_count = chunk.item_count,
                "Meilisearch delete payload POST started"
            );
            let upload_started = std::time::Instant::now();
            let response = self
                .request(
                    reqwest::Method::POST,
                    &format!("/indexes/{}/documents/delete-batch", self.index_name),
                )
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(chunk.body)
                .send()
                .await
                .map_err(request_error)?;
            let upload_ms = upload_started.elapsed().as_millis();
            let wait_started = std::time::Instant::now();
            self.wait_for_response_task(response).await?;
            tracing::info!(
                upload_ms,
                meilisearch_wait_ms = wait_started.elapsed().as_millis(),
                "Meilisearch delete payload POST finished"
            );
        }
        Ok(())
    }
}

impl SearchReconcilerClient for MeilisearchClient {
    async fn ensure_index(&self, schema: &SearchIndexSchema) -> Result<(), SearchIndexError> {
        let get = self
            .request(
                reqwest::Method::GET,
                &format!("/indexes/{}", self.index_name),
            )
            .send()
            .await
            .map_err(request_error)?;
        if get.status() == StatusCode::NOT_FOUND {
            let create_body = serde_json::json!({
                "uid": self.index_name,
                "primaryKey": schema.primary_key,
            });
            let create = self
                .request(reqwest::Method::POST, "/indexes")
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(create_body.to_string())
                .send()
                .await
                .map_err(request_error)?;
            self.wait_for_response_task(create).await?;
        } else if !get.status().is_success() {
            return Err(response_error(get).await);
        }

        self.ensure_settings("searchable-attributes", &schema.searchable_attributes)
            .await?;
        self.ensure_settings("displayed-attributes", &schema.displayed_attributes)
            .await?;
        self.ensure_settings("filterable-attributes", &schema.filterable_attributes)
            .await?;
        self.ensure_settings("sortable-attributes", &schema.sortable_attributes)
            .await?;
        self.ensure_settings("ranking-rules", &schema.ranking_rules)
            .await?;
        self.ensure_settings(
            "pagination",
            &serde_json::json!({ "maxTotalHits": schema.max_total_hits }),
        )
        .await?;
        Ok(())
    }

    async fn highest_skiptoken(
        &self,
        source_category: &str,
    ) -> Result<Option<i64>, SearchIndexError> {
        let body = MeiliDocumentsFetchRequest {
            limit: Some(1),
            offset: Some(0),
            filter: Some(category_filter(source_category)?),
            sort: vec!["latest_skiptoken:desc"],
            fields: vec!["latest_skiptoken"],
        };
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/indexes/{}/documents/fetch", self.index_name),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body).map_err(json_error)?)
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        let body = response.text().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(SearchIndexError::Http {
                status: Some(status.as_u16()),
                message: body,
            });
        }
        let response: MeiliDocumentsFetchResponse =
            serde_json::from_str(&body).map_err(json_error)?;
        response
            .results
            .first()
            .map(|hit| {
                hit.get("latest_skiptoken")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| {
                        SearchIndexError::InvalidResponse(
                            "highest skiptoken hit missing latest_skiptoken".to_owned(),
                        )
                    })
            })
            .transpose()
    }

    async fn count_category_prefix(
        &self,
        source_category: &str,
        boundary: Option<i64>,
    ) -> Result<u64, SearchIndexError> {
        let filter = match boundary {
            Some(boundary) => format!(
                "{} AND latest_skiptoken <= {boundary}",
                category_filter(source_category)?
            ),
            None => category_filter(source_category)?,
        };
        let body = MeiliDocumentsFetchRequest {
            limit: Some(1),
            offset: Some(0),
            filter: Some(filter),
            sort: Vec::new(),
            fields: vec!["id"],
        };
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/indexes/{}/documents/fetch", self.index_name),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body).map_err(json_error)?)
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        let body = response.text().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(SearchIndexError::Http {
                status: Some(status.as_u16()),
                message: body,
            });
        }
        let response: MeiliDocumentsFetchResponse =
            serde_json::from_str(&body).map_err(json_error)?;
        Ok(response.total)
    }

    async fn delete_category_after(
        &self,
        source_category: &str,
        boundary: i64,
    ) -> Result<(), SearchIndexError> {
        let body = serde_json::json!({
            "filter": format!(
                "{} AND latest_skiptoken > {boundary}",
                category_filter(source_category)?
            ),
        });
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/indexes/{}/documents/delete", self.index_name),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(request_error)?;
        self.wait_for_response_task(response).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JsonArrayChunk {
    body: String,
    item_count: usize,
}

fn json_array_chunks<T: Serialize>(
    items: &[T],
    max_payload_bytes: usize,
) -> Result<Vec<JsonArrayChunk>, SearchIndexError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut body = String::from("[");
    let mut item_count = 0_usize;
    for item in items {
        let item_json = serde_json::to_string(item).map_err(json_error)?;
        let separator_bytes = usize::from(item_count > 0);
        let projected_bytes = body.len() + separator_bytes + item_json.len() + 1;
        if item_json.len() + 2 > max_payload_bytes {
            return Err(SearchIndexError::PayloadTooLarge {
                item_bytes: item_json.len() + 2,
                max_payload_bytes,
            });
        }
        if item_count > 0 && projected_bytes > max_payload_bytes {
            body.push(']');
            chunks.push(JsonArrayChunk { body, item_count });
            body = String::from("[");
            item_count = 0;
        }
        if item_count > 0 {
            body.push(',');
        }
        body.push_str(&item_json);
        item_count += 1;
    }
    body.push(']');
    chunks.push(JsonArrayChunk { body, item_count });
    Ok(chunks)
}

impl SearchQueryClient for MeilisearchClient {
    fn search<'a>(
        &'a self,
        request: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchResponse, SearchIndexError>> + Send + 'a>> {
        Box::pin(async move { self.search_index(request).await })
    }
}

impl SearchHealthClient for MeilisearchClient {
    fn health<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<(), SearchIndexError>> + Send + 'a>> {
        Box::pin(async move { self.validate_reachable().await })
    }
}

impl SearchCountClient for MeilisearchClient {
    fn count<'a>(
        &'a self,
        filter: SearchFilter,
    ) -> Pin<Box<dyn Future<Output = Result<u64, SearchIndexError>> + Send + 'a>> {
        Box::pin(async move { self.count_index(filter).await })
    }
}

impl MeilisearchClient {
    async fn search_index(
        &self,
        request: SearchRequest,
    ) -> Result<SearchResponse, SearchIndexError> {
        let body = MeiliSearchRequest::from_search_request(&request)?;
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/indexes/{}/search", self.index_name),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body).map_err(json_error)?)
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        let body = response.text().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(SearchIndexError::Http {
                status: Some(status.as_u16()),
                message: body,
            });
        }
        let response: MeiliSearchResponse = serde_json::from_str(&body).map_err(json_error)?;
        Ok(response.into_search_response(request))
    }

    async fn count_index(&self, filter: SearchFilter) -> Result<u64, SearchIndexError> {
        let body = MeiliSearchRequest::from_search_request(&SearchRequest {
            query: String::new(),
            limit: 0,
            offset: 0,
            filter: Some(filter),
        })?;
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/indexes/{}/search", self.index_name),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body).map_err(json_error)?)
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        let body = response.text().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(SearchIndexError::Http {
                status: Some(status.as_u16()),
                message: body,
            });
        }
        let response: MeiliSearchResponse = serde_json::from_str(&body).map_err(json_error)?;
        response.estimated_total_hits.map(u64::from).ok_or_else(|| {
            SearchIndexError::InvalidResponse("search count missing estimatedTotalHits".to_owned())
        })
    }

    /// Check that Meilisearch accepts authenticated lightweight requests.
    ///
    /// # Errors
    ///
    /// Returns [`SearchIndexError`] when the stats request fails, returns a
    /// non-success status, or cannot be read.
    pub async fn validate_reachable(&self) -> Result<(), SearchIndexError> {
        let response = self
            .request(reqwest::Method::GET, "/stats")
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        let body = response.text().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(SearchIndexError::Http {
                status: Some(status.as_u16()),
                message: body,
            });
        }
        Ok(())
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let mut request = self.http.request(method, url);
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        request
    }

    async fn ensure_settings<T: Serialize + ?Sized>(
        &self,
        setting: &str,
        value: &T,
    ) -> Result<(), SearchIndexError> {
        let desired = serde_json::to_value(value).map_err(json_error)?;
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/indexes/{}/settings/{setting}", self.index_name),
            )
            .send()
            .await
            .map_err(request_error)?;
        let status = response.status();
        let body = response.text().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(SearchIndexError::Http {
                status: Some(status.as_u16()),
                message: body,
            });
        }
        let current: Value = serde_json::from_str(&body).map_err(json_error)?;
        if current == desired {
            tracing::debug!(setting, "Meilisearch setting already matches desired value");
            return Ok(());
        }
        self.apply_settings(setting, &desired).await
    }

    async fn apply_settings<T: Serialize + ?Sized>(
        &self,
        setting: &str,
        value: &T,
    ) -> Result<(), SearchIndexError> {
        let method = if setting == "pagination" {
            reqwest::Method::PATCH
        } else {
            reqwest::Method::PUT
        };
        let response = self
            .request(
                method,
                &format!("/indexes/{}/settings/{setting}", self.index_name),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(value).map_err(json_error)?)
            .send()
            .await
            .map_err(request_error)?;
        self.wait_for_response_task(response).await
    }

    async fn wait_for_response_task(
        &self,
        response: reqwest::Response,
    ) -> Result<(), SearchIndexError> {
        let status = response.status();
        let body = response.text().await.map_err(request_error)?;
        if !status.is_success() {
            return Err(SearchIndexError::Http {
                status: Some(status.as_u16()),
                message: body,
            });
        }
        let task: MeiliTaskCreate = serde_json::from_str(&body).map_err(json_error)?;
        self.wait_for_task(task.task_uid).await
    }

    async fn wait_for_task(&self, task_uid: u64) -> Result<(), SearchIndexError> {
        for _ in 0..7200 {
            let response = self
                .request(reqwest::Method::GET, &format!("/tasks/{task_uid}"))
                .send()
                .await
                .map_err(request_error)?;
            let status = response.status();
            let body = response.text().await.map_err(request_error)?;
            if !status.is_success() {
                return Err(SearchIndexError::Http {
                    status: Some(status.as_u16()),
                    message: body,
                });
            }
            let task: MeiliTaskStatus = serde_json::from_str(&body).map_err(json_error)?;
            match task.status.as_str() {
                "succeeded" => return Ok(()),
                "failed" | "canceled" => {
                    return Err(SearchIndexError::InvalidResponse(
                        task.error
                            .and_then(|error| error.message)
                            .unwrap_or_else(|| format!("Meilisearch task {task_uid} failed")),
                    ));
                }
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
        Err(SearchIndexError::InvalidResponse(format!(
            "Meilisearch task {task_uid} did not finish before timeout"
        )))
    }
}

#[derive(Debug, Deserialize)]
struct MeiliTaskCreate {
    #[serde(rename = "taskUid")]
    task_uid: u64,
}

#[derive(Debug, Deserialize)]
struct MeiliTaskStatus {
    status: String,
    error: Option<MeiliTaskError>,
}

#[derive(Debug, Deserialize)]
struct MeiliTaskError {
    message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeiliSearchRequest {
    q: String,
    limit: u32,
    offset: u32,
    attributes_to_highlight: Vec<&'static str>,
    attributes_to_crop: Vec<&'static str>,
    show_ranking_score: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeiliDocumentsFetchRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sort: Vec<&'a str>,
    fields: Vec<&'a str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeiliDocumentsFetchResponse {
    results: Vec<Map<String, Value>>,
    total: u64,
}

impl MeiliSearchRequest {
    fn from_search_request(request: &SearchRequest) -> Result<Self, SearchIndexError> {
        Ok(Self {
            q: request.query.clone(),
            limit: request.limit,
            offset: request.offset,
            attributes_to_highlight: api_result_shape().snippet_fields,
            attributes_to_crop: vec![
                "summary",
                "metadata_text",
                "extracted_text",
                "extracted_html",
            ],
            show_ranking_score: true,
            filter: request.filter.as_ref().map(meili_filter).transpose()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeiliSearchResponse {
    hits: Vec<MeiliSearchHit>,
    estimated_total_hits: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct MeiliSearchHit {
    key: String,
    source_category: String,
    source_id: Uuid,
    entity_kind: SearchEntityKind,
    title: String,
    summary: Option<String>,
    source_url: Option<String>,
    date: Option<String>,
    document_number: Option<String>,
    #[serde(rename = "_formatted")]
    formatted: Option<Map<String, Value>>,
    #[serde(rename = "_rankingScore")]
    ranking_score: Option<f64>,
}

impl MeiliSearchResponse {
    fn into_search_response(self, request: SearchRequest) -> SearchResponse {
        SearchResponse {
            query: request.query,
            limit: request.limit,
            offset: request.offset,
            estimated_total_hits: self.estimated_total_hits,
            results: self.hits.into_iter().map(search_result_from_hit).collect(),
        }
    }
}

fn search_result_from_hit(hit: MeiliSearchHit) -> SearchResult {
    let snippets = hit
        .formatted
        .as_ref()
        .map(snippets_from_formatted)
        .unwrap_or_default();
    SearchResult {
        key: hit.key,
        source_category: hit.source_category,
        source_id: hit.source_id,
        entity_kind: hit.entity_kind,
        title: hit.title,
        summary: hit.summary,
        source_url: hit.source_url,
        date: hit.date,
        document_number: hit.document_number,
        snippets,
        ranking_score: hit.ranking_score,
    }
}

fn snippets_from_formatted(formatted: &Map<String, Value>) -> Vec<SearchSnippet> {
    let public_snippet_fields = api_result_shape()
        .snippet_fields
        .into_iter()
        .collect::<BTreeSet<_>>();
    formatted
        .iter()
        .filter(|(field, _)| public_snippet_fields.contains(field.as_str()))
        .filter_map(|(field, highlighted)| {
            let highlighted = formatted_text(highlighted)?;
            Some(SearchSnippet {
                field: field.clone(),
                text: strip_highlight_tags(&highlighted),
                highlighted: Some(highlighted),
            })
        })
        .collect()
}

fn formatted_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => present_string(Some(value)),
        Value::Array(values) => {
            let text = values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| present_string(Some(value)))
                .collect::<Vec<_>>()
                .join(" ");
            present_string(Some(&text))
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => None,
    }
}

fn strip_highlight_tags(value: &str) -> String {
    value.replace("<em>", "").replace("</em>", "")
}

fn meili_filter(filter: &SearchFilter) -> Result<String, SearchIndexError> {
    let mut clauses = Vec::new();
    if let Some(category) = &filter.source_category {
        clauses.push(format!(
            "source_category = {}",
            serde_json::to_string(category).map_err(json_error)?
        ));
    }
    if let Some(entity_kind) = &filter.entity_kind {
        clauses.push(format!(
            "entity_kind = {}",
            serde_json::to_string(entity_kind).map_err(json_error)?
        ));
    }
    Ok(clauses.join(" AND "))
}

fn category_filter(source_category: &str) -> Result<String, SearchIndexError> {
    Ok(format!(
        "source_category = {}",
        serde_json::to_string(source_category).map_err(json_error)?
    ))
}

fn request_error(error: reqwest::Error) -> SearchIndexError {
    let status = error.status().map(|status| status.as_u16());
    let message = error.to_string();
    drop(error);
    SearchIndexError::Http { status, message }
}

async fn response_error(response: reqwest::Response) -> SearchIndexError {
    let status = response.status();
    let message = response
        .text()
        .await
        .unwrap_or_else(|error| error.to_string());
    SearchIndexError::Http {
        status: Some(status.as_u16()),
        message,
    }
}

fn json_error(error: serde_json::Error) -> SearchIndexError {
    let message = error.to_string();
    drop(error);
    SearchIndexError::InvalidResponse(message)
}

/// Maps one source record from `PostgreSQL` into the operation to submit to the search index.
///
/// # Errors
///
/// Returns an error when a required identity/title/body field is absent or when
/// scalar metadata cannot be converted into stable searchable text.
pub fn map_record_to_operation(
    record: &SearchSourceRecord,
) -> Result<SearchIndexOperation, SearchMappingError> {
    let id = document_id(&record.metadata.category, record.metadata.source_id);
    let key = document_key(&record.metadata.category, record.metadata.source_id);
    if record.metadata.deleted {
        return Ok(SearchIndexOperation::Delete(id));
    }

    let entity_kind = entity_kind(&record.metadata.category);
    let title = title_for_record(record)?;
    let document_content = record.document_content.as_ref();
    let source_url = source_url(record, document_content);
    let document_number = optional_string(&record.fields, "document_nummer");
    let extracted_text =
        document_content.and_then(|content| present_string(content.extracted_text.as_deref()));
    let extracted_html =
        document_content.and_then(|content| present_string(content.extracted_html.as_deref()));

    if entity_kind == SearchEntityKind::Document {
        require_option(document_number.as_ref(), "document_nummer")?;
        require_field(&record.fields, "content_type")?;
        require_field(&record.fields, "content_length")?;
    }

    let metadata_text = metadata_text(&record.fields, document_content)?;
    let relation_labels = record
        .relations
        .iter()
        .map(|relation| {
            format!(
                "{} {} {}",
                relation.relation_name, relation.target_category, relation.label
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let filter_categories = filter_categories(record);

    Ok(SearchIndexOperation::Upsert(Box::new(
        SearchIndexDocument {
            id,
            key,
            source_category: record.metadata.category.clone(),
            source_id: record.metadata.source_id,
            entity_kind,
            title,
            summary: optional_string(&record.fields, "samenvatting"),
            source_url,
            date: optional_string(&record.fields, "datum"),
            document_number,
            extracted_text,
            extracted_html,
            metadata_text,
            relation_labels,
            filter_categories,
            latest_skiptoken: record.metadata.latest_skiptoken,
            source_updated_at: record.metadata.source_updated_at,
            atom_updated_at: record.metadata.atom_updated_at,
        },
    )))
}

fn filter_categories(record: &SearchSourceRecord) -> Vec<String> {
    let target_categories = record
        .relations
        .iter()
        .filter_map(|relation| present_string(Some(relation.target_category.as_str())))
        .collect::<BTreeSet<_>>();
    let mut categories = vec![record.metadata.category.clone()];
    categories.extend(
        target_categories
            .into_iter()
            .filter(|category| category != &record.metadata.category),
    );
    categories
}

fn document_id(source_category: &str, source_id: Uuid) -> SearchDocumentId {
    format!("{source_category}_{source_id}")
}

fn document_key(source_category: &str, source_id: Uuid) -> SearchDocumentKey {
    format!("{source_category}:{source_id}")
}

fn entity_kind(source_category: &str) -> SearchEntityKind {
    match source_category {
        "Document" => SearchEntityKind::Document,
        "Persoon" => SearchEntityKind::Person,
        "Activiteit" => SearchEntityKind::Activity,
        "Kamerstukdossier" | "Zaak" => SearchEntityKind::Dossier,
        _ => SearchEntityKind::Other,
    }
}

fn title_for_record(record: &SearchSourceRecord) -> Result<String, SearchMappingError> {
    if record.metadata.category == "Persoon" {
        if let Some(value) = person_display_name(&record.fields) {
            return Ok(value);
        }
    }

    for field in ["titel", "onderwerp", "nummer", "document_nummer"] {
        if let Some(value) = optional_string(&record.fields, field) {
            return Ok(value);
        }
    }

    record
        .relations
        .iter()
        .find_map(|relation| present_string(Some(relation.label.as_str())))
        .or_else(|| {
            Some(format!(
                "{} {}",
                record.metadata.category, record.metadata.source_id
            ))
        })
        .ok_or(SearchMappingError::MissingRequiredField { field: "title" })
}

fn person_display_name(fields: &Map<String, Value>) -> Option<String> {
    let initials = optional_string(fields, "initialen");
    let call_sign = optional_string(fields, "roepnaam");
    let prefix = optional_string(fields, "tussenvoegsel");
    let surname = optional_string(fields, "achternaam");

    let first = call_sign.or(initials);
    let display = [first, prefix, surname]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if display.is_empty() {
        None
    } else {
        Some(display)
    }
}

fn source_url(
    record: &SearchSourceRecord,
    document_content: Option<&SearchDocumentContent>,
) -> Option<String> {
    optional_string(&record.fields, "enclosure_url")
        .or_else(|| document_content.map(|content| content.selected_source_url.clone()))
        .and_then(|value| present_string(Some(value.as_str())))
}

fn optional_string(fields: &Map<String, Value>, field: &'static str) -> Option<String> {
    fields
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| present_string(Some(value)))
}

fn present_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn require_option<T>(value: Option<&T>, field: &'static str) -> Result<(), SearchMappingError> {
    if value.is_some() {
        Ok(())
    } else {
        Err(SearchMappingError::MissingRequiredField { field })
    }
}

fn require_field(
    fields: &Map<String, Value>,
    field: &'static str,
) -> Result<(), SearchMappingError> {
    match fields.get(field) {
        Some(Value::Null) | None => Err(SearchMappingError::MissingRequiredField { field }),
        Some(Value::String(value)) if value.trim().is_empty() => {
            Err(SearchMappingError::MissingRequiredField { field })
        }
        Some(_) => Ok(()),
    }
}

fn metadata_text(
    fields: &Map<String, Value>,
    document_content: Option<&SearchDocumentContent>,
) -> Result<Vec<String>, SearchMappingError> {
    let mut values = Vec::new();
    for (field, value) in fields {
        if matches!(
            field.as_str(),
            "titel" | "onderwerp" | "samenvatting" | "datum" | "document_nummer" | "enclosure_url"
        ) {
            continue;
        }
        append_metadata_value(field, value, &mut values)?;
    }

    if let Some(content) = document_content {
        values.push(format!("official_source: {}", content.official_source));
        values.push(format!("extraction_status: {}", content.extraction_status));
        values.push(format!("validation_status: {}", content.validation_status));
        if let Some(content_type) = present_string(content.selected_source_content_type.as_deref())
        {
            values.push(format!("selected_source_content_type: {content_type}"));
        }
        if let Some(output_hash) = present_string(content.output_hash.as_deref()) {
            values.push(format!("output_hash: {output_hash}"));
        }
    }

    Ok(values)
}

fn append_metadata_value(
    field: &str,
    value: &Value,
    values: &mut Vec<String>,
) -> Result<(), SearchMappingError> {
    match value {
        Value::Null => Ok(()),
        Value::Bool(value) => {
            values.push(format!("{field}: {value}"));
            Ok(())
        }
        Value::Number(value) => {
            values.push(format!("{field}: {value}"));
            Ok(())
        }
        Value::String(value) => {
            if !value.trim().is_empty() {
                values.push(format!("{field}: {}", value.trim()));
            }
            Ok(())
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                append_metadata_value(&format!("{field}[{index}]"), item, values)?;
            }
            Ok(())
        }
        Value::Object(object) => {
            for (name, item) in object {
                append_metadata_value(&format!("{field}.{name}"), item, values)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_array_chunks_flush_before_payload_limit() {
        let items = vec!["alpha", "beta", "gamma"];
        let chunks = json_array_chunks(&items, 16).expect("chunks fit");

        assert_eq!(
            chunks,
            vec![
                JsonArrayChunk {
                    body: "[\"alpha\",\"beta\"]".to_owned(),
                    item_count: 2,
                },
                JsonArrayChunk {
                    body: "[\"gamma\"]".to_owned(),
                    item_count: 1,
                },
            ]
        );
        assert!(chunks.iter().all(|chunk| chunk.body.len() <= 16));
    }

    #[test]
    fn json_array_chunks_rejects_single_item_over_limit() {
        let error = json_array_chunks(&["too-large"], 4).expect_err("single item is too large");

        assert!(matches!(
            error,
            SearchIndexError::PayloadTooLarge {
                item_bytes: 13,
                max_payload_bytes: 4
            }
        ));
    }
}
