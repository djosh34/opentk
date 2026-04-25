//! `SyncFeed` ingestion boundary for `OpenTK`.
//!
//! Fetching, parsing, and applying `SyncFeed` pages belong here. Shared domain
//! decisions stay in `opentk-core`; database writes go through `opentk-db`.

pub mod official_schema;
pub mod payload;
pub mod syncfeed;
