# Plan: Implement LISTEN/NOTIFY CDC for Search Sync

## Current Read

The database/search code already has these relevant surfaces:

- `migrations/20260426000000_sync_schema.up.sql` and `.down.sql` are generated from `crates/opentk-db/src/postgres_schema.rs`; tests assert checked-in migration SQL exactly matches `render_up_migration` and `render_down_migration`.
- `sync_entity` is the registry table every entity write touches through `sync_writer`.
- `crates/opentk-db/src/search_sync.rs` exposes cursor-based `full_reindex` and `incremental_index`.
- `incremental_index` indexes all records after the durable per-category cursor and advances that cursor.
- Existing tests use real PostgreSQL schemas and a `MemoryIndexClient` through the public `SearchIndexClient` boundary.

Important design constraint: targeted CDC indexing must not advance the durable category cursor from a per-record notification. If one `Document` notification with skiptoken 100 advances the `Document` cursor to 100 while skiptoken 99 was not flushed yet, the normal incremental lane can lose work. Targeted CDC should apply search operations for notified records directly; the existing cursor-based `incremental_index` remains the category catch-up/fallback path.

## Public Interface

Add a new public module from `crates/opentk-db/src/lib.rs`:

- `pub mod search_cdc;`

In `crates/opentk-db/src/search_cdc.rs` expose:

- `pub const SEARCH_CDC_CHANNEL: &str = "sync_entity_change";`
- `pub const DEFAULT_SEARCH_CDC_FLUSH_WINDOW: Duration = Duration::from_secs(1);`
- `pub const DEFAULT_SEARCH_CDC_MAX_UNIQUE_RECORDS: usize = 100;`
- `pub struct SearchCdcNotification` with `source_category`, `source_id`, `latest_skiptoken`, and `deleted`.
- `pub struct SearchCdcBatchConfig` with `flush_window` and `max_unique_records`.
- `pub struct SearchCdcBatcher` with `push`, `should_flush`, `drain`, and `is_empty`.
- `pub struct SearchCdcListener` as the deep module that owns `PgListener`, batching, and flush behavior.

Keep fields private unless tests and callers need direct read access. Prefer constructor/accessor methods over public struct fields if that keeps invariants tighter.

Extend `crates/opentk-db/src/search_sync.rs` with one focused public function:

- `pub async fn index_records<C>(pool: &PgPool, client: &C, config: &SearchSyncConfig, records: &[SearchSyncRecordKey]) -> Result<SearchSyncReport, SearchSyncError>`

`SearchSyncRecordKey` should carry `source_category`, `source_id`, and `latest_skiptoken`. It should reuse existing mapping internals, validate categories through the same code path, dedupe/sort records deterministically, apply upsert/delete operations, and return a report. It must not reset or advance `search_index_cursor`.

## TDD Plan

Use the `tdd` skill as the execution loop: one behavior test, smallest implementation, repeat.

1. RED: add a unit test in `crates/opentk-db/tests/search_cdc.rs` that parses a valid notification JSON payload into a typed notification.
   GREEN: implement `SearchCdcNotification::from_payload`.

2. RED: add a unit test that invalid notification payloads return a typed error and do not silently default missing/invalid fields.
   GREEN: add `SearchCdcError` variants using `thiserror` and `serde_json` parsing.

3. RED: add a unit test that batching deduplicates repeated `(source_category, source_id)` notifications and keeps the newest `latest_skiptoken` for that key.
   GREEN: implement `SearchCdcBatcher` around one `HashMap` keyed by a typed key, not parallel vectors or string-concatenated keys.

4. RED: add a unit test that batching requests flush when either the configured 1-second window elapses or 100 unique records are buffered.
   GREEN: implement `SearchCdcBatchConfig`, `push`, `should_flush`, and `drain`.

5. RED: add a behavior test in `crates/opentk-db/tests/search_sync.rs` that `index_records` indexes only the affected source ids and leaves the durable `search_index_cursor` unchanged.
   GREEN: refactor existing private `source_record` / `operations_for_changes` code only as much as needed so targeted records and cursor-based changes share mapping behavior.

6. RED: add a test that `index_records` rejects unknown categories and invalid batch sizes through the existing `SearchSyncError` contract.
   GREEN: reuse `validate_config` / `ensure_category`; avoid separate validation logic.

7. RED: add an integration test in `crates/opentk-db/tests/search_cdc.rs` using a migrated PostgreSQL schema and `sqlx::postgres::PgListener` that starts `LISTEN sync_entity_change`, inserts/updates `sync_entity`, and receives a notification with the expected payload.
   GREEN: add trigger generation to `postgres_schema::render_up_migration` and matching function cleanup to `render_down_migration`; update checked-in migration SQL from the renderer output.

8. RED: add a listener behavior test that feeds one parsed notification through the listener/batcher flush path and observes `MemoryIndexClient` receiving the targeted operation.
   GREEN: implement `SearchCdcListener` with a dedicated `PgListener::connect(database_url)` path and a narrow flush method that calls `index_records`.

## Implementation Notes

Migration:

- Add a generated `notify_sync_entity_change()` PL/pgSQL function and `sync_entity_change_trigger` in `render_up_migration` after the `sync_entity` table exists.
- Payload fields must be `source_category`, `source_id`, `latest_skiptoken`, and `deleted`.
- Trigger should be `AFTER INSERT OR UPDATE ON sync_entity FOR EACH ROW`.
- Add `DROP FUNCTION IF EXISTS notify_sync_entity_change() CASCADE;` to the rendered down migration so rollback does not leave schema objects behind.
- Keep this in `postgres_schema.rs` rather than hand-editing only the SQL files, because `checked_in_migrations_match_schema_spec` treats the migration as generated output.

Listener:

- Use `sqlx::postgres::PgListener` for `LISTEN`, with a dedicated connection string rather than the pool.
- `SearchCdcListener::connect(database_url, pool, client, search_config, batch_config)` should call `LISTEN sync_entity_change`.
- Runtime loop should receive notifications, parse payloads, push into the batcher, and flush on window or max unique records.
- Do not swallow parse/index errors. Return them or surface them through the listener result type.
- If a category has many changes in one flush, call existing `incremental_index` for that category only by cloning `SearchSyncConfig` with those categories. Otherwise use `index_records`.

## Improve Code Boundaries Plan

Use the `improve-code-boundaries` skill during execution and final review:

- Avoid wrong-place runtime courier logic: `SearchCdcListener` should own `PgListener`, batching, parse, and flush orchestration. API integration belongs to the later task unless this task explicitly needs a minimal constructor.
- Avoid string soup: parse JSON once into `SearchCdcNotification`; do not pass raw payload strings beyond the parse boundary.
- Avoid duplicate connection shapes: accept a database URL string for `PgListener::connect` and reuse the existing `PgPool` for SQL reads/writes instead of creating ad-hoc connection structs.
- Avoid overengineering: one listener module, one batcher, one targeted `index_records` search-sync entry point. No generic CDC framework.
- Keep visibility tight: export only the types/functions tests and later API integration need.
- Final boundary check: look for duplicated notification key shapes, raw JSON maps, category validation outside `search_sync`, and helper functions with only one caller.

## Acceptance Mapping

- Migration adds trigger: covered by migration SQL match test and integration test against real PostgreSQL.
- `SearchCdcListener` receives notifications via `PgListener`: covered by listener/connect or integration test.
- Notifications deduplicate within 1-second window: covered by `SearchCdcBatcher` behavior tests.
- Listener triggers incremental sync for affected records: covered by targeted flush behavior test using `SearchIndexClient`.
- Unit tests verify parsing and dedup logic: covered in `crates/opentk-db/tests/search_cdc.rs`.
- Integration test verifies trigger fires and listener receives it: covered with real migrated schema and `PgListener`.
- Required final gates: run `make check`, `make test`, and `make lint`. Do not run `make test-long` for this task.

## Execution Checklist

- [x] Add first RED notification parsing test.
- [x] Implement typed notification parsing.
- [x] Add invalid payload test and typed errors.
- [x] Add batch dedup test.
- [x] Implement batcher.
- [x] Add flush threshold/window test.
- [x] Add targeted `index_records` behavior test.
- [x] Implement targeted indexing without cursor advancement.
- [x] Add invalid `index_records` validation test.
- [x] Add migration trigger integration test.
- [x] Implement generated trigger SQL and down cleanup.
- [x] Add listener receive/flush behavior test.
- [x] Implement `SearchCdcListener`.
- [x] Run `make check`.
- [x] Run `make test`.
- [x] Run `make lint`.
- [x] Final improve-code-boundaries review and cleanup.

NOW EXECUTE
