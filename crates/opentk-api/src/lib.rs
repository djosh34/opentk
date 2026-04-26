//! HTTP API boundary for `OpenTK`.

mod search;

use axum::{
    extract::{rejection::QueryRejection, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use opentk_db::{
    connect,
    read_model::{
        self, DocumentContentDetail, EntityChange, EntityDetail, ReadModelError, RelationDirection,
        RelationRow,
    },
    DatabaseConfig, DatabaseError,
};
use opentk_search::{MeilisearchClient, SearchIndexError, SearchQueryClient};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::PgPool;
use std::{net::SocketAddr, sync::Arc};
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

const DEFAULT_SEARCH_URL: &str = "http://meilisearch:7700";
const DEFAULT_SEARCH_INDEX: &str = "opentk_entities";

#[derive(Clone)]
struct ApiState {
    pool: PgPool,
    search: Arc<dyn SearchQueryClient + Send + Sync>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiConfig {
    pub bind_address: SocketAddr,
    pub database: DatabaseConfig,
    pub search: SearchBackendConfig,
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
            | Self::ReadModel(ReadModelError::Sql(_))
            | Self::Bind { .. }
            | Self::Serve(_) => (
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
    let pool = connect(&config.database).await?;
    let search = Arc::new(MeilisearchClient::new(
        config.search.url,
        config.search.api_key,
        config.search.index_name,
    ));

    Ok(ApiServer {
        bind_address: config.bind_address,
        router: router_with_search(pool, search),
    })
}

/// Starts the HTTP API server and serves requests until the listener exits.
///
/// # Errors
///
/// Returns [`ApiError::Database`] when database initialization fails,
/// [`ApiError::Bind`] when the configured socket cannot be bound, or
/// [`ApiError::Serve`] when `Axum` reports a serving failure.
pub async fn serve(config: ApiConfig) -> Result<(), ApiError> {
    let server = build_app(config).await?;
    let listener = TcpListener::bind(server.bind_address)
        .await
        .map_err(|source| ApiError::Bind {
            address: server.bind_address,
            source,
        })?;

    axum::serve(listener, server.router)
        .await
        .map_err(ApiError::Serve)
}

pub fn router(pool: PgPool) -> Router {
    router_with_search(pool, default_search_client_without_env())
}

pub fn router_with_search(
    pool: PgPool,
    search_client: Arc<dyn SearchQueryClient + Send + Sync>,
) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(openapi_json))
        .route("/search", get(search::search))
        .route("/categories", get(categories))
        .route("/sync/status", get(sync_status))
        .route("/changes/{category}", get(changes))
        .route("/entities/{category}/{source_id}", get(entity_detail))
        .route("/documents/{source_id}", get(document_detail))
        .route("/documents/{source_id}/content", get(document_content))
        .route("/activities/{source_id}", get(activity_detail))
        .route("/persons/{source_id}", get(person_detail))
        .route("/relations/{category}/{source_id}", get(relations))
        .with_state(ApiState {
            pool,
            search: search_client,
        })
}

fn default_search_client_without_env() -> Arc<dyn SearchQueryClient + Send + Sync> {
    Arc::new(MeilisearchClient::new(
        DEFAULT_SEARCH_URL.to_owned(),
        None,
        DEFAULT_SEARCH_INDEX.to_owned(),
    ))
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
                "/sync/status",
                PathItem::new(
                    HttpMethod::Get,
                    get_operation(
                        "sync_status",
                        "Category sync status",
                        SyncStatusResponse::name().as_ref(),
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
    relation_paths(typed_detail_paths(paths.path(
        "/entities/{category}/{source_id}",
        PathItem::new(HttpMethod::Get, generic_detail_operation()),
    )))
}

fn typed_detail_paths(paths: PathsBuilder) -> PathsBuilder {
    paths
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
            "/activities/{source_id}",
            PathItem::new(
                HttpMethod::Get,
                typed_detail_operation("activity_detail", "Activity detail", "Activity"),
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
        .schema_from::<SyncStatusResponse>()
        .schema_from::<CategorySyncStatus>()
        .schema_from::<ChangePageResponse>()
        .schema_from::<EntityChangeResponse>()
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

async fn health(State(state): State<ApiState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(ApiError::DatabaseUnavailable)?;

    Ok(Json(HealthResponse { status: "ok" }))
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
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
    let limit = i64::from(query.limit.unwrap_or(100));
    let page = read_model::list_changes(&state.pool, &category, query.after, limit).await?;
    Ok(Json(ChangePageResponse {
        category,
        items: page.items.into_iter().map(change_response).collect(),
        next_skiptoken: page.next_skiptoken,
        has_more: page.has_more,
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
