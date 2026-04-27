# Plan: Parallel Complete Sync Runner

## Sources

- Task file: `.ralph/tasks/story-02-complete-sync-postgres/task-05-implement-parallel-sync-runner.md`
- Story 02 task 03 implementation: `opentk-sync::syncfeed` owns HTTP, cursor validation, retry, concurrency limiting, and adaptive pacing.
- Story 02 task 04 implementation: `opentk-sync::payload` parses entries and `opentk-db::sync_writer` writes one page and advances the cursor in one PostgreSQL transaction.
- `tdd` skill: execute with vertical red-green tracer bullets through public behavior; one failing behavior test, minimum implementation, then repeat.
- `improve-code-boundaries` skill: keep orchestration, storage, and CLI boundaries narrow; remove duplicate DTOs, stringly state, and bootstrap spaghetti.

## Boundary Design

Add the runner as the high-level ingestion boundary, but keep existing deep modules intact:

- `opentk-sync::runner` coordinates category workers, page fetches, payload parsing, page writes, restart behavior, and status snapshots.
- `opentk-db::sync_state` owns durable PostgreSQL runner state: cursor reads, caught-up metadata, error persistence, and status queries.
- `opentk-db::sync_writer` remains the only module that writes entity tables and advances `sync_category.latest_skiptoken` in the page transaction.
- `opentk-sync::syncfeed` remains the only module that constructs and validates upstream cursor URLs.
- The CLI should be thin bootstrap code, not runner logic.

The runner must not use `SyncFeedClient::fetch_category_until_resume` for durable syncing because that helper accumulates pages before writing. Durable syncing needs a streaming loop: fetch one page, parse entries, commit page and cursor atomically, then fetch the next cursor.

## Public Runner Interface

Add `opentk-sync::runner`:

```rust
pub struct CompleteSyncRunner<S> {
    pub client: SyncFeedClient,
    pub store: S,
    pub config: CompleteSyncConfig,
}

pub struct CompleteSyncConfig {
    pub categories: Vec<String>,
    pub mode: SyncRunMode,
    pub poll_interval: Duration,
}

pub enum SyncRunMode {
    UntilCaughtUp,
    Continuous,
}

pub struct CategorySyncReport {
    pub category: String,
    pub pages_written: u64,
    pub entities_seen: u64,
    pub caught_up: bool,
    pub latest_skiptoken: Option<i64>,
}

pub struct CompleteSyncReport {
    pub categories: Vec<CategorySyncReport>,
}

impl<S: SyncStore> CompleteSyncRunner<S> {
    pub async fn run_once(&self) -> Result<CompleteSyncReport, CompleteSyncError>;
    pub async fn run_forever(&self) -> Result<(), CompleteSyncError>;
}
```

Use a small storage trait in `opentk-sync::runner` so runner tests can inject crash points without private database hooks:

```rust
#[async_trait]
pub trait SyncStore: Clone + Send + Sync + 'static {
    async fn load_category_cursor(&self, category: &str) -> Result<Option<StoredCategoryCursor>, SyncStoreError>;
    async fn write_page(&self, page: PreparedSyncPage) -> Result<SyncPageWriteOutcome, SyncStoreError>;
    async fn mark_caught_up(&self, category: &str, resume_url: Url, observed_at: DateTime<Utc>) -> Result<(), SyncStoreError>;
    async fn record_error(&self, error: DurableSyncError) -> Result<(), SyncStoreError>;
    async fn status(&self, categories: &[String]) -> Result<Vec<CategoryStatus>, SyncStoreError>;
}
```

If adding `async-trait` is avoidable with explicit boxed futures, prefer the smaller dependency-free shape. If the trait ergonomics become muddy, add `async-trait` at the workspace level and keep the trait strictly at the runner/storage boundary.

Core shared structs:

```rust
pub struct StoredCategoryCursor {
    pub category: String,
    pub latest_skiptoken: i64,
    pub next_url: Url,
    pub caught_up: bool,
}

pub struct PreparedSyncPage {
    pub category: String,
    pub latest_skiptoken: i64,
    pub atom_updated_at: DateTime<Utc>,
    pub entities: Vec<ParsedEntity>,
}

pub struct CategoryStatus {
    pub category: String,
    pub latest_skiptoken: Option<i64>,
    pub state: CategorySyncState,
    pub lag: Option<Duration>,
    pub last_fetch_at: Option<DateTime<Utc>>,
    pub last_error: Option<DurableSyncError>,
}

pub enum CategorySyncState {
    NotStarted,
    Running,
    CaughtUp,
    Error,
}

pub struct DurableSyncError {
    pub phase: SyncPhase,
    pub category: String,
    pub skiptoken: Option<i64>,
    pub entity_id: Option<Uuid>,
    pub message: String,
}

pub enum SyncPhase {
    Fetch,
    Parse,
    Write,
    MarkCaughtUp,
}
```

During execution, validate whether `StoredCategoryCursor.next_url` is necessary. If the SyncFeed next URL can always be reconstructed from `base_url`, `category`, `content_mode`, and `latest_skiptoken`, remove `next_url` and keep only the numeric cursor plus the validated constructor. Do not persist both if that creates duplicate cursor truth.

## PostgreSQL State Design

Evolve `sync_category` instead of creating a parallel progress table:

- `source_category text primary key`
- `latest_skiptoken bigint not null`
- `last_synced_at timestamptz`
- add `next_url text`
- add `resume_url text`
- add `state text not null default 'not_started'`
- add `last_fetch_at timestamptz`
- add `caught_up_at timestamptz`

Use a small Rust enum for `CategorySyncState`; store it as text through one conversion helper. PostgreSQL enums are not needed for this greenfield code unless the implementation becomes cleaner with them.

Evolve `ingest_error` because the existing table has message/payload but not all task-required fields:

- keep `id`, `source_category`, `source_id`, `latest_skiptoken`, `message`, `payload`, `created_at`
- add `phase text not null`

If the current migration lacks identity/sequence support for `ingest_error.id`, fix it now. Durable error writes must not require the caller to invent ids.

Update `opentk-db::postgres_schema`, migrations, migration tests, and docs together. This is greenfield; no backwards compatibility layer.

## Runner Semantics

For each category worker:

1. Load stored progress.
2. Build the starting cursor:
   - no row or no committed cursor: `SyncFeedCursor::first_page(...)`
   - committed cursor: resume from the stored next URL or reconstructed skiptoken cursor
   - caught-up row in `UntilCaughtUp`: return immediately
3. Fetch exactly one page.
4. If the page has entries:
   - parse each entry payload through `opentk_sync::payload::parse_entry_payload`
   - derive `latest_skiptoken` from the page's `next_request`
   - write entities through `store.write_page(...)`
   - continue with the next cursor only after the store call succeeds
5. If the page is empty with `resume`:
   - mark the category caught up durably through `store.mark_caught_up(...)`
   - return for `UntilCaughtUp`
   - for `Continuous`, sleep `poll_interval`, then fetch from `resume`
6. On fetch/parse/write/mark errors:
   - record a durable `DurableSyncError` with phase, category, skiptoken, entity id when available, and message
   - return the error; do not swallow or retry outside the existing client retry policy

Run category workers concurrently with `tokio::task::JoinSet`. Preserve per-category order by keeping exactly one sequential loop per category. Global HTTP concurrency stays controlled by `SyncFeedClient`'s shared semaphore and adaptive pacing.

Crash semantics:

- Crash before `write_page` commits: no cursor row advances, so the next run refetches the same page. Because writer replacement is idempotent by source identity and cursor, applying the page after restart yields one current row per entity, not duplicates.
- Crash after `write_page` commits but before the next fetch: the stored cursor has advanced in the same transaction as entity writes, so the next run starts from the advanced cursor.
- Crash after caught-up mark: `UntilCaughtUp` reports that category as caught up on restart.

## CLI Design

Add a binary under `crates/opentk-sync/src/bin/complete-sync.rs` or one workspace-level binary if current conventions prefer it.

Commands:

```text
complete-sync run --database-url <url> --base-url <url> --category Document --category Zaak
complete-sync poll --database-url <url> --base-url <url> --poll-interval-seconds 30 [--category ...]
complete-sync status --database-url <url> [--category ...]
```

Defaults:

- `DATABASE_URL` or `OPENTK_DATABASE_URL` may provide `--database-url`.
- default base URL is the official SyncFeed base only in CLI bootstrap, not tests.
- default categories should be all modeled `opentk_core::official_schema::entity_types()` categories.
- status output should be stable table-like text or JSON. Prefer JSON only if it reduces formatting tests; the acceptance criterion only requires category cursor, state, lag, last fetch, and errors.

## TDD Execution Plan

Use vertical red-green cycles. For each RED, add one failing public behavior test, run the narrow test command, then implement only enough production code to pass.

- [x] RED 1: Add a runner integration test with an in-memory `SyncStore` crash hook that fails immediately before `write_page` commits. It should run the same category again and assert the same first page URL is requested again and the entity is applied exactly once through the public store view. Confirm failure because no runner exists.
- [x] GREEN 1: Add `opentk-sync::runner`, storage trait, one-category `run_once`, page parsing, page writing, and explicit error propagation.
- [x] RED 2: Add a runner integration test that lets `write_page` commit and then simulates process restart. Assert the next runner starts from the advanced cursor and does not refetch the committed page. Confirm failure before durable cursor load/start logic exists.
- [x] GREEN 2: Implement stored cursor loading and next-cursor start behavior using validated `SyncFeedCursor` construction.
- [x] RED 3: Add a multi-category local HTTP test proving two categories overlap in time while each category observes ordered cursor requests. Confirm failure before `JoinSet` category workers exist in the runner.
- [x] GREEN 3: Run category workers concurrently while preserving one sequential page loop per category.
- [x] RED 4: Add a caught-up test where an empty page with `resume` marks category state `CaughtUp`, stores resume metadata, and makes the report show `caught_up = true`. Confirm failure before caught-up persistence exists.
- [x] GREEN 4: Implement caught-up transitions in the runner and storage trait.
- [x] RED 5: Add a PostgreSQL-backed `opentk-db::sync_state` integration test proving status reports cursor, state, lag, last fetch, and last error from durable tables. Confirm failure because storage module/schema support does not exist.
- [x] GREEN 5: Evolve `postgres_schema`, migrations, docs, and add `opentk-db::sync_state` public functions/types for cursor load, caught-up mark, durable error write, and status query.
- [x] RED 6: Add a durable error integration test for fetch, parse, and write failures. Assert each record includes phase, category, skiptoken when available, entity id when available, and message. Confirm failure before error recording is complete.
- [x] GREEN 6: Wire error persistence from runner phases and implement `ingest_error.phase`/identity support.
- [x] RED 7: Add CLI tests or command-level integration tests for `run` category selection and `status` output fields. Confirm failure before CLI exists.
- [x] GREEN 7: Add the CLI binary with thin config parsing/bootstrap, status output, category selection, and shared runner construction.
- [x] REFACTOR: Use the improve-code-boundaries review. Remove duplicate cursor/progress shapes, remove any runner code from CLI/bootstrap, keep SQL state transitions in `opentk-db::sync_state`, and ensure there is no second page writer hidden in the runner.

## Test Strategy

- Runner orchestration tests can use a local mock HTTP server plus an in-memory `SyncStore` to simulate crash points precisely.
- PostgreSQL behavior belongs in `opentk-db` integration tests through public `sync_state` and `sync_writer` APIs. Missing `OPENTK_TEST_DATABASE_URL` or `DATABASE_URL` must fail clearly.
- CLI tests should not hit the live SyncFeed. Use local base URLs and test databases.
- Do not test `.ralph/` files.
- Do not run `make test-long`; task 06 is the story-ending deep verification task.

## Documentation

Update focused docs after implementation:

- `docs/syncfeed-client.md` or a new `docs/complete-sync-runner.md` for runner restart semantics, category concurrency, caught-up/polling behavior, and status/error fields.
- `docs/postgres-schema.md` for the evolved `sync_category` and `ingest_error` semantics.
- `README.md` only if the CLI becomes user-facing enough to need a short command example.

## Verification

Run narrow tests after each red-green cycle. Before completion, run in order:

- `cargo fmt`
- `make check`
- `make lint`
- `make test`

Do not run `make test-long`; this is not the story-ending task and the task does not explicitly require long/e2e validation.

After all checks pass:

- update every completed acceptance criterion in `task-05-implement-parallel-sync-runner.md`;
- set `<passes>true</passes>`;
- run `/bin/bash .ralph/task_switch.sh`;
- add and commit all changed files, including `.ralph` files, with `task finished task-05-implement-parallel-sync-runner: ...`;
- push;
- quit immediately.

NOW EXECUTE
