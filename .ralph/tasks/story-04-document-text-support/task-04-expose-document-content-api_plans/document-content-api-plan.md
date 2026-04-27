# Story 04 Task 04 Plan: Expose Document Content API

## Current State

- Story 04 Task 01 added `document_asset` and `document_content` PostgreSQL storage.
- Story 04 Task 02 records fetched document assets and persists official text/HTML sources directly into `document_content`.
- Story 04 Task 03 added extraction reports, `source_hash`, `output_hash`, `extraction_error`, exact fixture validation, and `record_document_content_extractions`.
- The HTTP API currently exposes generic entity detail through `/entities/{category}/{source_id}` and typed entity detail through `/documents/{source_id}`, `/activities/{source_id}`, and `/persons/{source_id}`.
- `opentk-db::read_model` has no document-content read boundary, and `opentk-api` has no content-specific response model or OpenAPI schema.

## Public Interface

Add one typed document content endpoint:

```text
GET /documents/{source_id}/content
```

The response is intentionally content-specific instead of overloading `EntityDetailResponse`:

```json
{
  "document": {
    "source_category": "Document",
    "source_id": "11111111-1111-4111-8111-111111111111"
  },
  "asset": {
    "id": 10,
    "asset_url": "https://example.test/document.pdf",
    "upstream_url": "https://example.test/document.pdf",
    "upstream_content_type": "application/pdf",
    "upstream_content_length": 12345,
    "retrieval_status": "fetched",
    "retrieval_error": null,
    "retrieved_at": "2026-04-26T12:02:00+00:00"
  },
  "content": {
    "id": 20,
    "selected_source_url": "https://example.test/document.pdf",
    "selected_source_content_type": "application/pdf",
    "selected_source_content_length": 12345,
    "official_source": false,
    "source_rank": 20,
    "extraction_status": "extracted",
    "validation_status": "valid",
    "extraction_tool": "pdf-extract",
    "extraction_tool_version": "0.10.0",
    "source_hash": "sha256-source",
    "output_hash": "sha256-output",
    "extraction_error": null,
    "extracted_text": "Stored document text",
    "extracted_html": null,
    "extracted_at": "2026-04-26T12:03:00+00:00"
  }
}
```

Use clear status semantics:

- `200 OK`: a `document_content` row exists for the document, including `failed` or `empty` rows. Failed extraction is not an HTTP transport failure; it is durable content state.
- `404 not_found`: no `document` row exists for the UUID.
- `404 document_content_not_found`: the document exists but no `document_content` row exists yet.
- `400 invalid_request`: malformed UUID.

The response must include both `extracted_text` and `extracted_html` as nullable fields. HTML assets therefore return stored HTML where available without requiring a second endpoint.

## Database Read Boundary

Add typed read-model structs in `opentk-db::read_model`:

```rust
pub struct DocumentContentDetail {
    pub document_source_category: String,
    pub document_source_id: Uuid,
    pub asset: DocumentAssetDetail,
    pub content: DocumentContentRow,
}

pub struct DocumentAssetDetail {
    pub id: i64,
    pub asset_url: String,
    pub upstream_url: String,
    pub upstream_content_type: Option<String>,
    pub upstream_content_length: Option<i64>,
    pub upstream_last_modified_at: Option<String>,
    pub retrieval_status: String,
    pub retrieval_error: Option<String>,
    pub retrieved_at: Option<String>,
}

pub struct DocumentContentRow {
    pub id: i64,
    pub selected_source_url: String,
    pub selected_source_content_type: Option<String>,
    pub selected_source_content_length: Option<i64>,
    pub official_source: bool,
    pub source_rank: i32,
    pub extraction_status: String,
    pub validation_status: String,
    pub extraction_tool: String,
    pub extraction_tool_version: String,
    pub source_hash: String,
    pub output_hash: Option<String>,
    pub extraction_error: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub extracted_at: String,
}
```

Add:

```rust
pub async fn get_document_content(
    pool: &PgPool,
    source_id: Uuid,
) -> Result<DocumentContentDetail, ReadModelError>;
```

Selection rule:

- Join `document` -> `document_content` -> `document_asset`.
- Filter by `document.source_category = 'Document'` and `document.source_id = $1`.
- Prefer official content first, then lower `source_rank`, then newest `extracted_at`, then highest `document_content.id`.
- Return `ReadModelError::NotFound` only when the document row itself is missing.
- Add `ReadModelError::DocumentContentNotFound` for existing documents without content.

This keeps SQL ownership in `opentk-db` and keeps HTTP serialization in `opentk-api`.

## API And OpenAPI Boundary

In `opentk-api`:

- Add `.route("/documents/{source_id}/content", get(document_content))`.
- Map `ReadModelError::DocumentContentNotFound` to HTTP `404` with `code = "document_content_not_found"` and message `document content not found`.
- Keep existing `not_found` handling for missing document rows and unknown categories.
- Add `DocumentContentResponse`, `DocumentIdentityResponse`, `DocumentAssetResponse`, and `DocumentContentBodyResponse` with `Serialize` and `ToSchema`.
- Add the new path to the manually built OpenAPI paths.
- Add the new schemas to `api_components`.

Do not make the generic `/documents/{source_id}` detail response include large extracted bodies. Generic document detail remains metadata/fields/relations; the content endpoint carries document text and HTML bodies.

## TDD Execution Plan

Follow vertical red-green cycles. Do not write all tests first.

- [x] RED 1: Add one API integration test in `crates/opentk-api/tests/cases/core_read_endpoints.rs` for `/documents/{source_id}/content` returning an extracted PDF text row with asset metadata, source type/content type, `official_source = false`, `extraction_status = "extracted"`, provenance, hashes, and extracted text. Confirm it fails because the route/read model do not exist.
- [x] GREEN 1: Add `get_document_content`, the API route, response model mapping, and minimal OpenAPI component registration needed for the first response.
- [x] RED 2: Add one API integration test for an official HTML source where `selected_source_content_type = "text/html"`, `official_source = true`, `extracted_html` contains stored HTML, and `extracted_text` contains plain text. Confirm it fails until HTML fields are mapped.
- [x] GREEN 2: Complete response mapping for nullable HTML/text fields and official source metadata.
- [x] RED 3: Add one API integration test proving a failed extraction row returns `200 OK` with `extraction_status = "failed"`, `validation_status = "invalid"`, `extraction_error`, source hash, and no fake extracted body. Confirm it fails until failed content state is deliberately treated as a normal content response.
- [x] GREEN 3: Ensure failed rows are selected and serialized without converting durable extraction failure into an HTTP error.
- [x] RED 4: Add one API integration test proving missing document content for an existing document returns `404` with `code = "document_content_not_found"`, while a missing document UUID still returns existing `not_found`. Confirm it fails before the read-model error distinction exists.
- [x] GREEN 4: Add `ReadModelError::DocumentContentNotFound` and map it at the API boundary.
- [x] RED 5: Extend the OpenAPI test to require `/documents/{source_id}/content` plus `DocumentContentResponse`, `DocumentAssetResponse`, and `DocumentContentBodyResponse` schemas. Confirm it fails before the OpenAPI document is complete.
- [x] GREEN 5: Update manual OpenAPI path and component registration.

## Boundary Review

Use the `improve-code-boundaries` skill before final verification:

- [x] Keep `document_content` SQL query and ordering in `opentk-db::read_model`, not in API handlers.
- [x] Keep API response DTOs in `opentk-api`; do not expose SQL row structs directly as public HTTP types.
- [x] Do not duplicate a second document-content repository module unless `read_model` becomes too muddy; one typed read function is enough for this task.
- [x] Keep large extracted bodies out of generic entity detail endpoints.
- [x] Remove any compatibility aliases or legacy field names if discovered; this project has no backwards compatibility requirement.

## Verification

Run, in order:

- [x] `cargo fmt`
- [x] `make check`
- [x] `make lint`
- [x] `make test`

Do not run `make test-long`; this task is not the story-ending deep verification task and does not explicitly require the long/e2e lane.

After all checks pass:

- [x] Update acceptance boxes in `task-04-expose-document-content-api.md`.
- [x] Set `<passes>true</passes>`.
- [x] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Add all files, including `.ralph` updates, commit with `task finished task-04-expose-document-content-api: ...`, push, and quit immediately.

DONE
