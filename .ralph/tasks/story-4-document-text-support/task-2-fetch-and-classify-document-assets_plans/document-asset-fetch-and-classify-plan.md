# Story 4 Task 2 Plan: Fetch and Classify Document Assets

## Current State

- Task 1 already added the storage schema for `document_asset` and `document_content`.
- Execution verification found that `document_content` currently mixes selected
  source bookkeeping with actual persisted content. Its
  `document_content_extracted_body_check` requires `extracted_text IS NOT NULL OR
  extracted_html IS NOT NULL`, which correctly rejects bodyless rows but exposed
  a bad plan: this task must not insert pending metadata rows into
  `document_content`.
- Boundary correction: `document_asset` owns retrieval attempts, upstream link
  metadata, binary input classification, failures, and retry state.
  `document_content` owns only real persisted official/extracted text or HTML
  bodies. A PDF/DOCX with no official text/HTML alternative creates only a
  `document_asset` row in this task; Task 3 can create `document_content` after
  extraction produces actual content.
- `Document` rows already retain official sync metadata and `downloadEntiteitType` fields: `content_type`, `content_length`, and `enclosure_url`.
- `opentk-sync` owns HTTP client behavior for Tweede Kamer syncfeed fetching, retry, timeout, and bounded concurrency.
- `opentk-db` owns PostgreSQL writes and already depends on `opentk-sync`, so durable asset retrieval recording can live there without introducing a dependency cycle.
- Existing tests use local TCP fixture servers instead of external HTTP and run PostgreSQL migrations through `scripts/cargo-test-with-postgres.sh`.

## Interface Design

Keep the HTTP fetch/classification boundary in `opentk-sync`:

```rust
pub mod document_asset;

pub struct DocumentAssetFetchConfig {
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub max_retries: u32,
    pub initial_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub max_concurrent_requests: NonZeroUsize,
    pub max_asset_bytes: u64,
}

pub struct DocumentAssetFetcher { ... }

impl DocumentAssetFetcher {
    pub fn new(config: DocumentAssetFetchConfig) -> Result<Self, DocumentAssetFetchError>;
    pub async fn fetch_one(&self, request: DocumentAssetFetchRequest)
        -> DocumentAssetFetchReport;
    pub async fn fetch_many<I>(&self, requests: I)
        -> Vec<DocumentAssetFetchReport>
    where
        I: IntoIterator<Item = DocumentAssetFetchRequest>;
}
```

Use owned request/response types at this boundary:

```rust
pub struct DocumentAssetFetchRequest {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub asset_url: Url,
    pub expected_content_type: Option<String>,
    pub expected_content_length: Option<u64>,
}

pub struct DocumentAssetFetchReport {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub asset_url: Url,
    pub upstream_url: Url,
    pub upstream_content_type: Option<String>,
    pub upstream_content_length: Option<u64>,
    pub upstream_last_modified_at: Option<DateTime<Utc>>,
    pub retrieval_status: RetrievalStatus,
    pub retrieval_error: Option<String>,
    pub retrieved_at: DateTime<Utc>,
    pub content_hash: Option<String>,
    pub selected_source: Option<DocumentSelectedSource>,
    pub discovered_sources: Vec<DocumentDiscoveredSource>,
}
```

Model classification with enums, not string matching spread through call sites:

```rust
pub enum RetrievalStatus {
    Fetched,
    NotFound,
    UnsupportedContentType,
    Failed,
}

pub enum DocumentAssetKind {
    OfficialText,
    OfficialHtml,
    OfficialTranscript,
    Pdf,
    Docx,
}

pub struct DocumentSelectedSource {
    pub url: Url,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub kind: DocumentAssetKind,
    pub official_source: bool,
    pub source_rank: i32,
}
```

Keep durable writes in `opentk-db` behind a small repository/service boundary:

```rust
pub mod document_assets;

pub struct DocumentAssetWriteOutcome {
    pub assets_recorded: usize,
    pub contents_recorded: usize,
    pub failures_recorded: usize,
}

pub async fn record_document_asset_fetches(
    pool: &PgPool,
    reports: &[DocumentAssetFetchReport],
) -> Result<DocumentAssetWriteOutcome, DocumentAssetWriteError>;
```

The database layer will convert typed enums to checked `text` states at the SQL
boundary only. It must not duplicate HTTP classification rules. It must also
keep bodyless binary/pending extraction state out of `document_content`; the
presence of a `document_content` row means searchable text or HTML exists.

## Classification Rules

- Supported persisted official content:
  - `text/plain`
  - `text/html`
  - `application/xhtml+xml`
  - known official transcript/text links discovered from HTML using `rel=alternate`, anchor text, or URL/content-type hints.
- Supported binary extraction inputs:
  - `application/pdf`
  - DOCX media type `application/vnd.openxmlformats-officedocument.wordprocessingml.document`
- Unsupported content types produce a durable `unsupported_content_type` report and do not create `document_content`.
- HTTP `404`/`410` produce `not_found`; other non-success responses and transport/retry exhaustion produce `failed`.
- Content length validation:
  - If HTTP `Content-Length` is present, compare it with bytes actually read.
  - If the request has `expected_content_length`, compare it with the HTTP length when present and with actual bytes read.
  - Wrong length is `failed` with a retryable error message.
  - Missing length is allowed and recorded as `None`.
- Hash validation:
  - Compute SHA-256 over the selected source bytes and store it in `content_hash` when a source is fetched successfully.
  - If the same asset/content hash is recorded twice, use the existing `(document_asset_id, content_hash)` uniqueness as idempotency rather than swallowing the database error.
- Official source preference:
  - Fetch the upstream asset.
  - If the upstream response is official text/HTML/transcript, select it directly.
  - If upstream is HTML and advertises official text/HTML/transcript alternatives, fetch candidates and select the best official source by rank.
  - If upstream is PDF/DOCX and no official text/HTML/transcript alternative is discovered, select the binary as an extraction input but do not persist extracted body content in this task.
  - Task 3 will implement extraction, so this task records binary-only selected
    source metadata on the fetched `document_asset` report and persisted
    `document_asset` row only. Do not insert `document_content` for binary-only
    fetches.
  - Official text/HTML/transcript alternatives are already content, not
    extraction inputs. Persist their real body into `document_content`
    (`extracted_text` for text/transcript and `extracted_html` for HTML/XHTML)
    with the selected source metadata and hash. This is acceptable even though
    the current column names say `extracted_*`; during execution, improve this
    boundary if the schema refactor is small enough, otherwise leave a focused
    follow-up only if renaming would distract from the task.

## TDD Execution Plan

Follow vertical red-green cycles. Do not write all tests before implementation.

- [x] RED 1: Add one `opentk-sync` fixture-server test proving an HTML upstream asset that links an official text alternative is fetched, classified as official text, preferred over the original HTML/binary input, and returns upstream URL, selected URL, content type, length, SHA-256 hash, and `Fetched`. Confirm it fails because `document_asset` fetcher does not exist.
- [x] GREEN 1: Add `crates/opentk-sync/src/document_asset.rs`, export it from `lib.rs`, add any focused dependencies required for SHA-256/HTML link parsing, and implement the minimal fetch/classification path for HTML plus official text alternative discovery.
- [x] RED 2: Add the next fixture-server test proving PDF and DOCX assets are accepted as binary extraction inputs when no official text/HTML/transcript source exists, with content type, optional length, selected kind, and hash reported. Confirm it fails before binary classification exists.
- [x] GREEN 2: Add PDF/DOCX classification and selected-source ranking without adding extraction logic.
- [x] RED 3: Add one fixture-server test for missing content length and wrong length: missing length succeeds and records `None`; wrong length returns `Failed` with explicit error detail. Confirm it fails before length validation is complete.
- [x] GREEN 3: Implement content-length parsing and validation in one place inside `DocumentAssetFetcher`.
- [x] RED 4: Add one fixture-server test for unsupported content type and HTTP error responses: unsupported media type maps to `UnsupportedContentType`, `404` maps to `NotFound`, and `500`/transport retry exhaustion maps to `Failed`. Confirm it fails before error classification exists.
- [x] GREEN 4: Implement retry/status mapping and ensure errors are carried as data in `DocumentAssetFetchReport` rather than swallowed.
- [x] RED 5: Add one concurrency/timeout fixture-server test proving `fetch_many` honors `max_concurrent_requests` and request timeout. Confirm it fails before bounded concurrency is wired.
- [x] GREEN 5: Reuse the syncfeed-style semaphore/retry pattern or extract a tiny shared helper only if that removes real duplication. Keep the public fetcher interface small.
- [x] RED 6: Add an `opentk-db` integration test that writes a real `Document` row, records successful fetch reports, and verifies `document_asset` rows store upstream URL, content metadata, status, error, and timestamps. Confirm it fails because the repository does not exist.
- [x] GREEN 6: Add `crates/opentk-db/src/document_assets.rs`, export it from `lib.rs`, and implement transactional upserts into `document_asset`.
- [x] RED 7: Add an `opentk-db` integration test for selected official text/HTML
      source recording: successful official source reports create
      `document_content` rows with selected URL/content type/length,
      `official_source`, `source_rank`, `content_hash`, the real fetched body in
      `extracted_text` or `extracted_html`, `extraction_status = 'extracted'`,
      and `validation_status = 'unverified'`. Confirm it fails before content
      recording exists.
- [x] GREEN 7: Insert real official text/HTML content into `document_content`.
      Do not insert a `document_content` row for binary-only PDF/DOCX reports.
      If implementation reveals that the `extracted_*` names are causing muddy
      call-site conversions, refactor the schema to content-neutral names in the
      generated schema and migration in the same TDD cycle.
- [x] RED 8: Add an `opentk-db` integration test proving failure reports are durable and retryable: wrong length/unsupported/not found/HTTP failure update the same asset row with status and error detail, and do not create fake `document_content` rows. Confirm it fails before failure persistence exists.
- [x] GREEN 8: Implement status/error updates idempotently by `(document_source_category, document_source_id, asset_url)`.

## Boundary Review

Use the `improve-code-boundaries` skill after the green cycles:

- [x] Keep HTTP concerns, response headers, retry, and content sniffing out of `opentk-db`.
- [x] Keep SQL string/status conversion inside `opentk-db::document_assets`; do not expose database status strings as the sync crate API.
- [x] Do not let `document_content` become a queue table. If no real text/HTML body exists, store only `document_asset` retrieval state and leave extraction to Task 3.
- [x] Remove any duplicate `DocumentAssetKind`/status shapes if they appear in both crates.
- [x] If the syncfeed client and document asset fetcher grow duplicated retry/concurrency code, extract a private shared HTTP helper in `opentk-sync`; do not create a generic framework unless it clearly simplifies both call sites.
- [x] Delete dead/legacy wording or code that implies binary files are the persisted final content. This is greenfield; no backwards compatibility is required.

## Documentation

Update only focused docs that become stale:

- [x] `docs/storage-model.md`: describe how fetched asset rows and selected official source rows are populated.
- [x] `docs/postgres-schema.md`: clarify which task creates `document_content` rows and how pending extraction is represented.
- [x] `docs/syncfeed-client.md` or a focused new doc only if a public fetcher interface needs operator-facing explanation.

## Verification

Run, in order:

- [x] `cargo fmt`
- [x] `make check`
- [x] `make lint`
- [x] `make test`

Do not run `make test-long`; this task is not the story-ending deep verification task and does not explicitly require the long/e2e lane.

After all checks pass:

- [x] Update acceptance boxes in `task-2-fetch-and-classify-document-assets.md`.
- [x] Set `<passes>true</passes>`.
- [x] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Add all files, including `.ralph` updates, commit with `task finished task-2-fetch-and-classify-document-assets: ...`, push, and quit immediately.

NOW EXECUTE
