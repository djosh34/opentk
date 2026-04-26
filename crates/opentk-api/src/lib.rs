//! HTTP API boundary for `OpenTK`.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use opentk_db::{connect, DatabaseConfig, DatabaseError};
use serde::Serialize;
use sqlx::PgPool;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::net::TcpListener;
use utoipa::{
    openapi::{
        path::{HttpMethod, OperationBuilder, PathItem, PathsBuilder},
        response::ResponseBuilder,
        schema::ComponentsBuilder,
        Content, Info, Ref,
    },
    ToSchema,
};

const DEFAULT_MAX_DATABASE_CONNECTIONS: u32 = 5;

#[derive(Clone)]
struct ApiState {
    pool: PgPool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiConfig {
    pub bind_address: SocketAddr,
    pub database_url: String,
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
            Self::Database(_) | Self::Bind { .. } | Self::Serve(_) => (
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

/// Builds an API router and stores the bind address that should be used by the listener.
///
/// # Errors
///
/// Returns [`ApiError::Database`] when the configured `PostgreSQL` pool cannot be
/// initialized.
pub async fn build_app(config: ApiConfig) -> Result<ApiServer, ApiError> {
    let pool = connect(&DatabaseConfig {
        url: config.database_url,
        max_connections: DEFAULT_MAX_DATABASE_CONNECTIONS,
    })
    .await?;

    Ok(ApiServer {
        bind_address: config.bind_address,
        router: router(pool),
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
    Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(openapi_json))
        .with_state(ApiState { pool })
}

#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    let paths = PathsBuilder::new()
        .path(
            "/health",
            PathItem::new(
                HttpMethod::Get,
                OperationBuilder::new()
                    .operation_id(Some("health"))
                    .response(
                        "200",
                        json_response(
                            "API and database are reachable",
                            HealthResponse::name().as_ref(),
                        ),
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
        .build();

    let mut document = utoipa::openapi::OpenApi::new(Info::new("OpenTK API", "0.1.0"), paths);
    document.components = Some(
        ComponentsBuilder::new()
            .schema_from::<ErrorResponse>()
            .schema_from::<HealthResponse>()
            .build(),
    );
    document
}

fn json_response(description: &str, schema_name: &str) -> ResponseBuilder {
    ResponseBuilder::new().description(description).content(
        "application/json",
        Content::new(Some(Ref::from_schema_name(schema_name))),
    )
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
