//! `PostgreSQL` database boundary for `OpenTK`.
//!
//! `SQLx` repositories, transaction orchestration, and migration-facing database
//! code belong here. Source-neutral domain decisions stay in `opentk-core`.

mod database;

pub mod document_assets;
pub mod postgres_schema;
pub mod read_model;
pub mod schema_lifecycle;
pub mod search_projection;
pub mod search_reconciler;
pub mod startup_validation;
pub mod sync_state;
pub mod sync_verification;
pub mod sync_writer;

pub use database::{connect, DatabaseConfig, DatabaseError};
