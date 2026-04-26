//! `PostgreSQL` database boundary for `OpenTK`.
//!
//! `SQLx` repositories, transaction orchestration, and migration-facing database
//! code belong here. Source-neutral domain decisions stay in `opentk-core`.

mod database;

pub mod document_assets;
pub mod postgres_schema;
pub mod read_model;
pub mod search_cdc;
pub mod search_sync;
pub mod startup_validation;
pub mod sync_state;
pub mod sync_verification;
pub mod sync_writer;

pub use database::{connect, DatabaseConfig, DatabaseError};
