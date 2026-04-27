# Storage Model

## Baseline

OpenTK stores Tweede Kamer `SyncFeed` data in PostgreSQL. The importer writes
parsed feed entities into relational tables in the same transaction that
advances cursor metadata.

The write path is:

```text
fetch feed page
parse Atom envelope
parse embedded entity XML
write sync metadata
write typed entity row
write relation rows
write upstream document asset metadata from SyncFeed
advance category cursor
commit
fetch linked document assets
record retrieval status and official text/HTML content
extract binary document content when no official text/HTML source is available
```

The critical invariant is:

```text
cursor moves after relational writes commit
```

## Why Direct Relational Storage

The official information model is represented directly in database structure:

- every official entity type has a table,
- official scalar fields are typed columns or generated repeated-scalar side
  tables,
- official references are relation tables with indexed source and target
  endpoints,
- cursor state is transactional,
- source metadata is queryable without reparsing XML,
- `opentk-db::postgres_schema` defines the database shape explicitly.

JSONB is reserved for source-adjacent diagnostics whose shape is not part of
the official model, such as ingest error payloads. Official scalar fields and
relations are not stored as opaque JSON.

## Source Of Truth

Schema generation flows through one boundary:

```text
vendored official XSDs
opentk-core::official_schema
opentk-db::postgres_schema::SchemaSpec
opentk-db::schema_lifecycle
```

`opentk-core` owns source-neutral official metadata. `opentk-db` owns
PostgreSQL naming, type mapping, table shape, indexes, startup creation, and
API compatibility validation. Schema lifecycle tests verify the live PostgreSQL
catalog against `SchemaSpec` so entity, field, relation, primary-key,
foreign-key, and index coverage are not hand-maintained in separate lists.

## Core Tables

Shared tables:

- `sync_category`: category-level cursor progress.
- `sync_entity`: current registry row for each `(source_category, source_id)`.
- `ingest_error`: structured ingest failures plus optional JSONB diagnostic
  payload.

Every official entity table has:

- `source_category text not null`
- `source_id uuid not null`
- `latest_skiptoken bigint not null`
- `deleted boolean not null`
- `source_updated_at timestamptz not null`
- `atom_updated_at timestamptz not null`

The canonical identity is `(source_category, source_id)`. Each typed entity
table uses that pair as its primary key and foreign-keys it to `sync_entity`.

## Document Assets And Content

Official document rows keep upstream metadata from SyncFeed, including
download metadata (`content_type`, `content_length`, and `enclosure_url`) when
the official source exposes an enclosure.

Document retrieval and extraction state is storage-owned:

- `document_asset` preserves fetchable and upstream URLs, upstream content
  metadata, retrieval status, retrieval error details, and retrieval
  timestamps.
- `document_content` stores the selected source URL, official-source ranking,
  source content metadata, extraction provenance, validation status, content
  source hash, output hash, extraction error detail, extraction timestamp, and
  extracted text/HTML.

The asset fetcher records every retrieval attempt in `document_asset`,
including durable `not_found`, `unsupported_content_type`, and `failed` states
with retryable error detail. When the upstream asset or an official alternative
is already text, HTML, XHTML, or transcript content, the real body is persisted
in `document_content` immediately with `extraction_status = 'extracted'` and
`validation_status = 'unverified'`. Binary PDF and DOCX files are extraction
inputs only when no official text/HTML source was selected. Extraction uses
strict fixture matching for supported PDF, DOCX, and HTML inputs; mismatches,
parser errors, missing text, ordering defects, or encoding defects are recorded
as explicit `failed`/`invalid` `document_content` rows with an
`extraction_error` rather than accepted approximately. Deterministic
normalization hashes the selected source as `source_hash` and the stored output
as `output_hash`.

`document_asset` and `document_content` both keep a direct
`(document_source_category, document_source_id)` owner path back to
`document(source_category, source_id)`. `document_content` also links to the
specific `document_asset` row that produced the extracted body. This keeps the
source metadata queryable independently from extraction attempts while letting
readers prefer official text/HTML rows through `official_source` and
`source_rank`.

Storage verification reports document-content footprint separately from base
relation size. `link_metadata_bytes` measures durable fetch/link metadata in
`document_asset`, `extracted_text_bytes` measures stored text bodies,
`extracted_html_bytes` measures stored HTML bodies, and `index_bytes` plus
`constraint_count` show the database structures that keep the owner and
provenance reads enforceable. The older aggregate HTML placeholder is not kept;
HTML is now measured from actual `document_content.extracted_html` bytes.

## Relations

Each official relation field has a generated table named:

```text
<source_entity>__<relation_name>
```

Relation tables contain:

- source identity: `source_category`, `source_id`,
- relation identity: `relation_name`,
- target endpoint: `target_category`, `target_id`,
- relation order: `ordinal`,
- update metadata: `source_updated_at`.

Targets foreign-key to `sync_entity(source_category, source_id)`, making
relation endpoints queryable before choosing a concrete typed target table.
Single relations add a uniqueness constraint on
`(source_category, source_id, relation_name)`.

## Repeated Scalars

Repeated official scalar fields use side tables named:

```text
<entity>__<field>
```

They use `(source_category, source_id, ordinal)` as the primary key and store
the scalar in a typed `value` column.

## Indexes

The schema includes indexes for the access paths needed by sync and direct HTTP
queries:

- primary UUID lookups,
- category cursor progress,
- source update timestamp scans,
- document number lookup,
- official date and timestamp scans,
- relation source endpoints,
- relation target endpoints,
- document asset owner lookups,
- document asset URL lookups,
- document extracted-content owner lookups,
- official content source ranking,
- foreign-key paths.

## Schema Lifecycle Tests

`make test` starts a local throwaway PostgreSQL server under `target/` when no
external `OPENTK_TEST_DATABASE_URL` is supplied, then runs the workspace test
suite. Direct PostgreSQL lifecycle tests require `OPENTK_TEST_DATABASE_URL` or
`DATABASE_URL`; running them directly without one fails instead of silently
skipping database verification.

The lifecycle tests create isolated temporary schemas, run `ensure_schema`,
validate that expected tables and indexes exist, and verify repeated startup
does not remove durable cursor or entity data.
