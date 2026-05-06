# Search Indexing Pipeline

OpenTK uses Meilisearch as a derived public search read model. PostgreSQL is the source of truth. Search synchronization is owned by the dedicated `opentk-search-reconciler` binary, not by `opentk-api` and not by `opentk-sync`.

The API keeps the public `/search` response contract stable and only queries Meilisearch. The sync binary only ingests source data into PostgreSQL. The old full-reindex, incremental indexing, and API-side CDC search indexing operational paths are retired.

## Index Schema

Use one Meilisearch index per environment, with deterministic document IDs derived from source identity:

```text
<source_category>_<source_id>
```

The public result key remains:

```text
<source_category>:<source_id>
```

Searchable attributes are ordered by domain value: `title`, `summary`, `document_number`, `metadata_text`, `relation_labels`, `extracted_text`, then `extracted_html`. Stored HTML is searchable and highlightable, but ranks below extracted text because HTML can contain duplicated markup-adjacent text.

Displayed attributes are `id`, `key`, `source_category`, `source_id`, `entity_kind`, `title`, `summary`, `source_url`, `date`, `document_number`, `metadata_text`, `relation_labels`, `latest_skiptoken`, `source_updated_at`, `atom_updated_at`, and `_formatted`.

Filterable attributes are `source_category`, `entity_kind`, `date`, `filter_categories`, and `latest_skiptoken`. Sortable attributes are `date`, `source_updated_at`, and `latest_skiptoken`. The reconciler relies on `latest_skiptoken` being sortable so it can fetch the highest indexed skiptoken with a sorted `limit = 1` query.

## Source Mapping

`SearchSourceRecord` carries entity metadata, scalar fields as structured JSON, optional document content, and resolved relation labels. `map_record_to_operation` returns either an upsert document or a delete operation.

Documents map `titel`, `document_nummer`, `datum`, `content_type`, `content_length`, `enclosure_url`, extracted text, stored HTML, extraction/validation status, output hash, and relation labels. Missing expected indexed document fields are mapping errors; incomplete rows must not be silently indexed.

People map display names from `roepnaam` or `initialen`, optional `tussenvoegsel`, and `achternaam`. Person scalar metadata and relation labels, such as party/fractie labels, are indexed without requiring document-only fields.

Activities, dossiers, and other entities use the same generic source record. Title fallback order is `titel`, `onderwerp`, person display name, `nummer`, `document_nummer`, then relation label or category/id fallback when no better label exists. Scalar metadata is generated through structured JSON traversal with stable field paths; null and empty string values are skipped.

The reconciler intentionally reuses this Rust mapper when populating the scratch table. PostgreSQL still owns source-of-truth counts, skiptoken windows, and the inspectable projection rows. Duplicating the mapping in SQL would create a second source of truth for public search semantics.

## Reconciliation Algorithm

For each category, `opentk-search-reconciler`:

1. Ensures the Meilisearch index exists and has the configured schema settings without deleting existing documents.
2. Fetches the highest Meilisearch `latest_skiptoken` for the category using `sort=["latest_skiptoken:desc"]` and `limit=1`.
3. Compares PostgreSQL and Meilisearch prefix counts for `latest_skiptoken <= A`.
4. If prefix counts mismatch, binary-searches for the highest count-matching prefix boundary and deletes Meilisearch category documents above that verified boundary.
5. Selects PostgreSQL batch windows ordered by `latest_skiptoken, source_id`.
6. Replaces rows in `search_reconciler_scratch` for the current `(index_name, source_category)` window.
7. Stores one inspectable JSONB/NDJSON-compatible scratch row per Meilisearch operation, including `document_id`, `source_category`, `source_id`, `latest_skiptoken`, `operation`, `payload`, and `payload_bytes`.
8. Applies scratch operations to Meilisearch with byte-bounded requests independent from the PostgreSQL row batch size.
9. Clears scratch only after successful Meilisearch writes/deletes.
10. Repeats until category counts match.

If counts already match after a nonzero verified prefix, the reconciler refreshes the full category from PostgreSQL anyway. Count checks alone cannot detect a same-count stale payload, so this periodic idempotent refresh is what makes stale Meilisearch documents converge without a durable cursor.

## Update And Delete Semantics

Upserts are idempotent because the deterministic Meilisearch primary key replaces the existing document. Rewriting the same PostgreSQL source row is expected and safe.

Deleted source rows map to deterministic Meilisearch deletes. During prefix repair, the reconciler may also delete category documents above the verified prefix before rebuilding the suffix from PostgreSQL. This removes ahead or extra indexed documents that cannot be repaired by upserts alone.

`search_index_cursor` and `search_index_failure` may still exist in old databases for compatibility with previous migrations, but they are not reconciliation source-of-truth state. The scratch table is not a queue and does not store durable progress.

## Status And Operations

Use:

```bash
opentk-search-reconciler --config /etc/opentk/config.toml once
opentk-search-reconciler --config /etc/opentk/config.toml status
opentk-search-reconciler --config /etc/opentk/config.toml --health-check
```

Category status includes PostgreSQL count, Meilisearch count, highest Meilisearch skiptoken, verified prefix boundary, target boundary, scratch row count, payload bytes, inserted/deleted row counts, completion state, and loop duration.

The deployment batch size is expected to be `search.batch_size = 50000`; Meilisearch payloads remain bounded separately by `search.max_payload_bytes`.
