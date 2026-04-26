# Story 4 Task 5 Plan: Deep Verify Document Content

## Current State

- Story 4 Task 2 added `opentk-sync::document_asset` for controlled HTTP fetching, content-type classification, official text/HTML alternative selection, retry status reporting, source hashes, and selected bodies.
- Story 4 Task 3 added `opentk-sync::document_content` with strict fixture validation for official text, HTML, DOCX, and PDF plus `opentk-db::document_assets::record_document_content_extractions`.
- Story 4 Task 4 added `GET /documents/{source_id}/content`, backed by `opentk-db::read_model::get_document_content`.
- Existing extractor tests verify individual format behavior, and existing API tests verify manually seeded content rows. What is missing is one end-to-end fixture path that fetches, classifies, extracts, stores, reads from PostgreSQL, and serves through HTTP with exact body comparisons.
- `opentk-db::sync_verification::StorageVerification` currently reports only total table/index bytes, `html_asset_bytes = 0`, and binary metadata rows. Task 5 requires separated storage evidence for links/metadata, extracted text, stored HTML, and indexes/constraints.
- `crates/opentk-db/tests/sync_verification.rs` has an ignored live SyncFeed smoke under the long lane, but no live controlled document-content provenance/hash sample.

## Public Interfaces

Keep production APIs narrow. Do not add new HTTP endpoints.

Strengthen the existing verification boundary in `opentk-db::sync_verification`:

```rust
pub struct StorageVerification {
    pub table_bytes: i64,
    pub index_bytes: i64,
    pub link_metadata_bytes: i64,
    pub extracted_text_bytes: i64,
    pub stored_html_bytes: i64,
    pub constraint_count: i64,
    pub binary_asset_metadata_rows: i64,
    pub document_content_rows: i64,
}
```

Add document-content evidence to the same crate because it owns PostgreSQL verification:

```rust
pub struct DocumentContentVerification {
    pub document_source_id: Uuid,
    pub selected_source_url: String,
    pub upstream_content_type: Option<String>,
    pub selected_source_content_type: Option<String>,
    pub official_source: bool,
    pub source_rank: i32,
    pub extraction_status: String,
    pub validation_status: String,
    pub extraction_tool: String,
    pub extraction_tool_version: String,
    pub source_hash: String,
    pub output_hash: Option<String>,
    pub extracted_text_bytes: i64,
    pub extracted_html_bytes: i64,
}

pub async fn verify_document_content(
    pool: &PgPool,
    document_source_id: Uuid,
) -> Result<DocumentContentVerification, SyncVerificationError>;
```

The verification helper is deliberately read-only and row-backed. It must not parse PDF/DOCX/HTML, normalize bodies, or know fixture extraction rules.

## Fixture Pipeline

Add one shared test-support pipeline for Story 4 deep verification:

```rust
struct DeepDocumentFixture {
    document_source_id: Uuid,
    asset_path: &'static str,
    asset_content_type: &'static str,
    official_alternative_path: Option<&'static str>,
    expected_text: Option<&'static str>,
    expected_html: Option<&'static str>,
}
```

Use the existing fixture assets:

- `crates/opentk-sync/tests/fixtures/document_content/fixture.pdf`
- `crates/opentk-sync/tests/fixtures/document_content/fixture.pdf.expected.txt`
- `crates/opentk-sync/tests/fixtures/document_content/fixture.docx`
- `crates/opentk-sync/tests/fixtures/document_content/fixture.docx.expected.txt`
- `crates/opentk-sync/tests/fixtures/document_content/fixture.html`
- `crates/opentk-sync/tests/fixtures/document_content/fixture.html.expected.html`
- `crates/opentk-sync/tests/fixtures/document_content/fixture.html.expected.txt`

The deep fixture path should:

1. Seed a real `document` row.
2. Serve fixture assets from the local controlled test server used by `document_asset_fetcher`.
3. Fetch through `DocumentAssetFetcher`.
4. Convert the fetched report with `DocumentContentExtractionInput::from_fetch_report`.
5. Extract through `DocumentContentExtractor` configured with exact fixture expectations.
6. Persist asset and content rows through `record_document_asset_fetches` and `record_document_content_extractions`.
7. Assert PostgreSQL row values and stored bodies exactly.
8. Assert `GET /documents/{source_id}/content` returns the same text, HTML, source hashes, output hashes, provenance, and selected-source metadata as the PostgreSQL row.

For official-source preference, use an HTML landing page that links to an official text alternative and a PDF link. The selected source must be the official text/HTML alternative, `official_source = true`, `source_rank = 0`, and the stored body must match the official expected output, not the binary fallback.

## TDD Execution Plan

Follow vertical red-green cycles. Do not write all tests before implementation.

- [x] RED 1: Add one `opentk-sync` test proving fixture validation fails on a one-word mismatch for an existing supported fixture, using exact comparison and asserting `ExtractionStatus::Failed`, `ValidationStatus::Invalid`, and a non-empty `extraction_error`. Confirm it fails if mismatch handling is loosened.
- [x] GREEN 1: Tighten only the minimum extractor validation code needed if the new test exposes a gap; otherwise keep the production extractor unchanged and keep the test as regression coverage.
- [x] RED 2: Add one end-to-end fixture test in `opentk-api/tests/cases/deep_verify_http_api.rs` for the official-source preference path: controlled local HTTP fetch, selected official text/HTML source, extraction, PostgreSQL persistence, HTTP response, and exact row/body comparison. Confirm it fails before shared fixture orchestration exists.
- [x] GREEN 2: Add narrow deep-test support helpers for seeding documents, serving fixtures, recording fetch reports and extraction reports, and comparing API JSON to database rows. Keep helpers in tests unless production code needs the behavior.
- [x] RED 3: Add one PDF deep fixture test using the same public path: fetch, classify, extract, store, verify PostgreSQL row, and verify `GET /documents/{source_id}/content` returns `fixture.pdf.expected.txt` exactly. Confirm it fails until the helper handles binary fixture reports and extraction persistence.
- [x] GREEN 3: Complete the PDF fixture path without adding PDF-specific SQL or HTTP behavior outside existing extraction boundaries.
- [x] RED 4: Add one DOCX deep fixture test with exact `fixture.docx.expected.txt` storage and HTTP response comparison. Confirm it fails before the helper handles DOCX content-type and selected-source metadata correctly.
- [x] GREEN 4: Complete the DOCX path with the existing `DocumentContentExtractor`; keep DOCX parsing out of DB and API tests.
- [x] RED 5: Add one HTML fixture deep test proving stored HTML and extracted text match `fixture.html.expected.html` and `fixture.html.expected.txt` exactly through PostgreSQL and HTTP. Confirm it fails before exact HTML body comparison is wired.
- [x] GREEN 5: Complete HTML fixture orchestration and exact row/API comparison.
- [x] RED 6: Add retry/error behavior coverage through the same controlled HTTP server: retryable 503 exhausts configured retries, persists an explicit failed asset report, and does not create fake `document_content`; missing content returns `document_content_not_found`. Confirm it fails before persistence/assertion helper covers failure rows.
- [x] GREEN 6: Extend only test support or existing persistence code needed to preserve explicit failed retrieval state with no swallowed errors.
- [x] RED 7: Add `opentk-db` verification tests requiring `StorageVerification` to separately report link/metadata bytes, extracted text bytes, stored HTML bytes, index bytes, constraint count, binary metadata rows, and document content rows. Confirm existing `html_asset_bytes = 0` style reporting fails this test.
- [x] GREEN 7: Refactor `StorageVerification` to compute separated values from PostgreSQL catalog and `document_asset`/`document_content` columns. Remove the muddy `html_asset_bytes` field instead of keeping compatibility aliases.
- [x] RED 8: Add a read-only `verify_document_content` DB test proving API-relevant PostgreSQL provenance can be reported for a stored fixture row. Confirm it fails before the helper exists.
- [x] GREEN 8: Implement `verify_document_content` in `opentk-db::sync_verification`, keeping it row/provenance focused and independent from extraction implementation.
- [x] RED 9: Add an ignored `make test-long` live controlled document-content smoke test. It should fetch one configured official document asset URL, record selected-source URL, upstream/selected content types, source/output hashes, extraction tool/version, official-source flag, and byte counts. Confirm it is ignored by default `make test` and selected by `make test-long`.
- [x] GREEN 9: Implement the ignored long-lane test using environment variables for the live URL and expected category/id when provided, with a deterministic local fallback only if no live variables are set. Do not require long-lane execution during this normal task unless the task is being finished as the entire story validation gate.

## Boundary Review

Use the `improve-code-boundaries` skill before final verification:

- [x] Keep fetch/classification/retry behavior in `opentk-sync::document_asset`.
- [x] Keep PDF/DOCX/HTML extraction and exact fixture validation in `opentk-sync::document_content`.
- [x] Keep SQL persistence and storage/provenance verification in `opentk-db`.
- [x] Keep HTTP JSON shape checks in `opentk-api`; do not expose DB row structs as API DTOs.
- [x] Delete any compatibility fields introduced during this task, especially the old `html_asset_bytes` aggregate if storage reporting is split.
- [x] Keep deep verification helpers in test modules unless they are genuine production read boundaries.
- [x] If exact fixture tests require changing public types/enums or the row contract, switch this plan back to `TO BE VERIFIED` and stop.

## Documentation

Update focused docs after code is green:

- [x] `docs/complete-sync-verification.md`: describe document-content fixture verification, long-lane live provenance, and exact output comparison.
- [x] `docs/storage-model.md`: describe separated storage measurement for document links/metadata, extracted text, stored HTML, and indexes/constraints if the storage report changed.

## Verification

Run, in order:

- [x] `cargo fmt`
- [x] `make check`
- [x] `make lint`
- [x] `make test`

Do not run `make test-long` for this normal task unless finishing the entire Story 4 validation gate or unless the final task instructions explicitly require story-end validation at that point. The ignored long-lane test must still be present and selectable by `make test-long`.

After all required checks pass:

- [x] Update acceptance boxes in `task-5-deep-verify-document-content.md`.
- [x] Set `<passes>true</passes>`.
- [x] Run `/bin/bash .ralph/task_switch.sh`.
- [x] Add all files, including `.ralph` updates, commit with `task finished task-5-deep-verify-document-content: ...`, push, and quit immediately.

NOW EXECUTE
