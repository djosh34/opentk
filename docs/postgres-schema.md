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

## Sync Page Writes

`opentk-db::sync_writer` writes one SyncFeed page per transaction. It upserts
`sync_entity`, replaces the current typed entity row for non-deleted updates,
inserts relation and repeated-scalar rows from the parsed payload, records
download metadata, and advances `sync_category.latest_skiptoken` in the same
commit.

Delete markers are persisted in `sync_entity` and remove current typed rows,
which cascades stale relation and repeated-scalar rows. Relation targets are
registered in `sync_entity` before relation rows are inserted, so relation
tables remain queryable by both source and target endpoint.

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
