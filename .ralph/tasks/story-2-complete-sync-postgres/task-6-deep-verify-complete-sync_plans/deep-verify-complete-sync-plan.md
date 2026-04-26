# Plan: Deep Verify Complete PostgreSQL Sync

## Sources

- Task file: `.ralph/tasks/story-2-complete-sync-postgres/task-6-deep-verify-complete-sync.md`
- Story 2 tasks 1-5 implementation:
  - `opentk-core::official_schema` is the official model oracle derived from vendored XSD metadata.
  - `opentk-db::postgres_schema` renders the PostgreSQL schema from that oracle.
  - `opentk-sync::payload` parses SyncFeed embedded entity XML into one typed parsed shape.
  - `opentk-db::sync_writer` writes one parsed page transactionally.
  - `opentk-sync::runner` coordinates per-category catch-up, restart, concurrency, and durable error reporting.
- `tdd` skill: execute by vertical red-green tracer bullets through public behavior. One failing behavior test, minimal implementation, repeat.
- `improve-code-boundaries` skill: keep verification as a deep module with one oracle/report shape. Avoid duplicate official lists, duplicate SQL-name rendering, CLI-owned verification logic, and stringly coverage assertions spread across tests.

## Boundary Design

Add a verification boundary in `opentk-db` because the final proof is PostgreSQL-readiness:

- `opentk-db::sync_verification` owns deterministic database verification, direct SQL probes, relation checks, idempotency snapshots, update replacement checks, and storage measurements.
- `opentk-core::official_schema` remains the only official entity/field/relation oracle.
- `opentk-db::postgres_schema` remains the only schema/table/column naming oracle.
- `opentk-sync::runner` remains the only live catch-up orchestration boundary.
- The `complete-sync` CLI may call the verification module only as thin bootstrap; it must not contain schema walking, SQL assertion, or report construction logic.

Do not add a second fixture model, hard-coded official category list, or parallel DTOs that mirror `EntityType`, `Field`, or `TableSpec`. Verification reports should reference existing names from the official/schema layers and include counts/evidence, not re-declare model truth.

## Public Verification Interface

Add `crates/opentk-db/src/sync_verification.rs`:

```rust
pub struct SyncVerificationConfig {
    pub categories: Vec<String>,
    pub required_relation_samples: usize,
}

pub struct SyncVerificationReport {
    pub categories: Vec<CategoryVerification>,
    pub relation_tables: Vec<RelationVerification>,
    pub direct_queries: Vec<DirectQueryVerification>,
    pub storage: StorageVerification,
}

pub struct CategoryVerification {
    pub category: String,
    pub table_name: String,
    pub current_rows: i64,
    pub registry_rows: i64,
    pub latest_skiptoken: Option<i64>,
    pub state: CategorySyncState,
}

pub struct RelationVerification {
    pub source_category: String,
    pub relation_name: String,
    pub table_name: String,
    pub rows: i64,
    pub queryable_from_source: bool,
    pub queryable_from_target: bool,
}

pub struct DirectQueryVerification {
    pub name: String,
    pub rows: i64,
}

pub struct StorageVerification {
    pub table_bytes: i64,
    pub index_bytes: i64,
    pub html_asset_bytes: i64,
    pub binary_asset_metadata_rows: i64,
}

pub async fn verify_sync_database(
    pool: &sqlx::PgPool,
    config: SyncVerificationConfig,
) -> Result<SyncVerificationReport, SyncVerificationError>;
```

Keep the report plain and easy to test. It should be a product of public SQL-visible state, not private writer structs. If the implementation needs helper structs, keep them private unless tests need them through the public report.

## Verification Semantics

The deterministic CI verification should prove:

- Every official entity category in the selected config has:
  - a generated table in `postgres_schema::schema()`;
  - a row in `sync_entity` after a representative sync;
  - a current row in the typed entity table for non-deleted fixtures;
  - a durable cursor/status row where the runner wrote the page or reached `resume`.
- Every official relation field has:
  - exactly one generated relation table;
  - source and target indexes/foreign-key path from `postgres_schema`;
  - a direct SQL query path by source endpoint and by target endpoint.
- Representative direct HTTP-readiness queries work without XML or Rust-side transformation:
  - fetch a `Document` by `document_nummer` and include enclosure metadata;
  - join `Document` to `Kamerstukdossier` through `document__kamerstukdossier`;
  - list recent registry changes from `sync_entity`;
  - list category sync status from `sync_category`.
- Idempotency is measured through database snapshots before and after reapplying the same pages. Stable evidence should include per-table row counts and selected checksums for relation/current tables.
- Update replacement is measured by applying a changed fixture page and proving only current scalar/relation rows from the latest page remain.
- Storage measurement comes from PostgreSQL catalog functions where available:
  - total table bytes from `pg_total_relation_size` minus indexes where useful;
  - index bytes from `pg_indexes_size`;
  - HTML asset bytes from any stored HTML/text asset columns if present, otherwise `0` with explicit report evidence;
  - linked binary asset metadata rows from download/entity metadata such as `enclosure_url`, `content_type`, and `content_length`.

The live verification should be an ignored `make test-long` lane test that:

- uses the real default SyncFeed base URL unless overridden;
- runs controlled concurrency through `SyncFeedClientConfig.max_concurrent_requests`;
- uses a small explicit category set by default, for example `Document` and `Zaak`, while allowing `OPENTK_LIVE_SYNC_CATEGORIES` to widen it;
- runs the real runner to `resume` or a documented representative full-model bounded run if the full live feed is too large for the test environment;
- writes into a migrated PostgreSQL schema and then calls `verify_sync_database`;
- fails clearly if a required database URL is missing, because skipped tests give false confidence.

## TDD Execution Plan

Use vertical red-green cycles. For each RED, add one failing public behavior test, run the narrow test command, implement only enough production code to pass, then continue.

- [ ] RED 1: Add a deterministic PostgreSQL integration test that seeds one representative page through `write_sync_page`, calls `verify_sync_database`, and expects a `Document` category report plus direct `Document` by number query evidence. Confirm failure because `sync_verification` does not exist.
- [ ] GREEN 1: Add `opentk-db::sync_verification` with category table lookup, current/registry row counts, status reads, and the first direct query evidence.
- [ ] RED 2: Extend the verification test to iterate `official_schema::entity_types()` and assert every official entity has schema coverage and a typed-table verification entry after representative minimal pages are written. Confirm failure before full official coverage reporting exists.
- [ ] GREEN 2: Derive selected categories from `official_schema` and `postgres_schema`; write category verification for all modeled entities without hard-coded category lists.
- [ ] RED 3: Add relation coverage assertions that every official relation has one report entry and that representative relation tables can be queried by source and target endpoint. Confirm failure before relation-table verification exists.
- [ ] GREEN 3: Add relation-table verification using schema-derived relation tables, direct `SELECT ... WHERE source_category/source_id` and `SELECT ... WHERE target_category/target_id` probes, and schema/index evidence.
- [ ] RED 4: Add an idempotency integration test that applies the same representative sync pages twice, snapshots table counts/checksums through the public verification API, and asserts the second run does not duplicate current rows, relation rows, registry rows, or cursor state. Confirm failure before snapshot/idempotency reporting exists.
- [ ] GREEN 4: Add deterministic snapshot helpers inside `sync_verification` and keep the public evidence in `SyncVerificationReport`.
- [ ] RED 5: Add an update replacement integration test that applies an original `Document` page, then a changed page, and expects verification/direct SQL to show only the latest scalar and relation rows. Confirm failure before update evidence is part of verification.
- [ ] GREEN 5: Add update replacement proof via direct SQL evidence and keep the writer unchanged unless the test reveals an actual replacement bug.
- [ ] RED 6: Add storage measurement assertions for table bytes, index bytes, HTML asset bytes, and linked binary asset metadata rows. Confirm failure before storage reporting exists.
- [ ] GREEN 6: Query PostgreSQL catalog sizes and download metadata counts. If there is no HTML asset storage in the current schema, report `html_asset_bytes = 0` explicitly and document that Story 2 currently stores linked binary metadata, not fetched binary bodies.
- [ ] RED 7: Add an ignored live smoke test in the `make test-long` lane that runs selected official categories through the real SyncFeed with controlled concurrency, reaches `resume` or the configured representative bound, and then calls `verify_sync_database`. Confirm failure before test-long/live wiring exists.
- [ ] GREEN 7: Add the ignored live smoke test and any small runner hooks needed to keep the run bounded and explicit. Do not swallow missing database URL or live API errors.
- [ ] RED 8: Add CLI or command-level test coverage for a `complete-sync verify` command if verification is exposed operationally. Confirm failure before the CLI command exists.
- [ ] GREEN 8: Add a thin CLI `verify` command that connects to PostgreSQL, calls `verify_sync_database`, and prints stable report lines or JSON. Keep all real verification logic in `opentk-db::sync_verification`.
- [ ] REFACTOR: Use the improve-code-boundaries review. Remove duplicate category/relation loops, private-test-only `pub` fields, CLI-owned report logic, and any copied official schema facts. Keep exactly one verification report shape and one schema-to-SQL naming boundary.

## Test Commands During Execution

Use narrow commands during red-green cycles, then run the required gates:

- `cargo test -p opentk-db --test sync_verification`
- `cargo test -p opentk-db --bin complete-sync`
- `cargo test -p opentk-sync --test complete_sync_live -- --ignored` only for the live lane once it exists
- `make check`
- `make lint`
- `make test`
- `make test-long`

This is the Story 2 finishing verification task and explicitly requires live API smoke tests separated into `make test-long`, so `make test-long` is allowed here as the story-end validation gate.

## Documentation

Update focused docs after implementation:

- Add `docs/complete-sync-verification.md` describing deterministic verification, live smoke controls, storage metrics, direct SQL proof queries, and how to interpret the report.
- Update `docs/postgres-schema.md` only if verification exposes a schema expectation not already documented.
- Update `docs/syncfeed-client.md` only if live bounded verification changes runner/client semantics.

## Completion Steps

After all checks pass:

- tick completed acceptance criteria in `task-6-deep-verify-complete-sync.md`;
- set `<passes>true</passes>`;
- run `/bin/bash .ralph/task_switch.sh`;
- add and commit all changed files, including `.ralph` files, with `task finished task-6-deep-verify-complete-sync: ...`;
- include verification evidence in the commit message;
- push;
- quit immediately.

NOW EXECUTE
