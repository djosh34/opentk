# Story 2 Task 2 Plan: Complete PostgreSQL Schema

## Current State

- `opentk-core::official_schema` is the source-neutral, repository-owned model generated from the vendored official Tweede Kamer XSDs.
- `opentk-db` exists as the PostgreSQL boundary but currently has no schema API, no migrations, and no tests.
- `migrations/` contains only `.gitkeep`.
- The task requires PostgreSQL `sqlx` migrations, complete coverage of all official entities and relations, source metadata for sync correctness, and checks through `make check`, `make lint`, and `make test`.

## Interface Design

Add one database-facing schema specification in `opentk-db`, generated from `opentk-core::official_schema`:

```rust
pub mod postgres_schema;

pub fn schema() -> SchemaSpec;
pub fn sql_name(official_name: &str) -> String;
```

Core public types:

```rust
pub struct SchemaSpec {
    pub tables: Vec<TableSpec>,
    pub indexes: Vec<IndexSpec>,
}

pub struct TableSpec {
    pub name: String,
    pub kind: TableKind,
    pub columns: Vec<ColumnSpec>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKeySpec>,
}

pub enum TableKind {
    SyncMetadata,
    EntityRegistry,
    Entity { category: &'static str },
    Relation { source_category: &'static str, relation_name: &'static str },
    RepeatedScalar { category: &'static str, field_name: &'static str },
}

pub struct ColumnSpec {
    pub name: String,
    pub sql_type: SqlType,
    pub nullable: bool,
}

pub enum SqlType {
    Uuid,
    Text,
    Boolean,
    Integer,
    BigInteger,
    TimestampTz,
    Date,
    Jsonb,
}
```

The public interface stays in `opentk-db` because table naming, SQL type choices, relation-table shape, indexes, and migration coverage are PostgreSQL storage concerns. `opentk-core` remains source-neutral and only exposes official metadata.

## Schema Conventions

- Use lowercase snake_case SQL names derived from official entity and field names.
- Add these metadata columns to every official entity table:
  - `source_category text not null`
  - `source_id uuid not null`
  - `latest_skiptoken bigint not null`
  - `deleted boolean not null`
  - `source_updated_at timestamptz not null`
  - `atom_updated_at timestamptz not null`
- Use `(source_category, source_id)` as the canonical entity identity.
- Add `sync_entity` as the entity registry:
  - primary key `(source_category, source_id)`
  - current metadata fields listed above
  - unique/indexed lookup by category, id, skiptoken, deletion flag, and update timestamps
- Every typed entity table has primary key `(source_category, source_id)` and a foreign key to `sync_entity`.
- Scalar official attributes with maxOccurs `Exactly(1)` are typed columns on the entity table.
- Repeated scalar attributes, if any are present or added later, get generated side tables named `<entity>__<field>` with `(source_category, source_id, ordinal)` primary key and a typed `value` column.
- Relations are represented by generated relation tables named `<source_entity>__<relation_name>` with:
  - `source_category`, `source_id`
  - `relation_name`
  - `target_category text not null`
  - `target_id uuid not null`
  - `ordinal integer not null`
  - `source_updated_at timestamptz not null`
  - primary key including source, relation, target, and ordinal
  - foreign key from source to the source typed table
  - foreign-key-capable target reference to `sync_entity(source_category, source_id)`
- Single relations use the same table shape with a uniqueness constraint on `(source_category, source_id, relation_name)` to enforce maxOccurs `Exactly(1)`.
- Keep JSONB out of official scalar and relation fields. Use JSONB only for future source-adjacent data with explicitly flexible shape, such as raw upstream diagnostic payloads.

## SQL Type Mapping

- `idType`, `referentie*`, and relation target ids: `uuid`
- `xs:boolean`, `booleanType`: `boolean`
- `xs:int`: `integer`
- long counters such as skiptoken/content length: `bigint`
- `xs:dateTime`: `timestamptz`
- `xs:date`: `date`
- official string/token/year/code types: `text`
- missing or empty source `xsd_type`: `text` plus documentation that the official source omitted a concrete type

## Migration Shape

Add reversible SQLx migrations:

- `migrations/20260426000000_complete_sync_schema.up.sql`
- `migrations/20260426000000_complete_sync_schema.down.sql`

The up migration creates:

- `sync_category`
- `ingest_error`
- `sync_entity`
- all generated official entity tables
- all generated relation tables
- all generated repeated-scalar side tables if needed
- document asset metadata needed by `downloadEntiteitType` entities
- indexes for:
  - primary UUID lookups
  - category cursor progress
  - entity update timestamps
  - document numbers
  - common date columns
  - relation source and target endpoints
  - asset owner lookups
  - every foreign-key path

The down migration drops generated relation/side tables first, then entity tables, then shared metadata tables.

## TDD Execution Plan

Follow vertical red-green cycles. Do not write all tests first.

- [x] RED 1: Add one `opentk-db` public-interface test proving `postgres_schema::schema()` contains `Document`, mandatory sync metadata columns, `documentnummer`, and the `document__kamerstukdossier` relation table. Confirm the test fails because no schema API exists.
- [x] GREEN 1: Add the minimal `opentk-db` dependency on `opentk-core`, expose `postgres_schema`, and generate enough schema metadata for `Document` to pass.
- [x] RED 2: Add a coverage test that iterates every `official_schema::entity_types()` entry and fails unless there is exactly one entity table for each official entity. Confirm it fails while only `Document` exists.
- [x] GREEN 2: Generate all entity table specs from the official schema model.
- [x] RED 3: Add a field coverage test that every official scalar field appears either as an entity-table column or as a repeated scalar side table with ordinal semantics. Confirm it fails before complete field mapping.
- [x] GREEN 3: Implement XSD-to-PostgreSQL type mapping and repeated scalar side table generation.
- [x] RED 4: Add a relation coverage test that every official relation field has a generated relation table with source FK metadata, target endpoint columns, ordinal, maxOccurs enforcement, and endpoint indexes. Confirm it fails before relation generation is complete.
- [x] GREEN 4: Generate relation table specs and index specs for all official relations.
- [x] RED 5: Add a migration text/schema-spec test that fails when the checked-in SQL migration does not contain every table, column, primary key, foreign key, and required index from `SchemaSpec`.
- [x] GREEN 5: Add the generated SQLx up/down migrations and make the migration text/spec test pass.
- [x] RED 6: Add a PostgreSQL migration integration test using `sqlx::migrate!()` against `OPENTK_TEST_DATABASE_URL` or `DATABASE_URL`. The test must fail with a clear message if no PostgreSQL database URL is configured, because skipping DB migration tests would give false confidence. It should create an isolated temporary schema or database, run migrations, inspect `information_schema`/`pg_indexes`, then run revert/reset.
- [x] GREEN 6: Add the test harness code and any needed dev dependencies so `cargo test --workspace` runs the migration test against the configured PostgreSQL database.
- [x] RED 7: Add direct-query readiness assertions for the required access paths: category cursor, entity UUID lookup, updated timestamp scans, document number lookup, date scans where date/timestamp fields exist, relation endpoint lookups, asset owner lookup, and FK paths.
- [x] GREEN 7: Add or adjust indexes until the direct-query readiness test passes.
- [x] REFACTOR: Use the improve-code-boundaries skill review. Remove any duplicate hand-maintained entity/field/relation lists introduced during implementation. The official model should be read once from `opentk-core`, transformed once into `SchemaSpec`, and checked against the migration. Do not bury PostgreSQL naming or type decisions in tests.

## Documentation

Update storage documentation, preferably `docs/storage-model.md` plus a focused `docs/postgres-schema.md`, to cover:

- PostgreSQL is the selected storage backend for this story.
- schema generation flows from official XSD metadata through `opentk-core::official_schema` into `opentk-db::postgres_schema`;
- why official scalar fields are direct columns/side tables, not JSONB;
- relation table shape and target registry semantics;
- required source metadata columns and cursor/update replacement semantics;
- how to run migration tests locally, including the required PostgreSQL database URL;
- the reset/revert flow used by tests.

## Verification

Run, in order:

- `cargo fmt`
- `make check`
- `make lint`
- `make test`

Do not run `make test-long`; this task is not the story-ending deep verification task and does not explicitly require the long/e2e lane.

After all checks pass, update `task-2-design-postgres-schema.md` acceptance boxes and set `<passes>true</passes>`, run `/bin/bash .ralph/task_switch.sh`, commit all files with `task finished task-2-design-postgres-schema: ...`, push, and quit immediately.

NOW EXECUTE
