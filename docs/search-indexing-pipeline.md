# Search Indexing Pipeline

Story 5 uses Meilisearch as the production search engine. PostgreSQL remains the source of truth; the search index is a derived read model built from `sync_entity`, generated entity tables, relation tables, `document_asset`, and `document_content`.

Production search code lives in `opentk-search`. It accepts generic source records and emits explicit index operations. Database paging and Meilisearch HTTP calls belong in the next task so mapping stays testable without services.

## Index Schema

Use one Meilisearch index named `opentk_entities` with primary key `key`. The key is deterministic:

```text
<source_category>:<source_id>
```

Searchable attributes are ordered by domain value: `title`, `summary`, `document_number`, `metadata_text`, `relation_labels`, `extracted_text`, then `extracted_html`. Stored HTML is searchable and highlightable, but ranks below extracted text because HTML can contain duplicated markup-adjacent text.

Displayed attributes are `key`, `source_category`, `source_id`, `entity_kind`, `title`, `summary`, `source_url`, `date`, `document_number`, `metadata_text`, `relation_labels`, `latest_skiptoken`, `source_updated_at`, and `_formatted`.

Filterable attributes are `source_category`, `entity_kind`, `date`, `filter_categories`, and `latest_skiptoken`. Sortable attributes are `date`, `source_updated_at`, and `latest_skiptoken`. Ranking rules use the Meilisearch defaults in explicit order: `words`, `typo`, `proximity`, `attribute`, `sort`, `exactness`.

## Source Mapping

`SearchSourceRecord` carries entity metadata, scalar fields as structured JSON, optional document content, and resolved relation labels. `map_record_to_operation` returns either an upsert document or a delete operation.

Documents map `titel`, `document_nummer`, `datum`, `content_type`, `content_length`, `enclosure_url`, extracted text, stored HTML, extraction/validation status, output hash, and relation labels. Missing expected indexed document fields are mapping errors; incomplete rows must not be silently indexed.

People map display names from `roepnaam` or `initialen`, optional `tussenvoegsel`, and `achternaam`. Person scalar metadata and relation labels, such as party/fractie labels, are indexed without requiring document-only fields.

Activities, dossiers, and other entities use the same generic source record. Title fallback order is `titel`, `onderwerp`, person display name, `nummer`, `document_nummer`, then relation label or category/id fallback when no better label exists. Scalar metadata is generated through structured JSON traversal with stable field paths; null and empty string values are skipped.

## Update And Delete Propagation

Each `sync_entity` change becomes one index operation. A non-deleted source row is rebuilt into a full `SearchSourceRecord` and mapped to `Upsert`. A deleted row maps to `Delete` using only `source_category` and `source_id`; the generated entity table row does not need to exist.

The upsert carries `latest_skiptoken`, `source_updated_at`, and `atom_updated_at`, so incremental indexing can replace old index state without comparing partial payloads. Deletes use the same deterministic key as upserts, which avoids category-specific delete logic.

## Backfill Indexing

Task 3 should create or reset the Meilisearch index, apply schema settings, then page PostgreSQL source records in stable `latest_skiptoken, source_id` order. Each page is mapped to operations, sent to Meilisearch as a batch, and confirmed by waiting for the Meilisearch task result.

The cursor advances only after Meilisearch reports success for the submitted task. Failed rows or batches are recorded with the source identity, operation, skiptoken, and error message.

## Incremental Indexing

Incremental indexing reads `sync_entity` rows after the category cursor. For changed non-deleted rows it rebuilds the full source record from generated entity tables, document content, and relation labels. For deleted rows it emits delete operations directly from `sync_entity`.

Retries should run before or alongside new changes. One bad row must remain visible in failure state and must not block unrelated categories forever.

## Failure Recovery

Task 3 should add durable cursor and failure tables:

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

Startup resumes from `search_index_cursor`. Failed operations use capped exponential backoff through `search_index_failure`. Permanent mapping errors remain recorded until source data changes or an operator forces retry. Meilisearch task failures must bubble up as typed errors; no indexing error may be swallowed.

## API Result Shape

The public API should not expose raw Meilisearch hit JSON. Task 4 should translate hits into a stable result shape:

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

These fields are enough for clients to navigate to existing read endpoints such as `/entities/{category}/{source_id}`, `/documents/{source_id}`, and `/documents/{source_id}/content`.

## Storage Estimate

Task 1 measured both committed candidate-engine fixture outputs at 3,579 bytes for 5 records, or roughly 716 bytes per indexed record. That fixture is intentionally tiny, so this is a sizing model, not a production guarantee.

Task 3 should calculate:

```text
estimated_index_bytes =
  (document_content.extracted_text bytes
   + document_content.extracted_html bytes
   + relation label expansion bytes)
  * Meilisearch overhead factor
```

The input counts should come from PostgreSQL verification metrics: entity row count by category, extracted text bytes, extracted HTML bytes, and relation label expansion allowance. Task 5 must replace the estimate with a real synced-data measurement.
