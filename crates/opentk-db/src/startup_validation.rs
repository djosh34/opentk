use std::{fmt, time::Duration};

use opentk_config::Config;
use opentk_search::{MeilisearchClient, SearchIndexError};
use reqwest::{StatusCode, Url};
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;
use tokio::time::timeout;

use crate::DatabaseConfig;

pub const STARTUP_VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchRequirement {
    Optional,
    Required,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyValidationReport {
    pub database: DependencyCheckStatus,
    pub search: Option<DependencyCheckStatus>,
    pub syncfeed: Option<DependencyCheckStatus>,
}

impl fmt::Display for DependencyValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.database)?;
        if let Some(search) = &self.search {
            writeln!(f, "{search}")?;
        }
        if let Some(syncfeed) = &self.syncfeed {
            writeln!(f, "{syncfeed}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyCheckStatus {
    Reachable {
        dependency: DependencyKind,
        target: String,
    },
    Degraded {
        dependency: DependencyKind,
        target: String,
        error: DependencyValidationError,
    },
}

impl fmt::Display for DependencyCheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reachable { dependency, target } => {
                write!(f, "{dependency} at {target} is reachable")
            }
            Self::Degraded {
                dependency,
                target,
                error,
            } => write!(f, "{dependency} at {target} is degraded: {error}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyKind {
    Database,
    Meilisearch,
    SyncFeed,
}

impl fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database => f.write_str("Database"),
            Self::Meilisearch => f.write_str("Meilisearch"),
            Self::SyncFeed => f.write_str("SyncFeed base URL"),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DependencyValidationError {
    #[error("Database URL cannot be safely redacted: {message}")]
    DatabaseUrlRedactionFailed { message: String },
    #[error("Database at {target} is unreachable: {message}")]
    DatabaseUnreachable { target: String, message: String },
    #[error("Meilisearch at {target} is unreachable: {message}")]
    MeilisearchUnreachable { target: String, message: String },
    #[error("Meilisearch at {target} returned {status}: {message}")]
    MeilisearchStatus {
        target: String,
        status: StatusCode,
        message: String,
    },
    #[error("SyncFeed base URL {target} is unreachable: {message}")]
    SyncFeedUnreachable { target: String, message: String },
    #[error("SyncFeed base URL {target} returned {status}: {message}")]
    SyncFeedStatus {
        target: String,
        status: StatusCode,
        message: String,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DatabaseUrlRedactionError {
    #[error("invalid URL: {message}")]
    InvalidUrl { message: String },
    #[error("password redaction failed")]
    PasswordRedactionFailed,
}

impl From<DatabaseUrlRedactionError> for DependencyValidationError {
    fn from(error: DatabaseUrlRedactionError) -> Self {
        Self::DatabaseUrlRedactionFailed {
            message: error.to_string(),
        }
    }
}

/// Validate dependencies needed by `opentk-api`.
///
/// # Errors
///
/// Returns [`DependencyValidationError`] when `PostgreSQL` is unavailable, or
/// when Meilisearch is unavailable and [`SearchRequirement::Required`] is used.
pub async fn validate_api_dependencies(
    config: &Config,
    search_requirement: SearchRequirement,
) -> Result<DependencyValidationReport, DependencyValidationError> {
    let database = validate_database(config).await?;
    let search_result = validate_search(config).await;
    let search = match search_result {
        Ok(status) => status,
        Err(error) if search_requirement == SearchRequirement::Optional => {
            DependencyCheckStatus::Degraded {
                dependency: DependencyKind::Meilisearch,
                target: config.search.url.clone(),
                error,
            }
        }
        Err(error) => return Err(error),
    };
    Ok(DependencyValidationReport {
        database,
        search: Some(search),
        syncfeed: None,
    })
}

/// Validate dependencies needed by search synchronization.
///
/// # Errors
///
/// Returns [`DependencyValidationError`] when `PostgreSQL` or Meilisearch cannot
/// be reached within the startup validation timeout.
pub async fn validate_search_sync_dependencies(
    config: &Config,
) -> Result<DependencyValidationReport, DependencyValidationError> {
    Ok(DependencyValidationReport {
        database: validate_database(config).await?,
        search: Some(validate_search(config).await?),
        syncfeed: None,
    })
}

/// Validate dependencies needed by `opentk-sync`.
///
/// # Errors
///
/// Returns [`DependencyValidationError`] when `PostgreSQL` or the configured
/// `SyncFeed` base URL cannot be reached within the startup validation timeout.
pub async fn validate_sync_dependencies(
    config: &Config,
) -> Result<DependencyValidationReport, DependencyValidationError> {
    Ok(DependencyValidationReport {
        database: validate_database(config).await?,
        search: None,
        syncfeed: Some(validate_syncfeed(config).await?),
    })
}

/// Validate `PostgreSQL` from full application config.
///
/// # Errors
///
/// Returns [`DependencyValidationError::DatabaseUnreachable`] when connecting,
/// running `SELECT 1`, or completing within the validation timeout fails.
pub async fn validate_database(
    config: &Config,
) -> Result<DependencyCheckStatus, DependencyValidationError> {
    validate_database_config(&DatabaseConfig {
        url: config.database.url.clone(),
        max_connections: config.database.max_connections,
    })
    .await
}

/// Validate `PostgreSQL` from database-only runtime config.
///
/// # Errors
///
/// Returns [`DependencyValidationError::DatabaseUnreachable`] when connecting,
/// running `SELECT 1`, or completing within the validation timeout fails.
pub async fn validate_database_config(
    config: &DatabaseConfig,
) -> Result<DependencyCheckStatus, DependencyValidationError> {
    let target = redact_database_url(&config.url)?;
    let check = async {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&config.url)
            .await?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&pool)
            .await?;
        pool.close().await;
        Ok::<(), sqlx::Error>(())
    };
    timeout(STARTUP_VALIDATION_TIMEOUT, check)
        .await
        .map_err(|_| DependencyValidationError::DatabaseUnreachable {
            target: target.clone(),
            message: "timeout".to_owned(),
        })?
        .map_err(|source| DependencyValidationError::DatabaseUnreachable {
            target: target.clone(),
            message: source.to_string(),
        })?;
    Ok(DependencyCheckStatus::Reachable {
        dependency: DependencyKind::Database,
        target,
    })
}

/// Validate Meilisearch from full application config.
///
/// # Errors
///
/// Returns [`DependencyValidationError`] when the lightweight stats request
/// fails, returns a non-success status, or times out.
pub async fn validate_search(
    config: &Config,
) -> Result<DependencyCheckStatus, DependencyValidationError> {
    validate_meilisearch_config(
        config.search.url.clone(),
        config.search.api_key.clone(),
        config.search.index_name.clone(),
    )
    .await
}

/// Validate Meilisearch from search-only runtime settings.
///
/// # Errors
///
/// Returns [`DependencyValidationError`] when the lightweight stats request
/// fails, returns a non-success status, or times out.
pub async fn validate_meilisearch_config(
    url: String,
    api_key: Option<String>,
    index_name: String,
) -> Result<DependencyCheckStatus, DependencyValidationError> {
    let target = url.clone();
    let client = MeilisearchClient::new(url, api_key, index_name);
    timeout(STARTUP_VALIDATION_TIMEOUT, client.validate_reachable())
        .await
        .map_err(|_| DependencyValidationError::MeilisearchUnreachable {
            target: target.clone(),
            message: "timeout".to_owned(),
        })?
        .map_err(|source| map_search_error(target.clone(), source))?;
    Ok(DependencyCheckStatus::Reachable {
        dependency: DependencyKind::Meilisearch,
        target,
    })
}

async fn validate_syncfeed(
    config: &Config,
) -> Result<DependencyCheckStatus, DependencyValidationError> {
    let target = config.sync.base_url.to_string();
    let client = reqwest::Client::builder()
        .connect_timeout(STARTUP_VALIDATION_TIMEOUT)
        .build()
        .map_err(|source| DependencyValidationError::SyncFeedUnreachable {
            target: target.clone(),
            message: source.to_string(),
        })?;
    let response = timeout(
        STARTUP_VALIDATION_TIMEOUT,
        client
            .get(config.sync.base_url.clone())
            .timeout(STARTUP_VALIDATION_TIMEOUT)
            .send(),
    )
    .await
    .map_err(|_| DependencyValidationError::SyncFeedUnreachable {
        target: target.clone(),
        message: "timeout".to_owned(),
    })?
    .map_err(|source| DependencyValidationError::SyncFeedUnreachable {
        target: target.clone(),
        message: source.to_string(),
    })?;
    let status = response.status();
    if !status.is_success() {
        let body =
            response
                .text()
                .await
                .map_err(|source| DependencyValidationError::SyncFeedStatus {
                    target: target.clone(),
                    status,
                    message: format!("failed to read response body: {source}"),
                })?;
        return Err(DependencyValidationError::SyncFeedStatus {
            target,
            status,
            message: body,
        });
    }
    Ok(DependencyCheckStatus::Reachable {
        dependency: DependencyKind::SyncFeed,
        target,
    })
}

fn map_search_error(target: String, source: SearchIndexError) -> DependencyValidationError {
    match source {
        SearchIndexError::Http {
            status: Some(status),
            message,
        } => DependencyValidationError::MeilisearchStatus {
            target,
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            message,
        },
        SearchIndexError::Http { message, .. } | SearchIndexError::InvalidResponse(message) => {
            DependencyValidationError::MeilisearchUnreachable { target, message }
        }
        SearchIndexError::Mapping(error) => DependencyValidationError::MeilisearchUnreachable {
            target,
            message: error.to_string(),
        },
    }
}

/// Redact credentials from a database URL before it is used in diagnostics.
///
/// # Errors
///
/// Returns [`DatabaseUrlRedactionError`] when the URL is invalid or password
/// mutation fails.
pub fn redact_database_url(input: &str) -> Result<String, DatabaseUrlRedactionError> {
    let mut url = Url::parse(input).map_err(|source| DatabaseUrlRedactionError::InvalidUrl {
        message: source.to_string(),
    })?;
    if url.password().is_some() {
        url.set_password(Some("***"))
            .map_err(|()| DatabaseUrlRedactionError::PasswordRedactionFailed)?;
    }
    Ok(url.to_string())
}
