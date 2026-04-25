## Plan: Complete SyncFeed Parser and PostgreSQL Writers

## Sources

- Task file: `.ralph/tasks/story-2-complete-sync-postgres/task-4-implement-complete-parser-and-writers.md`
- Story 2 task 1 plan and implementation: `opentk-core::official_schema` is the source-neutral generated official model.
- Story 2 task 2 plan and implementation: `opentk-db::postgres_schema` is the PostgreSQL schema source and migration renderer.
- Story 2 task 3 plan and implementation: `opentk-sync::syncfeed` fetches Atom pages and exposes raw embedded entity XML plus enclosure URLs.
- `tdd` skill: use red-green tracer bullets through public behavior; one test, one implementation, then repeat.
- `improve-code-boundaries` skill: remove duplicate DTOs and stringly conversions; keep the parser and writer behind narrow public interfaces.

## Boundary Design

Keep the official schema model in `opentk-core`; do not copy entity/field/relation lists into parser or writer code. The task should add two deep modules:

- `opentk-sync::payload` parses embedded SyncFeed entity XML into one typed representation.
- `opentk-db::sync_writer` writes one parsed page into PostgreSQL transactionally.

The parser should use `quick-xml` for strict streaming XML parsing. `roxmltree` may stay in the existing Atom client, but entity payload parsing belongs in the new `quick-xml` parser because the task explicitly requires it.

The parser public interface should be small:

```rust
pub fn parse_entity_xml(category: &str, xml: &str) -> Result<ParsedEntity, PayloadParseError>;
pub fn parse_entry_payload(entry: &SyncFeedEntry) -> Result<ParsedEntity, PayloadParseError>;
```

Core parser types:

```rust
pub struct ParsedEntity {
    pub category: String,
    pub xml_element: String,
    pub source_id: uuid::Uuid,
    pub deleted: bool,
    pub source_updated_at: OffsetDateTime,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
    pub scalars: Vec<ParsedScalar>,
    pub relations: Vec<ParsedRelation>,
    pub enclosure_url: Option<Url>,
}

pub struct ParsedScalar {
    pub name: String,
    pub value: ParsedValue,
    pub ordinal: i32,
}

pub struct ParsedRelation {
    pub name: String,
    pub target_category: String,
    pub target_id: uuid::Uuid,
    pub target_updated_at: Option<OffsetDateTime>,
    pub ordinal: i32,
}

pub enum ParsedValue {
    Text(String),
    Bool(bool),
    I32(i32),
    I64(i64),
    Date(time::Date),
    DateTime(OffsetDateTime),
}
```

If `time` integration proves awkward with `sqlx`, use `chrono` consistently instead, but do not keep dates as strings after parsing. UUIDs, booleans, integers, dates, and timestamps must be validated at parse time so writer errors are storage errors, not delayed XML errors.

The writer public interface should be page-oriented and transactional:

```rust
pub struct SyncPageWrite {
    pub category: String,
    pub latest_skiptoken: i64,
    pub atom_updated_at: OffsetDateTime,
    pub entities: Vec<ParsedEntity>,
}

pub async fn write_sync_page(
    pool: &sqlx::PgPool,
    page: SyncPageWrite,
) -> Result<SyncPageWriteOutcome, SyncPageWriteError>;
```

The writer should:

- start one transaction per page;
- upsert `sync_entity`;
- replace the current row in the typed entity table for updated non-deleted entities;
- record deleted entities in `sync_entity` and remove typed scalar/relation current rows for that entity;
- replace relation and repeated-scalar rows for each updated source entity deterministically;
- write `content_type`, `content_length`, and enclosure URL metadata for `downloadEntiteitType` entities;
- update `sync_category.latest_skiptoken` in the same transaction;
- commit only after all entity rows, relation rows, asset metadata, and cursor metadata succeed.

## Sync Semantics

- `id`, `verwijderd`, and `bijgewerkt` from the official XML base attributes become canonical `source_id`, `deleted`, and `source_updated_at`.
- `SyncFeedEntry.updated` becomes `atom_updated_at`; if page-level plumbing cannot supply it initially, keep it explicit in `SyncPageWrite` and require tests to set it.
- A delete marker is not ignored. It is persisted in `sync_entity.deleted = true`, with `source_updated_at` and `latest_skiptoken` updated.
- A deleted entity should not leave stale queryable current entity rows or relation rows behind.
- Non-deleted updates are last-write-wins by source identity within the page transaction. The task is greenfield, so no compatibility layer for older row shapes is needed.
- Unknown XML elements, missing mandatory base attributes, invalid datatype text, duplicate single-occurrence fields, and category/root mismatches are hard errors.
- `xsi:nil="true"` on nullable scalar fields becomes absence/null. `xsi:nil` on non-nullable fields is a parser error.
- Relation elements are parsed from official `referentieLiteral` / `referentieBijgewerktLiteral` fields and must include a target id. If the official XML exposes target category explicitly, use it; otherwise infer it from official schema metadata only in one helper.

## TDD Execution Plan

Use vertical red-green cycles. For each RED, add one failing public behavior test, run the narrow test command and observe failure, then implement only enough production code to pass.

- [x] RED 1: Add a parser fixture test in `crates/opentk-sync/tests/payload_parser.rs` for one `Document` XML payload with base attributes, scalar `documentNummer`, one `kamerstukdossier` relation, and enclosure URL supplied from a `SyncFeedEntry`. Confirm it fails because no payload parser exists.
- [x] GREEN 1: Add `opentk-sync::payload`, `quick-xml`, `uuid`, and date/time dependencies as needed. Parse the root entity, base attributes, one scalar, one relation, and entry enclosure into `ParsedEntity`.
- [x] RED 2: Add a parser fixture test for repeated relations, using a representative official entity such as `Document.activiteit` or `Activiteit.voortgezetVanuit`, asserting stable ordinals and all target ids. Confirm failure before repeated relation handling.
- [x] GREEN 2: Drive relation parsing from `official_schema::EntityType` field metadata, preserving document order and ordinals for repeated fields.
- [x] RED 3: Add a parser fixture test for delete markers: a payload with `verwijderd="true"` must parse base metadata and refuse scalar/relation body content that would create current rows. Confirm failure before delete-specific handling.
- [x] GREEN 3: Implement delete-marker parsing and explicit validation. Deleted entities produce no scalars or relations, but still carry category/id/source update metadata.
- [x] RED 4: Add scalar datatype coverage through representative fixtures for boolean, integer, long, date, datetime, nullable/nil, and text fields. Confirm failure before type conversion is complete.
- [x] GREEN 4: Map official XSD types from `opentk-core::official_schema` to `ParsedValue`; reject invalid values with field/category/id context.
- [x] RED 5: Add a coverage test that iterates every `official_schema::entity_types()` entry and parses a representative minimal XML fixture for each category. Confirm failure until fixtures and model-driven parsing cover every official entity type.
- [x] GREEN 5: Add representative fixtures for all official modeled entity categories and parser support for all modeled fields, relations, base attributes, multiplicity, and `downloadEntiteitType` metadata.
- [x] RED 6: Add a PostgreSQL writer integration test in `crates/opentk-db/tests/sync_writer.rs` that writes one non-deleted `Document` page and then verifies via SQL public tables that `sync_entity`, `document`, `document__kamerstukdossier`, and category cursor rows are present in one committed transaction. Confirm failure because no writer exists.
- [x] GREEN 6: Add `opentk-db::sync_writer`, depend on the parsed payload types, build dynamic SQL from `postgres_schema::schema()`, and implement one entity/relation page write transaction.
- [x] RED 7: Add a writer update replacement test: write a `Document`, then write the same `source_id` with changed scalar and changed relation rows. Assert the current typed row and relation rows reflect only the second page. Confirm failure before deterministic replacement.
- [x] GREEN 7: Delete/replace relation and repeated-scalar rows for each source entity inside the page transaction before inserting current rows; upsert typed entity rows with `ON CONFLICT`.
- [x] RED 8: Add a writer delete-marker test: write a non-deleted entity, then write a deleted entity marker for the same id. Assert `sync_entity.deleted = true`, typed entity current row is gone or marked consistently with schema conventions, and relation rows are removed. Confirm failure before delete semantics.
- [x] GREEN 8: Implement delete-marker persistence and current-row cleanup in the same transaction.
- [x] RED 9: Add a writer asset metadata test for a `downloadEntiteitType` entity such as `Document`, asserting `content_type`, `content_length`, and enclosure URL metadata are written. Confirm failure before asset metadata write support.
- [x] GREEN 9: Persist download metadata to the generated entity table columns and add an asset/enclosure metadata column/table only if the existing migration lacks a place for the enclosure URL. If schema changes are needed, update `postgres_schema`, migrations, docs, and tests in this task.
- [x] RED 10: Add a transaction rollback test using a deliberately invalid second entity in a page, then assert no first entity or cursor row committed. Confirm failure before page transaction boundaries are complete.
- [x] GREEN 10: Ensure `write_sync_page` owns the transaction, returns explicit errors, and never commits partial page writes.
- [x] REFACTOR: Use the improve-code-boundaries review. Remove duplicate parser DTOs, duplicate SQL table/column naming logic, and any hand-written official entity lists. Keep exactly one parser output shape and one schema-to-SQL naming boundary.

## Test Fixture Strategy

- Keep parser fixtures under `crates/opentk-sync/tests/fixtures/syncfeed_payloads/`.
- Use small representative official payloads, not full live pages.
- Tests should call public parser/writer APIs and assert behavior through returned typed data or SQL-visible public tables.
- Do not test `.ralph/` files.
- Do not skip PostgreSQL tests. Missing `OPENTK_TEST_DATABASE_URL` or `DATABASE_URL` should fail with a clear message, matching the existing migration test stance.

## PostgreSQL Implementation Notes

- Prefer generated SQL from `opentk-db::postgres_schema` over hand-maintained table lists.
- Dynamic identifiers must be quoted through one local helper; values must always be bound parameters.
- Use `sqlx::QueryBuilder<Postgres>` for set-based inserts where it materially simplifies repeated scalars/relations.
- If bulk `COPY` would add complexity before correctness is proven, first implement bounded batched inserts in transactions, then refactor only where tests prove the behavior remains intact.
- Do not swallow unknown-field or SQL errors. Unknown fields are parser errors; SQL failures become writer errors with table/category/source id context.

## Documentation

Update focused docs after implementation:

- `docs/syncfeed-client.md` or a new `docs/syncfeed-parser.md` for embedded XML parser rules, strictness, datatype mapping, nil handling, and enclosure handling.
- `docs/postgres-schema.md` for page writer semantics, replacement/delete behavior, transaction scope, and relation queryability if the existing schema docs need new details.

## Verification

Run narrow tests after each red-green cycle. Before completion, run in order:

- `cargo fmt`
- `make check`
- `make lint`
- `make test`

Do not run `make test-long`; this is not the story-ending task and the task does not explicitly require long/e2e validation.

After all checks pass:

- update every completed acceptance criterion in `task-4-implement-complete-parser-and-writers.md`;
- set `<passes>true</passes>`;
- run `/bin/bash .ralph/task_switch.sh`;
- add and commit all changed files, including `.ralph` files, with `task finished task-4-implement-complete-parser-and-writers: ...`;
- push;
- quit immediately.

NOW EXECUTE
