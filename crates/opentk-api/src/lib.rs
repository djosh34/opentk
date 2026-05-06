//! HTTP API boundary for `OpenTK`.

mod search;

use axum::{
    body::Body,
    extract::MatchedPath,
    extract::{rejection::QueryRejection, Path, Query, State},
    http::{header::CONTENT_TYPE, Method, Request, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::DateTime;
use opentk_db::{
    connect,
    read_model::{
        self, DocumentContentDetail, EntityChange, EntityDetail, ReadModelError, RelationDirection,
        RelationRow,
    },
    startup_validation::{
        validate_database_config, validate_meilisearch_config, DependencyValidationError,
    },
    DatabaseConfig, DatabaseError,
};
use opentk_search::{
    MeilisearchClient, SearchIndexError, SearchQueryClient, SearchRequest, SearchResponse,
    SearchRuntimeClient,
};
use prometheus::{CounterVec, Encoder, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::PgPool;
use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::net::TcpListener;
use utoipa::{
    openapi::Required,
    openapi::{
        path::{
            HttpMethod, OperationBuilder, Parameter, ParameterBuilder, ParameterIn, PathItem,
            Paths, PathsBuilder,
        },
        response::ResponseBuilder,
        schema::{Components, ComponentsBuilder},
        Content, Info, Ref,
    },
    ToSchema,
};

const CHANGES_API_MAX_LIMIT: u32 = 500;
const DEFAULT_PUBLIC_QUERY_MAX_LIMIT: u32 = 1000;

#[derive(Clone)]
struct ApiState {
    pool: PgPool,
    search: Arc<dyn SearchRuntimeClient + Send + Sync>,
    max_public_query_limit: u32,
    metrics: Arc<ApiMetrics>,
}

struct ApiMetrics {
    registry: Registry,
    http_requests: CounterVec,
    http_request_duration: HistogramVec,
    sync_category_lag: GaugeVec,
    sync_category_last_successful_sync: GaugeVec,
    sync_category_caught_up: GaugeVec,
}

impl ApiMetrics {
    fn new() -> Self {
        let registry = Registry::new();
        let http_requests = CounterVec::new(
            Opts::new(
                "opentk_http_requests_total",
                "Total OpenTK API HTTP requests by method, matched route, and status class.",
            ),
            &["method", "route", "status_class"],
        )
        .expect("static HTTP request counter definition is valid");
        let http_request_duration = HistogramVec::new(
            HistogramOpts::new(
                "opentk_http_request_duration_seconds",
                "OpenTK API HTTP request duration by method, matched route, and status class.",
            )
            .buckets(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["method", "route", "status_class"],
        )
        .expect("static HTTP request duration histogram definition is valid");
        let sync_category_lag = GaugeVec::new(
            Opts::new(
                "opentk_sync_category_lag_seconds",
                "Seconds since the last successful OpenTK source category sync.",
            ),
            &["category"],
        )
        .expect("static sync category lag gauge definition is valid");
        let sync_category_last_successful_sync = GaugeVec::new(
            Opts::new(
                "opentk_sync_category_last_successful_sync_timestamp_seconds",
                "Unix timestamp of the last successful OpenTK source category sync.",
            ),
            &["category"],
        )
        .expect("static sync category last successful sync gauge definition is valid");
        let sync_category_caught_up = GaugeVec::new(
            Opts::new(
                "opentk_sync_category_caught_up_timestamp_seconds",
                "Unix timestamp when the OpenTK source category last reached caught-up state.",
            ),
            &["category"],
        )
        .expect("static sync category caught-up gauge definition is valid");

        registry
            .register(Box::new(http_requests.clone()))
            .expect("HTTP request counter registration is unique");
        registry
            .register(Box::new(http_request_duration.clone()))
            .expect("HTTP request duration histogram registration is unique");
        registry
            .register(Box::new(sync_category_lag.clone()))
            .expect("sync category lag gauge registration is unique");
        registry
            .register(Box::new(sync_category_last_successful_sync.clone()))
            .expect("sync category last successful sync gauge registration is unique");
        registry
            .register(Box::new(sync_category_caught_up.clone()))
            .expect("sync category caught-up gauge registration is unique");

        Self {
            registry,
            http_requests,
            http_request_duration,
            sync_category_lag,
            sync_category_last_successful_sync,
            sync_category_caught_up,
        }
    }

    async fn refresh_sync_categories(&self, pool: &PgPool) -> Result<(), ReadModelError> {
        let now = now_timestamp_seconds();
        for category in read_model::list_category_progress(pool).await? {
            let labels = [category.category.as_str()];
            let last_successful_sync =
                timestamp_seconds(category.last_synced_at.as_deref()).unwrap_or(f64::NAN);
            let caught_up_at =
                timestamp_seconds(category.caught_up_at.as_deref()).unwrap_or(f64::NAN);
            let lag_seconds = if last_successful_sync.is_finite() {
                (now - last_successful_sync).max(0.0)
            } else {
                f64::NAN
            };

            self.sync_category_lag
                .with_label_values(&labels)
                .set(lag_seconds);
            self.sync_category_last_successful_sync
                .with_label_values(&labels)
                .set(last_successful_sync);
            self.sync_category_caught_up
                .with_label_values(&labels)
                .set(caught_up_at);
        }
        Ok(())
    }

    fn observe_http_request(
        &self,
        method: &Method,
        route: &str,
        status: StatusCode,
        duration_seconds: f64,
    ) {
        let labels = [method.as_str(), route, status_class(status)];
        self.http_requests.with_label_values(&labels).inc();
        self.http_request_duration
            .with_label_values(&labels)
            .observe(duration_seconds);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiConfig {
    pub bind_address: SocketAddr,
    pub database: DatabaseConfig,
    pub search: SearchBackendConfig,
    pub max_public_query_limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchBackendConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub index_name: String,
}

pub struct ApiServer {
    pub bind_address: SocketAddr,
    pub router: Router,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("database initialization failed")]
    Database(#[from] DatabaseError),
    #[error("startup dependency validation failed")]
    StartupValidation(#[from] DependencyValidationError),
    #[error("database unavailable")]
    DatabaseUnavailable(#[source] sqlx::Error),
    #[error("read model request failed")]
    ReadModel(#[from] ReadModelError),
    #[error("search request failed")]
    Search(#[from] SearchIndexError),
    #[error("invalid request")]
    InvalidRequest,
    #[error("failed to bind API listener at {address}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("API server failed")]
    Serve(#[source] std::io::Error),
    #[error("API background task failed")]
    BackgroundTask(#[source] tokio::task::JoinError),
    #[error("metrics encoding failed")]
    MetricsEncode(#[source] prometheus::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::DatabaseUnavailable(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorResponse {
                    code: "database_unavailable",
                    message: "database unavailable",
                },
            ),
            Self::ReadModel(ReadModelError::UnknownCategory(_) | ReadModelError::NotFound) => (
                StatusCode::NOT_FOUND,
                ErrorResponse {
                    code: "not_found",
                    message: "resource not found",
                },
            ),
            Self::ReadModel(ReadModelError::DocumentContentNotFound) => (
                StatusCode::NOT_FOUND,
                ErrorResponse {
                    code: "document_content_not_found",
                    message: "document content not found",
                },
            ),
            Self::Search(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorResponse {
                    code: "search_unavailable",
                    message: "search unavailable",
                },
            ),
            Self::InvalidRequest | Self::ReadModel(ReadModelError::InvalidLimit) => (
                StatusCode::BAD_REQUEST,
                ErrorResponse {
                    code: "invalid_request",
                    message: "invalid request",
                },
            ),
            Self::Database(_)
            | Self::StartupValidation(_)
            | Self::ReadModel(ReadModelError::Sql(_))
            | Self::Bind { .. }
            | Self::Serve(_)
            | Self::BackgroundTask(_)
            | Self::MetricsEncode(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse {
                    code: "internal_error",
                    message: "internal server error",
                },
            ),
        };

        (status, Json(body)).into_response()
    }
}

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    code: &'static str,
    message: &'static str,
}

#[derive(Serialize, ToSchema)]
struct HealthResponse {
    status: &'static str,
    postgres: &'static str,
    meilisearch: &'static str,
    search_index: &'static str,
    search_sync: &'static str,
}

#[derive(Serialize, ToSchema)]
struct CategoryMetadataResponse {
    categories: Vec<CategoryMetadata>,
}

#[derive(Serialize, ToSchema)]
struct CategoryMetadata {
    category: String,
    table: String,
    field_count: usize,
    relation_count: usize,
}

#[derive(Serialize, ToSchema)]
struct SyncStatusResponse {
    categories: Vec<CategorySyncStatus>,
}

#[derive(Serialize, ToSchema)]
struct CategorySyncStatus {
    category: String,
    latest_skiptoken: Option<i64>,
    state: Option<String>,
    last_fetch_at: Option<String>,
    last_synced_at: Option<String>,
    caught_up_at: Option<String>,
    next_url: Option<String>,
    resume_url: Option<String>,
}

#[derive(Deserialize)]
struct ChangesQuery {
    after: Option<i64>,
    limit: Option<u32>,
}

#[derive(Serialize, ToSchema)]
struct ChangePageResponse {
    category: String,
    items: Vec<EntityChangeResponse>,
    next_skiptoken: Option<i64>,
    has_more: bool,
}

#[derive(Serialize, ToSchema)]
struct EntityChangeResponse {
    category: String,
    source_id: String,
    latest_skiptoken: i64,
    deleted: bool,
    source_updated_at: String,
    atom_updated_at: String,
}

#[derive(Deserialize)]
struct RelationsQuery {
    #[serde(default)]
    relations: RelationExpansion,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum RelationExpansion {
    #[default]
    None,
    Outgoing,
    Incoming,
    Both,
}

#[derive(Deserialize)]
struct RelationLookupQuery {
    #[serde(default)]
    direction: RelationLookupDirection,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum RelationLookupDirection {
    #[default]
    Outgoing,
    Incoming,
    Both,
}

#[derive(Deserialize)]
struct EntityListQuery {
    limit: Option<u32>,
    sort: Option<String>,
    #[serde(default)]
    relations: RelationExpansion,
}

#[derive(Serialize, ToSchema)]
struct EntityListResponse {
    category: String,
    items: Vec<EntityDetailResponse>,
    has_more: bool,
    effective_limit: u32,
}

#[derive(Serialize, ToSchema)]
struct EntityDetailResponse {
    metadata: EntityChangeResponse,
    fields: Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relations: Option<Vec<RelationResponse>>,
}

#[derive(Serialize, ToSchema)]
struct DocumentContentResponse {
    document: DocumentIdentityResponse,
    asset: DocumentAssetResponse,
    content: DocumentContentBodyResponse,
}

#[derive(Serialize, ToSchema)]
struct DocumentIdentityResponse {
    source_category: String,
    source_id: String,
}

#[derive(Serialize, ToSchema)]
struct DocumentAssetResponse {
    id: i64,
    asset_url: String,
    upstream_url: String,
    upstream_content_type: Option<String>,
    upstream_content_length: Option<i64>,
    upstream_last_modified_at: Option<String>,
    retrieval_status: String,
    retrieval_error: Option<String>,
    retrieved_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct DocumentContentBodyResponse {
    id: i64,
    selected_source_url: String,
    selected_source_content_type: Option<String>,
    selected_source_content_length: Option<i64>,
    official_source: bool,
    source_rank: i32,
    extraction_status: String,
    validation_status: String,
    extraction_tool: String,
    extraction_tool_version: String,
    source_hash: String,
    output_hash: Option<String>,
    extraction_error: Option<String>,
    extracted_text: Option<String>,
    extracted_html: Option<String>,
    extracted_at: String,
}

#[derive(Serialize, ToSchema)]
struct RelationLookupResponse {
    items: Vec<RelationResponse>,
}

#[derive(Serialize, ToSchema)]
struct RelationResponse {
    source_category: String,
    source_id: String,
    relation_name: String,
    target_category: String,
    target_id: String,
    ordinal: i32,
    source_updated_at: String,
}

/// Builds an API router and stores the bind address that should be used by the listener.
///
/// # Errors
///
/// Returns [`ApiError::Database`] when the configured `PostgreSQL` pool cannot be
/// initialized.
pub async fn build_app(config: ApiConfig) -> Result<ApiServer, ApiError> {
    validate_database_config(&config.database).await?;
    let pool = connect(&config.database).await?;
    let search = configured_search_client(SearchBackendConfig {
        url: config.search.url,
        api_key: config.search.api_key,
        index_name: config.search.index_name,
    })
    .await;

    Ok(ApiServer {
        bind_address: config.bind_address,
        router: router_with_search_and_public_limit(pool, search, config.max_public_query_limit),
    })
}

async fn configured_search_client(
    config: SearchBackendConfig,
) -> Arc<dyn SearchRuntimeClient + Send + Sync> {
    let search = Arc::new(MeilisearchClient::new(
        config.url.clone(),
        config.api_key.clone(),
        config.index_name.clone(),
    ));
    if let Err(error) =
        validate_meilisearch_config(config.url, config.api_key, config.index_name).await
    {
        tracing::error!(%error, "search backend unavailable at startup");
        return Arc::new(UnavailableSearchClient);
    }
    search
}

/// Starts the HTTP API server and serves requests until the listener exits.
///
/// # Errors
///
/// Returns [`ApiError::Database`] when database initialization fails,
/// [`ApiError::Bind`] when the configured socket cannot be bound, or
/// [`ApiError::Serve`] when `Axum` reports a serving failure.
pub async fn serve(config: ApiConfig) -> Result<(), ApiError> {
    serve_with_shutdown(config, shutdown_signal()).await
}

/// Starts the HTTP API server and stops it when the supplied shutdown future resolves.
///
/// # Errors
///
/// Returns [`ApiError`] when startup validation, database connection, TCP bind,
/// HTTP serving, or background task joining fails.
pub async fn serve_with_shutdown(
    config: ApiConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ApiError> {
    validate_database_config(&config.database).await?;
    let pool = connect(&config.database).await?;
    let concrete_search = Arc::new(MeilisearchClient::new(
        config.search.url.clone(),
        config.search.api_key.clone(),
        config.search.index_name.clone(),
    ));
    let search: Arc<dyn SearchRuntimeClient + Send + Sync> = match validate_meilisearch_config(
        config.search.url,
        config.search.api_key,
        config.search.index_name,
    )
    .await
    {
        Ok(_) => concrete_search.clone(),
        Err(error) => {
            tracing::error!(%error, "search backend unavailable at startup");
            Arc::new(UnavailableSearchClient)
        }
    };
    let server = ApiServer {
        bind_address: config.bind_address,
        router: router_with_search_and_public_limit(
            pool.clone(),
            search,
            config.max_public_query_limit,
        ),
    };
    let listener = TcpListener::bind(server.bind_address)
        .await
        .map_err(|source| ApiError::Bind {
            address: server.bind_address,
            source,
        })?;
    axum::serve(listener, server.router)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(ApiError::Serve)?;
    Ok(())
}

pub fn router_with_search(
    pool: PgPool,
    search_client: Arc<dyn SearchRuntimeClient + Send + Sync>,
) -> Router {
    router_with_search_and_public_limit(pool, search_client, DEFAULT_PUBLIC_QUERY_MAX_LIMIT)
}

pub fn router_with_search_and_public_limit(
    pool: PgPool,
    search_client: Arc<dyn SearchRuntimeClient + Send + Sync>,
    max_public_query_limit: u32,
) -> Router {
    let max_public_query_limit = max_public_query_limit.max(1);
    let metrics = Arc::new(ApiMetrics::new());
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics_handler))
        .route("/openapi.json", get(openapi_json))
        .route("/search", get(search::search))
        .route("/categories", get(categories))
        .route("/sync/status", get(sync_status))
        .route("/changes/{category}", get(changes))
        .route("/entities/{category}", get(entity_list))
        .route("/entities/{category}/{source_id}", get(entity_detail))
        .route("/documents", get(document_list))
        .route("/documents/{source_id}", get(document_detail))
        .route("/documents/{source_id}/content", get(document_content))
        .route("/activities", get(activity_list))
        .route("/activities/{source_id}", get(activity_detail))
        .route("/persons", get(person_list))
        .route("/persons/{source_id}", get(person_detail))
        .route("/relations/{category}/{source_id}", get(relations))
        .layer(from_fn_with_state(
            Arc::clone(&metrics),
            record_http_metrics,
        ))
        .with_state(ApiState {
            pool,
            search: search_client,
            max_public_query_limit,
            metrics,
        })
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
                message: "search backend unavailable at startup".to_owned(),
            })
        })
    }
}

impl opentk_search::SearchHealthClient for UnavailableSearchClient {
    fn health<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<(), SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search backend unavailable at startup".to_owned(),
            })
        })
    }
}

impl opentk_search::SearchCountClient for UnavailableSearchClient {
    fn count<'a>(
        &'a self,
        _filter: opentk_search::SearchFilter,
    ) -> Pin<Box<dyn Future<Output = Result<u64, SearchIndexError>> + Send + 'a>> {
        Box::pin(async {
            Err(SearchIndexError::Http {
                status: None,
                message: "search backend unavailable at startup".to_owned(),
            })
        })
    }
}

#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut document = utoipa::openapi::OpenApi::new(Info::new("OpenTK API", "0.1.0"), api_paths());
    document.components = Some(api_components());
    document
}

fn api_paths() -> Paths {
    read_paths(search_paths(base_paths(PathsBuilder::new()))).build()
}

fn base_paths(paths: PathsBuilder) -> PathsBuilder {
    paths
        .path(
            "/health",
            PathItem::new(
                HttpMethod::Get,
                get_operation(
                    "health",
                    "API and database are reachable",
                    HealthResponse::name().as_ref(),
                )
                .response(
                    "503",
                    json_response("Database is unreachable", ErrorResponse::name().as_ref()),
                ),
            ),
        )
        .path(
            "/openapi.json",
            PathItem::new(
                HttpMethod::Get,
                OperationBuilder::new()
                    .operation_id(Some("openapi_json"))
                    .response(
                        "200",
                        ResponseBuilder::new().description("OpenAPI document"),
                    ),
            ),
        )
}

fn search_paths(paths: PathsBuilder) -> PathsBuilder {
    paths.path(
        "/search",
        PathItem::new(
            HttpMethod::Get,
            get_operation(
                "search",
                "Search results",
                search::SearchResponseDto::name().as_ref(),
            )
            .parameters(Some([
                required_query_parameter("q", "Search text"),
                query_parameter("limit", "Page size from 1 through 100"),
                query_parameter("offset", "Result offset"),
                query_parameter("category", "Exact source category filter"),
                query_parameter("entity_kind", "Exact entity kind filter"),
            ]))
            .response(
                "400",
                json_response("Invalid search request", ErrorResponse::name().as_ref()),
            )
            .response(
                "503",
                json_response("Search backend unavailable", ErrorResponse::name().as_ref()),
            ),
        ),
    )
}

fn read_paths(paths: PathsBuilder) -> PathsBuilder {
    detail_paths(
        paths
            .path(
                "/categories",
                PathItem::new(
                    HttpMethod::Get,
                    get_operation(
                        "categories",
                        "Official category metadata",
                        CategoryMetadataResponse::name().as_ref(),
                    ),
                ),
            )
            .path(
                "/changes/{category}",
                PathItem::new(
                    HttpMethod::Get,
                    get_operation(
                        "changes",
                        "Category changes page",
                        ChangePageResponse::name().as_ref(),
                    )
                    .parameters(Some([
                        path_parameter("category", "Official entity category"),
                        query_parameter("after", "Exclusive skiptoken cursor"),
                        query_parameter("limit", "Page size from 1 through 500"),
                    ]))
                    .response(
                        "400",
                        json_response("Invalid cursor or limit", ErrorResponse::name().as_ref()),
                    )
                    .response(
                        "404",
                        json_response("Unknown category", ErrorResponse::name().as_ref()),
                    ),
                ),
            ),
    )
}

fn detail_paths(paths: PathsBuilder) -> PathsBuilder {
    relation_paths(typed_detail_paths(
        paths
            .path(
                "/entities/{category}",
                PathItem::new(HttpMethod::Get, generic_list_operation()),
            )
            .path(
                "/entities/{category}/{source_id}",
                PathItem::new(HttpMethod::Get, generic_detail_operation()),
            ),
    ))
}

fn typed_detail_paths(paths: PathsBuilder) -> PathsBuilder {
    paths
        .path(
            "/documents",
            PathItem::new(
                HttpMethod::Get,
                typed_list_operation("document_list", "Document list"),
            ),
        )
        .path(
            "/documents/{source_id}",
            PathItem::new(
                HttpMethod::Get,
                typed_detail_operation("document_detail", "Document detail", "Document"),
            ),
        )
        .path(
            "/documents/{source_id}/content",
            PathItem::new(HttpMethod::Get, document_content_operation()),
        )
        .path(
            "/activities",
            PathItem::new(
                HttpMethod::Get,
                typed_list_operation("activity_list", "Activity list"),
            ),
        )
        .path(
            "/activities/{source_id}",
            PathItem::new(
                HttpMethod::Get,
                typed_detail_operation("activity_detail", "Activity detail", "Activity"),
            ),
        )
        .path(
            "/persons",
            PathItem::new(
                HttpMethod::Get,
                typed_list_operation("person_list", "Person list"),
            ),
        )
        .path(
            "/persons/{source_id}",
            PathItem::new(
                HttpMethod::Get,
                typed_detail_operation("person_detail", "Person detail", "Person"),
            ),
        )
}

fn generic_list_operation() -> OperationBuilder {
    entity_list_operation("entity_list", "Entity list")
        .parameters(Some([
            path_parameter("category", "Official entity category"),
            query_parameter("limit", "Page size capped by api.max_public_query_limit"),
            query_parameter(
                "sort",
                "latest_asc/latest_desc/date_asc/date_desc/name_asc/name_desc",
            ),
            query_parameter("relations", "Relation expansion mode"),
        ]))
        .response(
            "404",
            json_response("Unknown category", ErrorResponse::name().as_ref()),
        )
}

fn typed_list_operation(operation_id: &'static str, description: &str) -> OperationBuilder {
    entity_list_operation(operation_id, description).parameters(Some([
        query_parameter("limit", "Page size capped by api.max_public_query_limit"),
        query_parameter(
            "sort",
            "latest_asc/latest_desc/date_asc/date_desc/name_asc/name_desc",
        ),
        query_parameter("relations", "Relation expansion mode"),
    ]))
}

fn entity_list_operation(operation_id: &'static str, description: &str) -> OperationBuilder {
    get_operation(
        operation_id,
        description,
        EntityListResponse::name().as_ref(),
    )
    .response(
        "400",
        json_response("Invalid list parameter", ErrorResponse::name().as_ref()),
    )
}

fn relation_paths(paths: PathsBuilder) -> PathsBuilder {
    paths.path(
        "/relations/{category}/{source_id}",
        PathItem::new(HttpMethod::Get, relation_lookup_operation()),
    )
}

fn generic_detail_operation() -> OperationBuilder {
    detail_operation("entity_detail", "Entity detail")
        .parameters(Some([
            path_parameter("category", "Official entity category"),
            path_parameter("source_id", "Entity source UUID"),
            query_parameter("relations", "Relation expansion mode"),
        ]))
        .response(
            "404",
            json_response("Missing entity", ErrorResponse::name().as_ref()),
        )
}

fn typed_detail_operation(
    operation_id: &'static str,
    description: &str,
    category: &str,
) -> OperationBuilder {
    detail_operation(operation_id, description)
        .parameters(Some([
            path_parameter("source_id", &format!("{category} source UUID")),
            query_parameter("relations", "Relation expansion mode"),
        ]))
        .response(
            "404",
            json_response(
                &format!("Missing {}", category.to_lowercase()),
                ErrorResponse::name().as_ref(),
            ),
        )
}

fn detail_operation(operation_id: &'static str, description: &str) -> OperationBuilder {
    get_operation(
        operation_id,
        description,
        EntityDetailResponse::name().as_ref(),
    )
    .response(
        "400",
        json_response("Invalid ID or parameter", ErrorResponse::name().as_ref()),
    )
}

fn document_content_operation() -> OperationBuilder {
    get_operation(
        "document_content",
        "Document content",
        DocumentContentResponse::name().as_ref(),
    )
    .parameters(Some([path_parameter("source_id", "Document source UUID")]))
    .response(
        "400",
        json_response("Invalid ID", ErrorResponse::name().as_ref()),
    )
    .response(
        "404",
        json_response(
            "Missing document or document content",
            ErrorResponse::name().as_ref(),
        ),
    )
}

fn relation_lookup_operation() -> OperationBuilder {
    get_operation(
        "relations",
        "Entity relations",
        RelationLookupResponse::name().as_ref(),
    )
    .parameters(Some([
        path_parameter("category", "Official entity category"),
        path_parameter("source_id", "Entity source UUID"),
        query_parameter("direction", "Relation lookup direction"),
    ]))
    .response(
        "400",
        json_response("Invalid ID or parameter", ErrorResponse::name().as_ref()),
    )
    .response(
        "404",
        json_response("Unknown category", ErrorResponse::name().as_ref()),
    )
}

fn get_operation(
    operation_id: &'static str,
    description: &str,
    schema_name: &str,
) -> OperationBuilder {
    OperationBuilder::new()
        .operation_id(Some(operation_id))
        .response("200", json_response(description, schema_name))
}

fn api_components() -> Components {
    ComponentsBuilder::new()
        .schema_from::<ErrorResponse>()
        .schema_from::<HealthResponse>()
        .schema_from::<CategoryMetadataResponse>()
        .schema_from::<CategoryMetadata>()
        .schema_from::<ChangePageResponse>()
        .schema_from::<EntityChangeResponse>()
        .schema_from::<EntityListResponse>()
        .schema_from::<EntityDetailResponse>()
        .schema_from::<DocumentContentResponse>()
        .schema_from::<DocumentIdentityResponse>()
        .schema_from::<DocumentAssetResponse>()
        .schema_from::<DocumentContentBodyResponse>()
        .schema_from::<RelationLookupResponse>()
        .schema_from::<RelationResponse>()
        .schema_from::<search::SearchResponseDto>()
        .schema_from::<search::SearchResultDto>()
        .schema_from::<search::SearchSnippetDto>()
        .schema_from::<search::SearchEntityKindDto>()
        .build()
}

fn json_response(description: &str, schema_name: &str) -> ResponseBuilder {
    ResponseBuilder::new().description(description).content(
        "application/json",
        Content::new(Some(Ref::from_schema_name(schema_name))),
    )
}

fn path_parameter(name: &str, description: &str) -> Parameter {
    ParameterBuilder::new()
        .name(name)
        .parameter_in(ParameterIn::Path)
        .required(Required::True)
        .description(Some(description))
        .build()
}

fn query_parameter(name: &str, description: &str) -> Parameter {
    ParameterBuilder::new()
        .name(name)
        .parameter_in(ParameterIn::Query)
        .required(Required::False)
        .description(Some(description))
        .build()
}

fn required_query_parameter(name: &str, description: &str) -> Parameter {
    ParameterBuilder::new()
        .name(name)
        .parameter_in(ParameterIn::Query)
        .required(Required::True)
        .description(Some(description))
        .build()
}

async fn health(
    State(state): State<ApiState>,
) -> Result<(StatusCode, Json<HealthResponse>), ApiError> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(ApiError::DatabaseUnavailable)?;
    let meilisearch = match state.search.health().await {
        Ok(()) => "ok",
        Err(error) => {
            tracing::warn!(%error, "search backend health check failed");
            "unavailable"
        }
    };
    let search_index = meilisearch;
    let status = if meilisearch == "ok" {
        "ok"
    } else {
        "degraded"
    };

    Ok((
        StatusCode::OK,
        Json(HealthResponse {
            status,
            postgres: "ok",
            meilisearch,
            search_index,
            search_sync: "external_reconciler",
        }),
    ))
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

async fn metrics_handler(State(state): State<ApiState>) -> Result<Response, ApiError> {
    state.metrics.refresh_sync_categories(&state.pool).await?;

    let encoder = prometheus::TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(ApiError::MetricsEncode)?;

    Ok(([(CONTENT_TYPE, encoder.format_type())], buffer).into_response())
}

async fn record_http_metrics(
    State(metrics): State<Arc<ApiMetrics>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.uri().path() == "/metrics" {
        return next.run(request).await;
    }

    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("_unmatched", MatchedPath::as_str)
        .to_owned();
    let started_at = Instant::now();
    let response = next.run(request).await;
    metrics.observe_http_request(
        &method,
        &route,
        response.status(),
        started_at.elapsed().as_secs_f64(),
    );
    response
}

fn timestamp_seconds(timestamp: Option<&str>) -> Option<f64> {
    let time = DateTime::parse_from_rfc3339(timestamp?).ok()?;
    let seconds = u64::try_from(time.timestamp()).ok()?;
    Some(Duration::new(seconds, time.timestamp_subsec_nanos()).as_secs_f64())
}

fn now_timestamp_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_secs_f64()
}

fn status_class(status: StatusCode) -> &'static str {
    match status.as_u16() {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "unknown",
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl-C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

async fn categories() -> Json<CategoryMetadataResponse> {
    Json(CategoryMetadataResponse {
        categories: read_model::list_category_metadata()
            .into_iter()
            .map(|category| CategoryMetadata {
                category: category.category,
                table: category.table,
                field_count: category.field_count,
                relation_count: category.relation_count,
            })
            .collect(),
    })
}

async fn sync_status(State(state): State<ApiState>) -> Result<Json<SyncStatusResponse>, ApiError> {
    Ok(Json(SyncStatusResponse {
        categories: read_model::list_category_progress(&state.pool)
            .await?
            .into_iter()
            .map(|category| CategorySyncStatus {
                category: category.category,
                latest_skiptoken: category.latest_skiptoken,
                state: category.state,
                last_fetch_at: category.last_fetch_at,
                last_synced_at: category.last_synced_at,
                caught_up_at: category.caught_up_at,
                next_url: category.next_url,
                resume_url: category.resume_url,
            })
            .collect(),
    }))
}

async fn changes(
    State(state): State<ApiState>,
    Path(category): Path<String>,
    query: Result<Query<ChangesQuery>, QueryRejection>,
) -> Result<Json<ChangePageResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let requested_limit = query.limit.unwrap_or(100);
    if requested_limit == 0 || requested_limit > CHANGES_API_MAX_LIMIT {
        return Err(ApiError::InvalidRequest);
    }
    let limit = i64::from(requested_limit);
    let page = read_model::list_changes(&state.pool, &category, query.after, limit).await?;
    Ok(Json(ChangePageResponse {
        category,
        items: page.items.into_iter().map(change_response).collect(),
        next_skiptoken: page.next_skiptoken,
        has_more: page.has_more,
    }))
}

async fn entity_list(
    State(state): State<ApiState>,
    Path(category): Path<String>,
    query: Result<Query<EntityListQuery>, QueryRejection>,
) -> Result<Json<EntityListResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    public_entity_list(state, category, query).await
}

async fn document_list(
    State(state): State<ApiState>,
    query: Result<Query<EntityListQuery>, QueryRejection>,
) -> Result<Json<EntityListResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    public_entity_list(state, "Document".to_owned(), query).await
}

async fn activity_list(
    State(state): State<ApiState>,
    query: Result<Query<EntityListQuery>, QueryRejection>,
) -> Result<Json<EntityListResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    public_entity_list(state, "Activiteit".to_owned(), query).await
}

async fn person_list(
    State(state): State<ApiState>,
    query: Result<Query<EntityListQuery>, QueryRejection>,
) -> Result<Json<EntityListResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    public_entity_list(state, "Persoon".to_owned(), query).await
}

async fn public_entity_list(
    state: ApiState,
    category: String,
    query: EntityListQuery,
) -> Result<Json<EntityListResponse>, ApiError> {
    let response_started_at = Instant::now();
    let effective_limit = effective_public_limit(query.limit, state.max_public_query_limit)?;
    let sort = public_sort(query.sort.as_deref())?;
    let mut page = read_model::query_public_entities(
        &state.pool,
        read_model::PublicEntityQuery {
            category: category.clone(),
            source_id: None,
            limit: i64::from(effective_limit),
            sort,
        },
    )
    .await?;
    let (relations_by_source_id, relation_query_count, relation_count) =
        expanded_relations_for_page(&state.pool, &page.items, query.relations).await?;
    page.stats.query_count += relation_query_count;
    page.stats.relation_count = relation_count;
    let items = page
        .items
        .into_iter()
        .map(|detail| {
            let relations = relations_by_source_id
                .get(&detail.metadata.source_id)
                .cloned()
                .map(|relations| relations.into_iter().map(relation_response).collect());
            detail_response(detail, relations)
        })
        .collect();
    let response_duration_ms = response_started_at.elapsed().as_millis();
    tracing::info!(
        category = %page.stats.category,
        effective_limit = page.stats.effective_limit,
        loaded_entity_count = page.stats.loaded_entity_count,
        relation_count = page.stats.relation_count,
        query_count = page.stats.query_count,
        query_duration_ms = page.stats.query_duration.as_millis(),
        materialization_duration_ms = page.stats.materialization_duration.as_millis(),
        response_duration_ms,
        sort = ?sort,
        source_id_filter = false,
        "public entity query completed"
    );
    Ok(Json(EntityListResponse {
        category,
        items,
        has_more: page.has_more,
        effective_limit,
    }))
}

async fn entity_detail(
    State(state): State<ApiState>,
    Path((category, source_id)): Path<(String, String)>,
    query: Result<Query<RelationsQuery>, QueryRejection>,
) -> Result<Json<EntityDetailResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let source_id = parse_uuid(&source_id)?;
    let detail = read_model::get_entity_detail(&state.pool, &category, source_id).await?;
    let relations = expanded_relations(&state.pool, &category, source_id, query.relations).await?;
    Ok(Json(detail_response(detail, relations)))
}

async fn document_detail(
    State(state): State<ApiState>,
    Path(source_id): Path<String>,
    query: Result<Query<RelationsQuery>, QueryRejection>,
) -> Result<Json<EntityDetailResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let source_id = parse_uuid(&source_id)?;
    let detail = read_model::get_document_detail(&state.pool, source_id).await?;
    let relations = expanded_relations(&state.pool, "Document", source_id, query.relations).await?;
    Ok(Json(detail_response(detail, relations)))
}

async fn document_content(
    State(state): State<ApiState>,
    Path(source_id): Path<String>,
) -> Result<Json<DocumentContentResponse>, ApiError> {
    let source_id = parse_uuid(&source_id)?;
    let detail = read_model::get_document_content(&state.pool, source_id).await?;
    Ok(Json(document_content_response(detail)))
}

async fn activity_detail(
    State(state): State<ApiState>,
    Path(source_id): Path<String>,
    query: Result<Query<RelationsQuery>, QueryRejection>,
) -> Result<Json<EntityDetailResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let source_id = parse_uuid(&source_id)?;
    let detail = read_model::get_activity_detail(&state.pool, source_id).await?;
    let relations =
        expanded_relations(&state.pool, "Activiteit", source_id, query.relations).await?;
    Ok(Json(detail_response(detail, relations)))
}

async fn person_detail(
    State(state): State<ApiState>,
    Path(source_id): Path<String>,
    query: Result<Query<RelationsQuery>, QueryRejection>,
) -> Result<Json<EntityDetailResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let source_id = parse_uuid(&source_id)?;
    let detail = read_model::get_person_detail(&state.pool, source_id).await?;
    let relations = expanded_relations(&state.pool, "Persoon", source_id, query.relations).await?;
    Ok(Json(detail_response(detail, relations)))
}

async fn relations(
    State(state): State<ApiState>,
    Path((category, source_id)): Path<(String, String)>,
    query: Result<Query<RelationLookupQuery>, QueryRejection>,
) -> Result<Json<RelationLookupResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let source_id = parse_uuid(&source_id)?;
    let direction = match query.direction {
        RelationLookupDirection::Outgoing => RelationDirection::Outgoing,
        RelationLookupDirection::Incoming => RelationDirection::Incoming,
        RelationLookupDirection::Both => RelationDirection::Both,
    };
    Ok(Json(RelationLookupResponse {
        items: read_model::list_relations(&state.pool, &category, source_id, direction)
            .await?
            .into_iter()
            .map(relation_response)
            .collect(),
    }))
}

async fn expanded_relations(
    pool: &PgPool,
    category: &str,
    source_id: uuid::Uuid,
    expansion: RelationExpansion,
) -> Result<Option<Vec<RelationResponse>>, ApiError> {
    let direction = match expansion {
        RelationExpansion::None => return Ok(None),
        RelationExpansion::Outgoing => RelationDirection::Outgoing,
        RelationExpansion::Incoming => RelationDirection::Incoming,
        RelationExpansion::Both => RelationDirection::Both,
    };
    Ok(Some(
        read_model::list_relations(pool, category, source_id, direction)
            .await?
            .into_iter()
            .map(relation_response)
            .collect(),
    ))
}

async fn expanded_relations_for_page(
    pool: &PgPool,
    items: &[EntityDetail],
    expansion: RelationExpansion,
) -> Result<(HashMap<uuid::Uuid, Vec<RelationRow>>, usize, usize), ApiError> {
    let direction = match expansion {
        RelationExpansion::None => return Ok((HashMap::new(), 0, 0)),
        RelationExpansion::Outgoing => RelationDirection::Outgoing,
        RelationExpansion::Incoming => RelationDirection::Incoming,
        RelationExpansion::Both => RelationDirection::Both,
    };
    let Some(first) = items.first() else {
        return Ok((HashMap::new(), 0, 0));
    };
    let category = first.metadata.category.as_str();
    let source_ids = items
        .iter()
        .map(|item| item.metadata.source_id)
        .collect::<Vec<_>>();
    let rows =
        read_model::list_relations_for_entities(pool, category, &source_ids, direction).await?;
    let relation_count = rows.len();
    Ok((
        read_model::group_relations_by_entity(category, &source_ids, direction, rows),
        1,
        relation_count,
    ))
}

fn effective_public_limit(requested: Option<u32>, configured_max: u32) -> Result<u32, ApiError> {
    if matches!(requested, Some(0)) || configured_max == 0 {
        return Err(ApiError::InvalidRequest);
    }
    Ok(requested.map_or(configured_max, |limit| limit.min(configured_max)))
}

fn public_sort(value: Option<&str>) -> Result<read_model::PublicEntitySort, ApiError> {
    match value.unwrap_or("latest_desc") {
        "latest_asc" => Ok(read_model::PublicEntitySort::LatestAsc),
        "latest_desc" => Ok(read_model::PublicEntitySort::LatestDesc),
        "date_asc" => Ok(read_model::PublicEntitySort::DateAsc),
        "date_desc" => Ok(read_model::PublicEntitySort::DateDesc),
        "name_asc" => Ok(read_model::PublicEntitySort::NameAsc),
        "name_desc" => Ok(read_model::PublicEntitySort::NameDesc),
        _ => Err(ApiError::InvalidRequest),
    }
}

fn detail_response(
    detail: EntityDetail,
    relations: Option<Vec<RelationResponse>>,
) -> EntityDetailResponse {
    EntityDetailResponse {
        metadata: change_response(detail.metadata),
        fields: detail.fields,
        relations,
    }
}

fn document_content_response(detail: DocumentContentDetail) -> DocumentContentResponse {
    DocumentContentResponse {
        document: DocumentIdentityResponse {
            source_category: detail.document_source_category,
            source_id: detail.document_source_id.to_string(),
        },
        asset: DocumentAssetResponse {
            id: detail.asset.id,
            asset_url: detail.asset.asset_url,
            upstream_url: detail.asset.upstream_url,
            upstream_content_type: detail.asset.upstream_content_type,
            upstream_content_length: detail.asset.upstream_content_length,
            upstream_last_modified_at: detail.asset.upstream_last_modified_at,
            retrieval_status: detail.asset.retrieval_status,
            retrieval_error: detail.asset.retrieval_error,
            retrieved_at: detail.asset.retrieved_at,
        },
        content: DocumentContentBodyResponse {
            id: detail.content.id,
            selected_source_url: detail.content.selected_source_url,
            selected_source_content_type: detail.content.selected_source_content_type,
            selected_source_content_length: detail.content.selected_source_content_length,
            official_source: detail.content.official_source,
            source_rank: detail.content.source_rank,
            extraction_status: detail.content.extraction_status,
            validation_status: detail.content.validation_status,
            extraction_tool: detail.content.extraction_tool,
            extraction_tool_version: detail.content.extraction_tool_version,
            source_hash: detail.content.source_hash,
            output_hash: detail.content.output_hash,
            extraction_error: detail.content.extraction_error,
            extracted_text: detail.content.extracted_text,
            extracted_html: detail.content.extracted_html,
            extracted_at: detail.content.extracted_at,
        },
    }
}

fn change_response(change: EntityChange) -> EntityChangeResponse {
    EntityChangeResponse {
        category: change.category,
        source_id: change.source_id.to_string(),
        latest_skiptoken: change.latest_skiptoken,
        deleted: change.deleted,
        source_updated_at: change.source_updated_at,
        atom_updated_at: change.atom_updated_at,
    }
}

fn relation_response(relation: RelationRow) -> RelationResponse {
    RelationResponse {
        source_category: relation.source_category,
        source_id: relation.source_id.to_string(),
        relation_name: relation.relation_name,
        target_category: relation.target_category,
        target_id: relation.target_id.to_string(),
        ordinal: relation.ordinal,
        source_updated_at: relation.source_updated_at,
    }
}

fn parse_uuid(value: &str) -> Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(value).map_err(|_| ApiError::InvalidRequest)
}
