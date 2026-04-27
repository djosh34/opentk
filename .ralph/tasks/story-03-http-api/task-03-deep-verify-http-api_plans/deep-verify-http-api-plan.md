# Story 03 Task 03 Plan: Deep Verify HTTP API

## Context Read

- Active task: `.ralph/tasks/story-03-http-api/task-03-deep-verify-http-api.md`.
- Story 03 Task 02 already added the implemented HTTP surface:
  - `GET /health`
  - `GET /openapi.json`
  - `GET /categories`
  - `GET /sync/status`
  - `GET /changes/{category}`
  - `GET /entities/{category}/{source_id}`
  - `GET /documents/{source_id}`
  - `GET /activities/{source_id}`
  - `GET /persons/{source_id}`
  - `GET /relations/{category}/{source_id}`
- Existing API tests mostly call `opentk_api::router(pool)` directly through Tower. This task must deepen verification by starting a real listener and calling the API through HTTP.
- Existing helper setup creates isolated PostgreSQL schemas, migrates them, seeds direct fixture rows, and uses real `PgPool` connections. Keep that database-backed approach and extract it if duplication starts muddying the API tests.

## Public Verification Boundary

- Add a new end-to-end API test file, likely `crates/opentk-api/tests/deep_verify_http_api.rs`.
- The test public interface should be real HTTP requests against a started test server:
  - create an isolated migrated PostgreSQL schema,
  - seed PostgreSQL fixture rows directly into synced tables,
  - bind `TcpListener` to `127.0.0.1:0`,
  - serve `opentk_api::router(pool)` on a background task,
  - use an HTTP client such as `reqwest` to call endpoints by URL,
  - abort the server task at test end.
- Add `reqwest` only as a dev-dependency for `opentk-api` if no existing HTTP client is available.
- Keep behavior assertions through public HTTP responses. Do not test handler internals.

## Fixture Design

- Seed one cohesive fixture database containing:
  - `sync_category` rows for at least `Document`, `Activiteit`, and `Persoon`;
  - `sync_entity` rows for multiple `Document` records with ordered skip tokens;
  - one `Activiteit` and one `Persoon`;
  - typed rows in `document`, `activiteit`, and `persoon`;
  - at least one generated relation row such as `document__activiteit`.
- Use stable UUID constants and source timestamps so HTTP bodies can be compared directly to database source rows.
- For every endpoint that returns database data, fetch the source row from PostgreSQL in the test and assert the HTTP body matches those values. This is the core acceptance criterion for direct database backing.

## TDD Execution Plan

- Use the `tdd` skill during execution and work in vertical red/green slices. Do not write a broad test matrix first.
- Each slice starts with one failing behavior test, then just enough code or test support to make it pass.
- Record meaningful RED and GREEN results in the progress log.

### Slice 1: Real HTTP Server Tracer Bullet

- RED: Add a single test that starts the server on an ephemeral port, calls `GET /health` through real HTTP, and expects `200` with `{ "status": "ok" }`.
- GREEN: Add the test server helper and dev-dependency/client setup needed to pass.
- Boundary check: if the helper duplicates migration/schema setup from `core_read_endpoints.rs`, extract a shared integration-test support module under `crates/opentk-api/tests/support/` rather than copying more bootstrap code.

### Slice 2: Endpoint Inventory and OpenAPI Path Coverage

- RED: Add an OpenAPI assertion that every implemented route in `router()` appears in `/openapi.json`.
- GREEN: Fix any missing OpenAPI path registration.
- The expected path inventory is:
  - `/health`
  - `/openapi.json`
  - `/categories`
  - `/sync/status`
  - `/changes/{category}`
  - `/entities/{category}/{source_id}`
  - `/documents/{source_id}`
  - `/activities/{source_id}`
  - `/persons/{source_id}`
  - `/relations/{category}/{source_id}`

### Slice 3: OpenAPI Parameters and Response Schemas

- RED: Extend OpenAPI tests to assert path parameters and query parameters are documented for:
  - `category`
  - `source_id`
  - `after`
  - `limit`
  - `relations`
  - `direction`
- RED: Assert success and error response schemas exist where applicable:
  - `HealthResponse`
  - `ErrorResponse`
  - `CategoryMetadataResponse`
  - `SyncStatusResponse`
  - `ChangePageResponse`
  - `EntityDetailResponse`
  - `RelationLookupResponse`
- GREEN: Add missing manual `utoipa` parameter and response metadata.
- Design constraint: keep OpenAPI construction in `opentk-api`; do not leak HTTP schema concerns into `opentk-db`.

### Slice 4: Database-Backed Core Endpoint Bodies

- RED: Through real HTTP, call `/categories`, `/sync/status`, `/entities/Document/{id}`, `/documents/{id}`, `/activities/{id}`, `/persons/{id}`, and `/relations/Document/{id}` against the seeded fixture.
- RED: For each response, cross-check the body against PostgreSQL rows queried in the test.
- GREEN: Fix any response mapping or serialization mismatch.
- Scope discipline: response body verification can be grouped in one cohesive fixture test only after the tracer bullet helper exists; keep assertions behavior-oriented and avoid inspecting private Rust types.

### Slice 5: Cursor Pagination Edge Cases

- RED: Add one pagination test over `/changes/Document` that covers:
  - first page with a small `limit`,
  - next page using the returned `next_skiptoken`,
  - empty page after the final skiptoken,
  - invalid cursor/limit parameters returning JSON `400`.
- GREEN: Fix query parsing, limit validation, or error mapping as needed.
- Expected behavior:
  - first page returns ordered rows and `has_more: true` when more rows exist,
  - next page starts after the exclusive cursor,
  - final/empty page returns no items, `has_more: false`, and no `next_skiptoken`,
  - invalid `limit`, including zero or above the cap, returns `invalid_request`.

### Slice 6: Error Cases

- RED: Add focused HTTP tests for:
  - invalid UUID path value returns `400` JSON error,
  - unknown category returns `404` JSON error,
  - missing valid entity ID returns `404` JSON error,
  - invalid `relations` or `direction` enum value returns `400` JSON error.
- GREEN: Add or fix Axum rejection handling if extractor rejections are currently not normalized into the API `ErrorResponse` shape.
- Boundary constraint: do not swallow extractor or database errors. Convert them deliberately into the documented JSON error contract.

### Slice 7: Relation Expansion Behavior

- RED: Through real HTTP, verify:
  - `relations=none` omits the `relations` field,
  - `relations=outgoing` returns outgoing relation rows,
  - `relations=incoming` returns incoming relation rows for the target,
  - `relations=both` returns both directions when both exist in the fixture.
- GREEN: Fix expansion mapping or relation scans as needed.

### Slice 8: Concurrency Smoke

- RED: Add a small concurrency smoke test that starts one server and concurrently calls representative read endpoints:
  - `/health`
  - `/categories`
  - `/changes/Document?limit=1`
  - `/documents/{id}`
  - `/relations/Document/{id}?direction=outgoing`
- GREEN: Fix any pool, server, or helper lifecycle issue.
- Keep this smoke test modest; it is not a load test and must remain in the default `make test` lane.

## Boundary Cleanup Plan

- Use `improve-code-boundaries` during execution and final review.
- Prefer a shared API integration test support module if setup is duplicated:
  - isolated migrated PostgreSQL schema creation,
  - quoted schema identifier helper,
  - test database URL search-path helper,
  - HTTP test server startup,
  - JSON GET helper,
  - fixture seeding helpers.
- Keep production route/DTO/read-model boundaries clean:
  - `opentk-api` owns HTTP extraction, error mapping, OpenAPI metadata, and DTO serialization;
  - `opentk-db::read_model` owns SQL reads and schema-driven table discovery;
  - test support owns fixture/bootstrap mechanics.
- If response DTOs and read-model types become duplicate shapes with no HTTP-specific value, flatten the conversion.
- If OpenAPI path registration repeats enough stringly metadata to become brittle, introduce a small local helper in `opentk-api` for path parameters/query parameters rather than scattering JSON snippets in tests.
- If invalid query/path rejection handling requires a new error layer, keep it at the API boundary and cover it through HTTP tests.

## Verification

- Required final checks:
  - `make check`
  - `make test`
  - `make lint`
- Do not run `make test-long` for this task unless the task file is changed to make this a story-ending/e2e validation task.
- Before setting `<passes>true</passes>`, do a final `improve-code-boundaries` review:
  - no copied test bootstrap that should be shared,
  - no ignored errors,
  - no undocumented implemented routes,
  - no response data that is asserted without a database cross-check.

## Completion Steps

- Tick the task acceptance checkboxes only after implementation and verification.
- Set `<passes>true</passes>` only after `make check`, `make test`, and `make lint` pass.
- Run `/bin/bash .ralph/task_switch.sh`.
- Add and commit all changed files, including `.ralph`, with message `task finished task-03-deep-verify-http-api: ...`.
- Push the commit.

NOW EXECUTE
