# Story 5 Task 2 Plan: Design Search Indexing Pipeline

## Current State

- Story 5 Task 1 selected Meilisearch because the committed benchmark showed better typo/fuzzy ranking while preserving exact, document text, stored HTML, entity metadata, mixed-query, and update behavior.
- The benchmark crate `opentk-search-eval` is experimental. Production search code must not reuse its DTOs because those shapes exist to compare engines, not to model the sync/read system.
- PostgreSQL remains the source of truth. Search indexing must derive from `sync_entity`, generated entity tables, document fields, `document_asset`, and `document_content`.
- The API already has read endpoints for generic entities, typed document/person/activity details, change pages, and document content. Search result shapes should carry enough identity/provenance to fetch the full record through those public endpoints later.
- This task is design plus mapping correctness. It should add production search mapping/schema artifacts and behavior tests, but it should not implement the Meilisearch service sync loop or HTTP search endpoint.

## Public Interfaces

Add a production search crate:

```text
crates/opentk-search/
```

The crate owns the search boundary and exposes a small pure mapping API that Task 3 can connect to PostgreSQL and Meilisearch:

```rust
pub struct SearchSourceRecord {
    pub metadata: SearchEntityMetadata,
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub document_content: Option<SearchDocumentContent>,
    pub relations: Vec<SearchRelationLabel>,
}

pub struct SearchEntityMetadata {
    pub category: String,
    pub source_id: uuid::Uuid,
    pub latest_skiptoken: i64,
    pub deleted: bool,
    pub source_updated_at: chrono::DateTime<chrono::Utc>,
    pub atom_updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct SearchDocumentContent {
    pub selected_source_url: String,
    pub selected_source_content_type: Option<String>,
    pub official_source: bool,
    pub extraction_status: String,
    pub validation_status: String,
    pub output_hash: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
}

pub struct SearchRelationLabel {
    pub relation_name: String,
    pub target_category: String,
    pub target_id: uuid::Uuid,
    pub label: String,
}

pub enum SearchIndexOperation {
    Upsert(Box<SearchIndexDocument>),
    Delete(SearchDocumentKey),
}

pub struct SearchIndexDocument {
    pub key: SearchDocumentKey,
    pub source_category: String,
    pub source_id: uuid::Uuid,
    pub entity_kind: SearchEntityKind,
    pub title: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub date: Option<String>,
    pub document_number: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub metadata_text: Vec<String>,
    pub relation_labels: Vec<String>,
    pub filter_categories: Vec<String>,
    pub latest_skiptoken: i64,
    pub source_updated_at: chrono::DateTime<chrono::Utc>,
    pub atom_updated_at: chrono::DateTime<chrono::Utc>,
}
```

Expose pure functions first:

```rust
pub fn map_record_to_operation(record: &SearchSourceRecord) -> Result<SearchIndexOperation, SearchMappingError>;
pub fn meilisearch_schema() -> SearchIndexSchema;
pub fn api_result_shape() -> SearchApiResultShape;
pub fn storage_estimate(sample: SearchStorageSample) -> Result<SearchStorageEstimate, SearchStorageError>;
```

Do not put the mapping code in `opentk-db`, `opentk-api`, or `opentk-search-eval`. `opentk-db` can later build `SearchSourceRecord` values from SQL rows; `opentk-api` can later translate search hits into HTTP responses.

## Index Schema

Design one Meilisearch index for all searchable entities, with `key` as the primary key. The key must be deterministic and collision-free:

```text
<source_category>:<source_id>
```

Searchable attributes:

- `title`
- `summary`
- `document_number`
- `extracted_text`
- `extracted_html`
- `metadata_text`
- `relation_labels`

Displayed attributes:

- `key`
- `source_category`
- `source_id`
- `entity_kind`
- `title`
- `summary`
- `source_url`
- `date`
- `document_number`
- `metadata_text`
- `relation_labels`
- `latest_skiptoken`
- `source_updated_at`
- `_formatted`, when returned by Meilisearch

Filterable attributes:

- `source_category`
- `entity_kind`
- `date`
- `filter_categories`
- `latest_skiptoken`

Sortable attributes:

- `date`
- `source_updated_at`
- `latest_skiptoken`

Ranking settings should keep Meilisearch defaults first, then make domain fields stronger:

```text
words, typo, proximity, attribute, sort, exactness
```

Attribute priority should prefer `title`, `document_number`, and `metadata_text` ahead of long body fields. `extracted_html` is searchable and highlightable but should rank lower than `extracted_text` because HTML can contain duplicated navigation or markup-adjacent text.

## Source-To-Index Mapping

The mapping must cover:

- Documents: `Document` rows map `titel`, `document_nummer`, `datum`, `content_type`, `content_length`, `enclosure_url`, extracted plain text, stored HTML, and relation labels.
- People: `Persoon` rows map stable display name from name fields, initials/callsign/surname metadata, party/fractie relation labels, and activity labels when available.
- Activities: `Activiteit` rows map number/type/location/date-like fields plus related people, committees, and dossiers.
- Dossiers and other entities: map category, source id, natural title/label fields, scalar metadata, and relation labels.
- Deletes: `sync_entity.deleted = true` maps to `SearchIndexOperation::Delete`, even when source detail fields are missing.
- Invalid source records: missing source category, source id, or title fallback failure returns an explicit error. Do not silently drop rows.

Title fallback order:

- `titel`
- `onderwerp`
- person display name fields for `Persoon`
- `nummer`
- `document_nummer`
- relation label or category/id fallback only when no better label exists

Metadata text should be generated from scalar fields with stable field labels. Skip empty/null values, but do not ignore type conversion errors. Arrays/objects are allowed only when converted through structured JSON traversal with explicit field paths.

## Update And Delete Propagation

Design a durable indexing state owned by PostgreSQL migrations in Task 3:

```sql
CREATE TABLE search_index_cursor (
    index_name text PRIMARY KEY,
    source_category text NOT NULL,
    latest_skiptoken bigint NOT NULL DEFAULT 0,
    last_indexed_at timestamptz,
    state text NOT NULL,
    last_error text
);

CREATE TABLE search_index_failure (
    id bigserial PRIMARY KEY,
    index_name text NOT NULL,
    source_category text NOT NULL,
    source_id uuid NOT NULL,
    latest_skiptoken bigint NOT NULL,
    operation text NOT NULL,
    attempt_count integer NOT NULL DEFAULT 1,
    next_retry_at timestamptz NOT NULL,
    error text NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (index_name, source_category, source_id, latest_skiptoken, operation)
);
```

Backfill path:

- Reset or create the Meilisearch index with schema settings.
- Page through source categories by stable `latest_skiptoken, source_id` order.
- Map each row to `Upsert` or `Delete`.
- Submit batches to Meilisearch and wait for task completion.
- Advance cursor only after the Meilisearch task succeeds.
- Persist any failed row or failed batch in `search_index_failure` with explicit error detail.

Incremental path:

- Read `sync_entity` changes after each category cursor.
- Rebuild the full source record for changed non-deleted rows.
- Send delete operations for deleted rows without requiring the entity table row to exist.
- Advance cursor only after successful Meilisearch task completion.
- Retry failures before or alongside new changes without letting one bad row disappear.

Failure recovery:

- Startup resumes from `search_index_cursor`.
- Failed operations are retried with capped exponential backoff.
- Permanent mapping errors remain recorded until source data changes or an operator forces retry.
- Meilisearch task failures must bubble up as typed errors. No swallowed errors; if implementation finds an ignored error path, create an add-bug task.

## API Result Shape

Task 4 should expose a stable response derived from Meilisearch hits:

```rust
pub struct SearchResult {
    pub key: String,
    pub source_category: String,
    pub source_id: uuid::Uuid,
    pub entity_kind: SearchEntityKind,
    pub title: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub date: Option<String>,
    pub document_number: Option<String>,
    pub snippets: Vec<SearchSnippet>,
    pub ranking_score: Option<f64>,
}

pub struct SearchSnippet {
    pub field: String,
    pub text: String,
    pub highlighted: Option<String>,
}
```

The result must include enough data for clients to call existing endpoints:

- `/entities/{category}/{source_id}`
- `/documents/{source_id}`
- `/documents/{source_id}/content`
- future typed endpoints for people/activities

Do not expose Meilisearch raw hit JSON as the public API contract.

## Storage Estimate

Update `docs/search-engine-investigation.md` or add `docs/search-indexing-pipeline.md` with a Task 2 storage estimate based on the real Task 1 measurement:

- Task 1 fixture: 5 records used 3,579 bytes in both committed engine result files.
- Naive fixture ratio: about 716 bytes indexed per search record.
- Production estimate formula should use current PostgreSQL verification metrics:
  - entity row count by category;
  - `document_content.extracted_text` bytes;
  - `document_content.extracted_html` bytes;
  - relation label expansion allowance;
  - Meilisearch overhead factor.
- Because the Task 1 fixture is tiny, document the estimate as a sizing model, not a guarantee. Task 5 must replace it with real synced-data measurement.

## TDD Execution Plan

Use vertical red-green cycles. Do not write every test upfront.

- [x] RED 1: Add a behavior test in `opentk-search` proving a complete `Document` source fixture maps to one `Upsert` with document identity, title, document number, source URL, extracted text, stored HTML, metadata text, and relation labels. The test must fail before the mapper exists.
- [x] GREEN 1: Add the minimal crate, types, and mapper branch for complete document records.
- [x] RED 2: Add a behavior test proving the same fixture fails with an explicit mapping error when an expected indexed field such as title/document number/body field is removed. This satisfies the acceptance requirement that missing indexed fields fail.
- [x] GREEN 2: Add required-field validation and structured error messages.
- [x] RED 3: Add a behavior test for an entity metadata fixture, preferably `Persoon`, proving names, category/id, scalar metadata, and relation labels map into search fields without using document-only fields.
- [x] GREEN 3: Implement non-document entity mapping and title fallback.
- [x] RED 4: Add a behavior test proving `deleted = true` maps to `Delete` using only metadata, and an update fixture maps to `Upsert` with a higher `latest_skiptoken` and changed body/title fields.
- [x] GREEN 4: Implement update/delete mapping semantics.
- [x] RED 5: Add a behavior test that validates the declared Meilisearch schema contains required searchable/displayed/filterable/sortable attributes and primary key.
- [x] GREEN 5: Implement `meilisearch_schema()`.
- [x] RED 6: Add a documentation validation test that fails unless `docs/search-indexing-pipeline.md` covers backfill, incremental indexing, retry/failure recovery, update/delete propagation, API result shape, and the storage estimate using Task 1 measurements.
- [x] GREEN 6: Write the design document.

Run targeted tests after each green step, then the full checks at the end.

## Boundary Review

Use the `improve-code-boundaries` skill before final verification:

- Keep production mapping and schema types in `opentk-search`.
- Keep SQL paging/cursor implementation out of this task unless a tiny design-only type is needed.
- Do not reuse or expose `opentk-search-eval` benchmark DTOs.
- Do not add HTTP endpoint DTOs to `opentk-api` in this task; define the intended API result contract in docs/types only.
- If mapping requires duplicating generated official schema shapes, stop and redesign around generic source records plus explicit category-specific display-title helpers.
- Delete any placeholder or unused module before final checks.

## Verification

Run, in order:

- [x] `cargo fmt`
- [x] `make check`
- [x] `make lint`
- [x] `make test`

Do not run `make test-long` for this task. This task designs mapping and indexing behavior; it does not finish the search story and does not change long/e2e test selection.

After all checks pass:

- [x] Update acceptance boxes in `task-2-design-search-indexing.md`.
- [x] Set `<passes>true</passes>`.
- [x] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Add all files, including `.ralph` updates, commit with `task finished task-2-design-search-indexing: ...`, push, and quit immediately.

NOW EXECUTE
