# Story 03 Task 02 Plan: Core Read Endpoints

## Context Read

- Active task: `.ralph/tasks/story-03-http-api/task-02-implement-core-read-endpoints.md`.
- Story 03 Task 01 already added `opentk-api::router(pool)`, `opentk_api::openapi()`, `/health`, `/openapi.json`, shared `ErrorResponse`, and database-backed API integration test style.
- `opentk-db` owns PostgreSQL pool setup, generated schema metadata, sync writes, and sync status storage.
- The database schema already has direct relational tables for:
  - category progress: `sync_category`,
  - registry/current changes: `sync_entity`,
  - typed entity tables such as `document`, `activiteit`, and `persoon`,
  - generated relation tables named `<source_entity>__<relation_name>`.
- This task must not implement document text or search.

## Public HTTP Interface

- `GET /categories`
  - Returns official category metadata derived from `opentk-core::official_schema`.
  - Response shape:
    - `categories: Vec<CategoryMetadata>`
    - each category includes `category`, `table`, `field_count`, and `relation_count`.
- `GET /sync/status`
  - Returns durable sync status for every official category.
  - Response shape:
    - `categories: Vec<CategorySyncStatus>`
    - each row includes `category`, `latest_skiptoken`, `state`, `last_fetch_at`, `last_synced_at`, `caught_up_at`, `next_url`, and `resume_url`.
- `GET /changes/{category}?after={skiptoken}&limit={limit}`
  - Returns current registry changes from `sync_entity` for one category ordered by `(latest_skiptoken, source_id)`.
  - `after` is exclusive. Missing `after` starts at the beginning.
  - `limit` defaults to a conservative value such as `100` and is capped, for example at `500`.
  - Response shape:
    - `category`
    - `items: Vec<EntityChange>`
    - `next_skiptoken: Option<i64>`
    - `has_more: bool`
- `GET /entities/{category}/{source_id}?relations={none|outgoing|incoming|both}`
  - Generic detail endpoint over any official category.
  - Returns source metadata, typed scalar fields as JSON values, and optional relation summaries.
  - Unknown categories return `404` with `ErrorResponse`.
  - Missing entities return `404` with `ErrorResponse`.
- `GET /documents/{source_id}`
  - Typed document detail endpoint backed by `document`.
  - Includes source metadata, important document fields, asset metadata columns, and optional relation summaries with the same `relations` query parameter.
- `GET /activities/{source_id}`
  - Typed activity detail endpoint backed by `activiteit`.
  - Includes source metadata, activity scalar fields, and optional relation summaries.
- `GET /persons/{source_id}`
  - Typed person detail endpoint backed by `persoon`.
  - Includes source metadata, person scalar fields, and optional relation summaries.
- `GET /relations/{category}/{source_id}?direction={outgoing|incoming|both}`
  - Returns relation rows connected to a registry entity.
  - Outgoing rows scan generated relation tables where the entity is the relation source.
  - Incoming rows scan generated relation tables where the entity is the relation target.

## Database Read Boundary

- Add a new `opentk-db` module, likely `read_model`, exported from `crates/opentk-db/src/lib.rs`.
- Public read-model types should be source-facing and independent from Axum:
  - `CategoryMetadata`
  - `CategoryProgress`
  - `EntityChange`
  - `EntityDetail`
  - `DocumentDetail`
  - `ActivityDetail`
  - `PersonDetail`
  - `RelationDirection`
  - `RelationRow`
  - `ReadPage`
  - `ReadModelError`
- Public read-model functions should take `&PgPool` and typed parameters:
  - `list_category_metadata() -> Vec<CategoryMetadata>`
  - `list_category_progress(pool: &PgPool) -> Result<Vec<CategoryProgress>, ReadModelError>`
  - `list_changes(pool: &PgPool, category: &str, after: Option<i64>, limit: i64) -> Result<ReadPage<EntityChange>, ReadModelError>`
  - `get_entity_detail(pool: &PgPool, category: &str, source_id: Uuid) -> Result<EntityDetail, ReadModelError>`
  - `get_document_detail(pool: &PgPool, source_id: Uuid) -> Result<DocumentDetail, ReadModelError>`
  - `get_activity_detail(pool: &PgPool, source_id: Uuid) -> Result<ActivityDetail, ReadModelError>`
  - `get_person_detail(pool: &PgPool, source_id: Uuid) -> Result<PersonDetail, ReadModelError>`
  - `list_relations(pool: &PgPool, category: &str, source_id: Uuid, direction: RelationDirection) -> Result<Vec<RelationRow>, ReadModelError>`
- Use `opentk_db::postgres_schema::schema()` and `TableKind` to discover entity and relation tables. Do not hand-maintain separate relation table lists.
- Use `QueryBuilder` and quoted identifiers from schema-owned table/column names for dynamic table queries. Bind all runtime values.
- Return typed errors for unknown category, missing entity, invalid limit, and SQL errors. Do not swallow SQL errors.

## API Boundary

- Keep route registration, path/query extraction, HTTP status mapping, DTO serialization, and OpenAPI schemas in `opentk-api`.
- Reuse the existing `ApiState { pool }`.
- Add API query enums:
  - `RelationsQuery { relations: RelationExpansion }`
  - `ChangesQuery { after: Option<i64>, limit: Option<u32> }`
  - `RelationLookupQuery { direction: RelationDirection }`
- Map `ReadModelError::UnknownCategory` and `ReadModelError::NotFound` to `404`.
- Map validation errors to `400`.
- Map SQL errors to `500` with the existing `ErrorResponse` shape.
- Keep response DTOs close to handlers unless they become shared across endpoints; avoid a second DTO layer that only renames read-model fields.

## OpenAPI Coverage

- Extend `opentk_api::openapi()` so it includes every new route path and every response schema.
- Register schemas for:
  - category metadata/status responses,
  - change page and change item,
  - generic entity detail,
  - document/activity/person details,
  - relation lookup response and relation item,
  - query enum values where supported by `utoipa`.
- Add OpenAPI tests that assert the new paths exist and key component schemas are present.

## TDD Execution Plan

- Use the `tdd` skill during execution. Work as vertical red/green tracer bullets, not by writing every test first.
- Use database-backed integration tests with real PostgreSQL fixture rows. Prefer inserting minimal rows directly with SQL in test setup because this task verifies read behavior over the synced relational tables, not the XML parser or sync runner.
- Reuse the existing API test pattern:
  - construct a real pool from `OPENTK_TEST_DATABASE_URL` or `DATABASE_URL`,
  - migrate/test schema if an existing helper is available,
  - build `opentk_api::router(pool)`,
  - call routes through `tower::ServiceExt::oneshot`.

### Slice 1: Category Metadata

- RED: Add an API test for `GET /categories` asserting `200`, includes `Document`, `Activiteit`, and `Persoon`, and exposes their table names/counts.
- GREEN: Add `opentk-db::read_model::list_category_metadata`, API route, DTO, and OpenAPI path.

### Slice 2: Sync Status

- RED: Insert two `sync_category` rows and test `GET /sync/status` returns their state, skiptoken, and timestamps.
- GREEN: Add `list_category_progress`, API route, DTO, and OpenAPI schema.

### Slice 3: Changes Pagination

- RED: Insert several `sync_entity` rows for `Document` and test `GET /changes/Document?after=10&limit=2` returns only rows after skiptoken `10`, ordered by skiptoken, with a next cursor.
- GREEN: Add category validation, capped limit handling, `list_changes`, route, and response model.
- RED: Add a focused test that an unknown category returns a JSON `404`.
- GREEN: Map unknown category through the API error boundary.

### Slice 4: Generic Entity Detail

- RED: Insert one `sync_entity` plus one typed `document` row and test `GET /entities/Document/{id}` returns source metadata and typed scalar fields from the `document` table.
- GREEN: Add schema-driven typed entity row loading that returns scalar fields as stable JSON values.

### Slice 5: Typed Document Detail

- RED: Insert a `document` row with `document_nummer`, `titel`, `onderwerp`, `datum`, `content_type`, `content_length`, and `enclosure_url`; test `GET /documents/{id}` returns those fields plus source metadata.
- GREEN: Add typed `DocumentDetail` read-model query and route.

### Slice 6: Typed Activity Detail

- RED: Insert an `activiteit` row with representative scalar fields such as `soort`, `nummer`, `onderwerp`, `datum`, `aanvangstijd`, `locatie`, and `status`; test `GET /activities/{id}`.
- GREEN: Add typed `ActivityDetail` read-model query and route.

### Slice 7: Typed Person Detail

- RED: Insert a `persoon` row with representative scalar fields such as `nummer`, `titels`, `initialen`, `achternaam`, `tussenvoegsel`, and `roepnaam`; test `GET /persons/{id}`.
- GREEN: Add typed `PersonDetail` read-model query and route.

### Slice 8: Relation Lookup

- RED: Insert source/target `sync_entity` rows and a generated relation row, for example `document__activiteit`; test `GET /relations/Document/{id}?direction=outgoing`.
- GREEN: Add schema-driven outgoing relation scans.
- RED: Test incoming lookup from the target entity returns the same relation.
- GREEN: Add schema-driven incoming relation scans and the `both` direction.

### Slice 9: Relation Expansion Controls

- RED: Test `GET /entities/Document/{id}?relations=outgoing` includes outgoing relations and `relations=none` omits them.
- GREEN: Wire relation expansion into generic and typed detail routes.

### Slice 10: OpenAPI

- RED: Extend the OpenAPI test to assert all new paths and key schemas exist.
- GREEN: Register paths/schemas in `openapi()`.

## Boundary Cleanup Plan

- Use `improve-code-boundaries` during final review.
- Keep dynamic SQL identifier quoting in one `opentk-db` helper; do not duplicate formatting in API handlers.
- Remove any DTOs that only wrap a read-model type without changing HTTP behavior.
- Keep category and table discovery schema-driven through `opentk-core`/`opentk-db`; do not introduce a hard-coded endpoint category registry except for the intentionally typed document/activity/person routes.
- If repeated scalar loading makes the generic entity endpoint muddy, keep Task 02 scoped to typed table scalar columns and add a bug/task for repeated scalars instead of silently ignoring an error path.
- If a required generated table or column is absent, return a real error and add an `add-bug` task. Do not swallow or default missing data.

## Verification

- During execution, record RED failures and GREEN passes in the progress log after each meaningful slice.
- Required final checks:
  - `make check`
  - `make test`
  - `make lint`
- Do not run `make test-long` for this task unless the task file changes to explicitly require story-end or e2e validation.
- After checks pass, do a final `improve-code-boundaries` review and fix any boundary mud before setting `<passes>true</passes>`.

## Completion Steps

- Tick the task acceptance checkboxes after their implementation and verification are complete.
- Set `<passes>true</passes>` only after `make check`, `make test`, and `make lint` pass.
- Run `/bin/bash .ralph/task_switch.sh`.
- Add and commit all changed files, including `.ralph`, with message `task finished task-02-implement-core-read-endpoints: ...`.
- Push the commit.

NOW EXECUTE
