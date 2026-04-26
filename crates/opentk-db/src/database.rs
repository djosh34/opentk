use sqlx::{postgres::PgPoolOptions, PgPool};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("failed to connect to PostgreSQL")]
    Connect(#[source] sqlx::Error),
}

/// Opens a `PostgreSQL` connection pool from typed database configuration.
///
/// # Errors
///
/// Returns [`DatabaseError::Connect`] when `SQLx` cannot establish the pool.
pub async fn connect(config: &DatabaseConfig) -> Result<PgPool, DatabaseError> {
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.url)
        .await
        .map_err(DatabaseError::Connect)
}
