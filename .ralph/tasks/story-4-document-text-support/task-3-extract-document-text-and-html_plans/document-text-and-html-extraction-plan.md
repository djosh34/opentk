# Story 4 Task 3 Plan: Extract Document Text and HTML

## Current State

- Story 4 Task 1 added `document_asset` and `document_content` storage.
- Story 4 Task 2 added `opentk-sync::document_asset` for HTTP retrieval, content type classification, official text/HTML alternative discovery, SHA-256 hashing, and selected-source metadata.
- Task 2 deliberately kept binary PDF/DOCX reports out of `document_content`; binary rows currently stop at `document_asset` until extraction produces real text/HTML.
- `opentk-db::document_assets::record_document_asset_fetches` already persists official text/HTML/transcript bodies directly into `document_content` with `extraction_tool = 'official-source'`, `extraction_tool_version = '1'`, and `validation_status = 'unverified'`.
- `document_content` currently uses `content_hash` for the selected-source hash and does not store a separate output hash. Task 3 acceptance requires both source hash and output hash, so the schema boundary needs a focused correction instead of overloading one column.
- Existing HTML discovery in `document_asset.rs` uses tiny ad hoc string parsing for `<link rel="alternate">`. Task 3 should not expand that pattern for HTML text extraction.

## Tool And Library Choices

Use source-first official content before extraction:

- If `DocumentAssetFetchReport` has a selected official text/HTML/transcript source with a body, preserve it as the canonical content and do not run binary extraction.
- If the selected source is `Pdf` or `Docx`, run extraction as a fallback.
- If the selected source is unsupported, missing, empty, non-UTF-8 where UTF-8 is required, or mismatches an expected fixture, return an explicit extraction error.

Use small, deterministic extraction adapters:

- PDF: add `pdf-extract` for in-memory PDF text extraction. The current docs.rs release exposes `extract_text_from_mem` and reports crate version `0.10.0`; store `pdf-extract` plus `CARGO_PKG_VERSION`/dependency version string in provenance.
- DOCX: add `zip` with restricted features plus the existing `quick-xml` dependency. Read `word/document.xml` from the package, stream text nodes through `quick_xml::Reader`, and map paragraph/table-cell boundaries to deterministic newlines. This avoids introducing a broad DOCX object model when the required public behavior is plain text extraction.
- HTML: add `scraper` for HTML5 parsing and DOM text traversal. Store normalized HTML as `extracted_html` and deterministic plain text as `extracted_text`. Do not extend the existing string-based link parser for body extraction.

These choices are fixture-gated. A format is considered supported only when exact expected-output fixtures pass; any mismatch records `validation_status = 'invalid'` and an explicit extraction failure instead of accepting approximate text.

## Interface Design

Add a typed extraction boundary in `opentk-sync`:

```rust
pub mod document_content;

pub struct DocumentExtractionConfig {
    pub fixtures: Vec<DocumentExtractionFixture>,
}

pub struct DocumentExtractionFixture {
    pub source_hash: String,
    pub expected_text: Option<String>,
    pub expected_html: Option<String>,
}

pub struct DocumentContentExtractor {
    config: DocumentExtractionConfig,
}

impl DocumentContentExtractor {
    pub fn new(config: DocumentExtractionConfig) -> Self;
    pub fn extract(&self, input: DocumentContentInput) -> DocumentContentExtractionReport;
}
```

Use an owned input type so tests and later orchestration can call the extractor without a database:

```rust
pub struct DocumentContentInput {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub document_asset_id: Option<i64>,
    pub selected_source_url: Url,
    pub selected_source_content_type: Option<String>,
    pub selected_source_content_length: Option<u64>,
    pub selected_source_kind: DocumentAssetKind,
    pub official_source: bool,
    pub source_rank: i32,
    pub source_hash: String,
    pub source_body: Vec<u8>,
}
```

Return a single report shape for official content, extraction success, and extraction failure:

```rust
pub struct DocumentContentExtractionReport {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub document_asset_id: Option<i64>,
    pub selected_source_url: Url,
    pub selected_source_content_type: Option<String>,
    pub selected_source_content_length: Option<u64>,
    pub official_source: bool,
    pub source_rank: i32,
    pub extraction_status: ExtractionStatus,
    pub validation_status: ValidationStatus,
    pub extraction_error: Option<String>,
    pub extraction_tool: String,
    pub extraction_tool_version: String,
    pub source_hash: String,
    pub output_hash: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub extracted_at: DateTime<Utc>,
}

pub enum ExtractionStatus {
    Extracted,
    Empty,
    Failed,
}

pub enum ValidationStatus {
    Unverified,
    Valid,
    Invalid,
}
```

Keep DB writes behind `opentk-db`:

```rust
pub async fn record_document_content_extractions(
    pool: &PgPool,
    reports: &[DocumentContentExtractionReport],
) -> Result<DocumentContentWriteOutcome, DocumentContentWriteError>;
```

The DB layer converts typed status enums to checked SQL strings only at the SQL boundary. It must not know PDF/DOCX/HTML parsing rules.

## Schema Boundary Correction

Task 3 acceptance requires source hash and output hash. Refactor `document_content`:

- Rename current `content_hash` intent to `source_hash`.
- Add `output_hash text`.
- Add `extraction_error text`.
- Change the uniqueness constraint from `(document_asset_id, content_hash)` to `(document_asset_id, source_hash, output_hash)`.
- Update the body check so failure rows are allowed only when `extraction_status = 'failed'` and `extraction_error IS NOT NULL`; successful/valid rows still require `extracted_text IS NOT NULL OR extracted_html IS NOT NULL`.

Because this is greenfield with no backwards compatibility requirement, update the generated schema, migration, repository code, tests, and docs directly. Do not keep compatibility aliases or duplicate columns.

## Normalization Rules

Text normalization is deterministic and shared by every extractor:

- Decode official text and HTML text as UTF-8; non-UTF-8 official text is a failure with explicit error detail.
- Convert CRLF/CR to LF.
- Strip trailing spaces from each line.
- Collapse runs of three or more blank lines to two blank lines.
- Trim leading and trailing whitespace from the full text.
- Preserve meaningful paragraph boundaries as single blank lines for DOCX and HTML; preserve PDF extractor page/text order after normalization.
- Hash output after normalization with SHA-256 over UTF-8 bytes.

HTML normalization:

- Store the original official HTML bytes as UTF-8 if they parse and decode cleanly.
- Extract visible text through the HTML parser, excluding `script`, `style`, `noscript`, template-like hidden nodes, and comments.
- Normalize whitespace using the shared text rules.

DOCX normalization:

- Read only `word/document.xml` for this task.
- Extract text from `w:t` nodes in document order.
- Treat `w:tab` as one space, `w:br` as a line break, `w:p` as a paragraph break, and `w:tc` as a tab or single separating space inside a row.
- Return `Failed` for missing `word/document.xml`, malformed ZIP/XML, encrypted archives, or unsupported compression.

PDF normalization:

- Run `pdf_extract::extract_text_from_mem`.
- Apply only the shared text rules after extraction.
- Return `Failed` for parser errors and `Empty` for normalized empty output.
- Do not silently fix word substitutions or ordering problems. Fixture mismatch is `validation_status = 'invalid'` with error detail.

## Fixture Strategy

Add strict fixtures under `crates/opentk-sync/tests/fixtures/document_content/`:

- `fixture.pdf`
- `fixture.pdf.expected.txt`
- `fixture.docx`
- `fixture.docx.expected.txt`
- `fixture.html`
- `fixture.html.expected.html`
- `fixture.html.expected.txt`

Create small deterministic fixture files in-repo. For PDF, prefer a simple text PDF produced once and checked in as a binary fixture. For DOCX, use a minimal ZIP package with `word/document.xml` so the expected text and extraction rules stay obvious. For HTML, include nested elements, ignored script/style content, entities, and paragraph boundaries.

Tests must fail on exact mismatch by comparing stored/extracted output directly to expected fixture files. No contains-style assertions for golden outputs.

## TDD Execution Plan

Follow vertical red-green cycles. Do not write all tests before implementation.

- [x] RED 1: Add one `opentk-sync` fixture test proving official text content is selected as-is, normalized deterministically, hashed with separate `source_hash` and `output_hash`, and marked `Extracted`/`Valid` when it matches an expected fixture. Confirm it fails because `document_content` extraction does not exist.
- [x] GREEN 1: Add `opentk-sync::document_content`, shared normalization, SHA-256 output hashing, typed statuses, fixture validation, and official text handling.
- [x] RED 2: Add one HTML fixture test proving HTML input stores retrievable HTML and exact extracted plain text while ignoring script/style and preserving paragraph boundaries. Confirm it fails before HTML extraction exists.
- [x] GREEN 2: Add `scraper` dependency and implement HTML decode, storage, visible text extraction, normalization, tool provenance, and fixture validation.
- [x] RED 3: Add one DOCX fixture test proving exact expected text extraction from `word/document.xml`. Confirm it fails before DOCX extraction exists.
- [x] GREEN 3: Add `zip` dependency, implement DOCX ZIP/XML streaming extraction through `quick-xml`, and return explicit failure statuses for missing/malformed document XML.
- [x] RED 4: Add one PDF fixture test proving exact expected text extraction. Confirm it fails before PDF extraction exists.
- [x] GREEN 4: Add `pdf-extract` dependency, implement in-memory PDF extraction adapter, normalize output, and treat parser/empty output as explicit statuses.
- [x] RED 5: Add one validation test proving an expected fixture mismatch does not persist as valid content: report `ExtractionStatus::Failed` or `ValidationStatus::Invalid`, set `extraction_error`, and include source hash/tool metadata. Confirm it fails before mismatch errors are explicit.
- [x] GREEN 5: Make fixture validation strict for every supported format and route mismatches to explicit error reports.
- [x] RED 6: Add an `opentk-db` integration test that records a binary extraction report against a real `document_asset` row and verifies `document_content` stores selected source metadata, `official_source = false`, source hash, output hash, tool name/version, extracted text, timestamps, and `valid` status. Confirm it fails before the repository/schema correction exists.
- [x] GREEN 6: Refactor `postgres_schema.rs`, migrations, `document_assets.rs`, and docs from `content_hash` to `source_hash` plus `output_hash`; add `extraction_error`; add `record_document_content_extractions`.
- [x] RED 7: Add an `opentk-db` integration test proving extraction failures are durable: no swallowed parser/fixture errors, no fake body, explicit `failed`/`invalid` status, `extraction_error`, source hash, tool metadata, and deterministic idempotent upsert on repeated runs.
- [x] GREEN 7: Implement failure-row persistence with database checks that allow bodyless rows only for explicit failures.
- [x] RED 8: Add one end-to-end-ish local fixture test from `DocumentAssetFetchReport` to extraction report proving official text/HTML sources are preferred over binary PDF/DOCX extraction when both are available. Confirm it fails before a helper converts fetched reports into extraction inputs correctly.
- [x] GREEN 8: Add a small conversion/helper that turns fetched reports with selected bodies into extraction inputs while preserving official preference and refusing bodyless binary reports with explicit error.

## Boundary Review

Use the `improve-code-boundaries` skill after the green cycles:

- [x] Keep extraction/parsing in `opentk-sync`; keep SQL persistence in `opentk-db`.
- [x] Keep fixture validation near extraction results, not inside database tests.
- [x] Remove ad hoc HTML text parsing if any new code starts duplicating string-scan logic.
- [x] Ensure `document_content` remains a content/result table, not a pending queue.
- [x] Delete compatibility shims around `content_hash`; use `source_hash`/`output_hash` everywhere after the schema refactor.
- [x] Do not duplicate status enums across crates unless the DB crate needs only a private SQL conversion layer.

## Documentation

Update focused docs after code is green:

- [x] `docs/postgres-schema.md`: describe `source_hash`, `output_hash`, `extraction_error`, failure row constraints, and provenance.
- [x] `docs/storage-model.md`: describe official-source preference, binary fallback extraction, deterministic validation, and exact fixture matching.
- [x] `docs/syncfeed-client.md` only if the public extraction interface needs operator-facing explanation.

## Verification

Run, in order:

- [x] `cargo fmt`
- [x] `make check`
- [x] `make lint`
- [x] `make test`

Do not run `make test-long`; this task is not the story-ending deep verification task and does not explicitly require the long/e2e lane.

After all checks pass:

- [x] Update acceptance boxes in `task-3-extract-document-text-and-html.md`.
- [x] Set `<passes>true</passes>`.
- [x] Run `/bin/bash .ralph/task_switch.sh`.
- [x] Add all files, including `.ralph` updates, commit with `task finished task-3-extract-document-text-and-html: ...`, push, and quit immediately.

NOW EXECUTE
