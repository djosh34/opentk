# PostgreSQL Schema

The PostgreSQL schema is generated from `opentk-core::official_schema` by
`opentk-db::postgres_schema`.

## Naming

Official names are converted to lowercase snake case:

- `Document` becomes `document`.
- `DocumentActor` becomes `document_actor`.
- `documentNummer` becomes `document_nummer`.
- `Document.kamerstukdossier` becomes `document__kamerstukdossier`.

Identifiers are quoted in migrations to avoid collisions with SQL keywords.

## Type Mapping

The schema maps official XSD types to PostgreSQL types:

- `idType`, `referentieLiteral`, `referentieType`: `uuid`
- `xs:boolean`, `booleanType`: `boolean`
- `xs:int`, `xs:unsignedInt`, `intType`: `integer`
- `xs:long`: `bigint`
- `xs:dateTime`: `timestamptz`
- `xs:date`: `date`
- strings, tokens, codes, years, and omitted concrete XSD types: `text`

Omitted XSD types are stored as `text` because the official source did not
provide a narrower type.

## Entity Tables

Each official entity has one typed table. The table primary key is
`(source_category, source_id)` and references the same key in `sync_entity`.

The base official attributes `id`, `verwijderd`, and `bijgewerkt` are stored in
canonical source metadata columns:

- `source_id`
- `deleted`
- `source_updated_at`

`downloadEntiteitType` asset metadata is stored as direct columns:

- `content_type`
- `content_length`
- `enclosure_url`

Document extraction storage is separate from the generated official
`document` entity table. The main `document` row keeps the upstream official
metadata; retrieval and extracted body state live in storage-owned tables.

`document_asset` stores one fetchable asset/source URL for a document:

- document owner: `document_source_category`, `document_source_id`
- source links: `asset_url`, `upstream_url`
- upstream metadata: `upstream_content_type`, `upstream_content_length`,
  `upstream_last_modified_at`
- retrieval state: `retrieval_status`, `retrieval_error`, `retrieved_at`
- storage timestamps: `created_at`, `updated_at`

`retrieval_status` is constrained to `pending`, `fetched`, `not_found`,
`unsupported_content_type`, or `failed`. Asset rows foreign-key to
`document(source_category, source_id)` and are unique per
`(document_source_category, document_source_id, asset_url)`.

`opentk-db::document_assets` upserts these rows from typed fetch reports. A
later retry of the same `(document_source_category, document_source_id,
asset_url)` updates the existing row with the latest upstream metadata,
retrieval status, error detail, and retrieval timestamp.

`document_content` stores official text/HTML source selection and extracted
content:

- source path: `document_asset_id`, `document_source_category`,
  `document_source_id`
- selected source metadata: `selected_source_url`,
  `selected_source_content_type`, `selected_source_content_length`,
  `official_source`, `source_rank`
- extraction provenance: `extraction_status`, `validation_status`,
  `extraction_tool`, `extraction_tool_version`, `source_hash`, `output_hash`,
  `extraction_error`, `extracted_at`
- extracted bodies: `extracted_text`, `extracted_html`
- storage timestamps: `created_at`, `updated_at`

Official government text/HTML sources are represented by `official_source`
and ordered with `source_rank` so readers can prefer those rows when available.
`extraction_status` is constrained to `pending`, `extracted`, `empty`, or
`failed`; `validation_status` is constrained to `unverified`, `valid`, or
`invalid`. Successful rows must contain at least one of `extracted_text` or
`extracted_html`. Bodyless rows are allowed only when
`extraction_status = 'failed'` and `extraction_error` is present, so parser or
fixture failures are durable instead of swallowed. `(document_asset_id,
source_hash, output_hash)` prevents duplicate extraction results for the same
asset/source/output.

For this stage, `document_content` means a real persisted text or HTML body
exists. Official text, HTML, XHTML, and transcript sources are inserted with the
official body in `extracted_text` or `extracted_html`; binary PDF/DOCX rows are
inserted only after extraction succeeds or fails with explicit provenance.

## Sync Page Writes

`opentk-db::sync_writer` writes one SyncFeed page per transaction. It upserts
`sync_entity`, replaces the current typed entity row for non-deleted updates,
inserts relation and repeated-scalar rows from the parsed payload, records
download metadata, and advances `sync_category.latest_skiptoken`, `next_url`,
`state`, and `last_fetch_at` in the same commit.

Delete markers are persisted in `sync_entity` and remove current typed rows,
which cascades stale relation and repeated-scalar rows. Relation targets are
registered in `sync_entity` before relation rows are inserted, so relation
tables remain queryable by both source and target endpoint.

`sync_category` is the durable runner progress table. A category row stores the
latest committed skiptoken, next cursor URL, optional resume URL, state
(`not_started`, `running`, `caught_up`, or `error`), last fetch time, and
caught-up time. The runner resumes from `next_url`; it does not keep a second
cursor truth outside PostgreSQL.

`ingest_error` stores durable runner failures with an identity primary key,
phase (`fetch`, `parse`, `write`, or `mark_caught_up`), category, optional
skiptoken, optional entity id, message, optional payload, and creation time.
Errors are inserted through `opentk-db::sync_state` and are reported by the
status command.

## Relation Tables

Every official relation has a generated table. Relation rows store both source
and target endpoints as category/id pairs so API queries can join through the
registry without reparsing document XML.

For relations with `maxOccurs="1"`, the table includes a uniqueness constraint
on `(source_category, source_id, relation_name)`.

## Verification

The schema has two levels of tests:

- `postgres_schema` tests validate the generated `SchemaSpec` covers every
  official entity, scalar field, relation, primary key, foreign-key path, and
  required index purpose.
- `postgres_migrations` runs and reverts the checked-in SQLx migrations against
  a fresh PostgreSQL schema.

The checked-in migration text must exactly match
`postgres_schema::render_up_migration` and
`postgres_schema::render_down_migration`.
