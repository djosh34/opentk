//! `PostgreSQL` database boundary for `OpenTK`.
//!
//! `SQLx` repositories, transaction orchestration, and migration-facing database
//! code belong here. Source-neutral domain decisions stay in `opentk-core`.

pub mod postgres_schema;
pub mod sync_state;
pub mod sync_verification;
pub mod sync_writer;
