# Story 03 Task 01 Plan: HTTP Server and OpenAPI

## Context Read

- Active task: `.ralph/tasks/story-03-http-api/task-01-add-http-server-and-openapi.md`.
- Existing workspace has `crates/opentk-api`, currently only a placeholder `lib.rs`.
- Existing `opentk-db` crate owns SQLx/PostgreSQL-facing code and should remain the database boundary.
- Workspace dependencies already include `axum`, `sqlx`, `tokio`, `serde`, `serde_json`, `thiserror`, `clap`, `tracing`, and `tracing-subscriber`.
- Need to add a maintained OpenAPI library. Prefer `utoipa` plus `utoipa-swagger-ui` only if serving UI is useful; otherwise serve the generated JSON document directly from `utoipa`.

## Interface Design

- Add `opentk_api::ApiConfig`:
  - `bind_address: std::net::SocketAddr`
  - `database_url: String`
- Add `opentk_api::serve(config: ApiConfig) -> Result<(), ApiError>`:
  - Builds the database pool.
  - Builds the Axum router.
  - Binds and serves on the configured address.
- Add `opentk_api::router(pool: sqlx::PgPool) -> axum::Router`:
  - Public testable app constructor.
  - No hidden global state.
- Add `opentk_api::openapi() -> utoipa::openapi::OpenApi`:
  - Public testable spec constructor.
- Add `opentk_api::ApiError` and `opentk_api::ErrorResponse`:
  - JSON error shape:
    - `code: String`
    - `message: String`
  - Axum error responses should use this shape consistently.
- Add `opentk-db` pool boundary:
  - `opentk_db::DatabaseConfig { url: String, max_connections: u32 }`
  - `opentk_db::connect(config: &DatabaseConfig) -> Result<PgPool, DatabaseError>`
  - `opentk_db::migrate(pool: &PgPool) -> Result<(), DatabaseError>` only if API startup should apply migrations. If added, tests must prove the behavior.
- Add `crates/opentk-api/src/bin/opentk-api.rs`:
  - CLI args/env via `clap`.
  - `--bind-address` / `OPENTK_API_BIND_ADDRESS`.
  - `--database-url` / `DATABASE_URL` or `OPENTK_DATABASE_URL`.

## HTTP Surface

- `GET /health`
  - Returns HTTP 200.
  - JSON body includes at least `status: "ok"`.
  - Performs a cheap DB reachability check through SQLx, such as `SELECT 1`, so the endpoint proves the API is connected to PostgreSQL.
- `GET /openapi.json`
  - Returns the generated OpenAPI JSON.
  - Must include `/health` and `/openapi.json` paths.
  - Must describe the health response and shared error response shape.

## TDD Execution Plan

- Use the `tdd` skill during execution. Work in vertical slices, one behavior test at a time.
- RED 1: Add an `opentk-api` integration test that constructs a real PostgreSQL pool from `OPENTK_TEST_DATABASE_URL`, builds `router(pool)`, calls `GET /health`, and asserts `200` plus `{"status":"ok"}`.
- GREEN 1: Add minimal `opentk-api` dependencies, router, health handler, and response DTO to pass.
- RED 2: Add an integration test that calls `GET /openapi.json` through the router and asserts the JSON has `openapi`, `paths./health`, and `paths./openapi.json`.
- GREEN 2: Add `utoipa` OpenAPI generation from Rust endpoint/type definitions and serve it from `/openapi.json`.
- RED 3: Add a DB pool initialization test in `opentk-db` that uses `OPENTK_TEST_DATABASE_URL`, calls the new `connect` boundary, and proves a simple query succeeds through the returned pool.
- GREEN 3: Add `DatabaseConfig`, `DatabaseError`, and `connect` in `opentk-db`.
- RED 4: Add an API startup/config test that proves `ApiConfig` carries a bind address and database URL into pool/router construction without starting a long-running server.
- GREEN 4: Add `serve` and CLI binary wiring. Keep long-running bind behavior out of unit tests; test the public construction pieces and let compile/lint cover the binary entrypoint.
- RED 5: Add an error-shape test using a deliberate failing route or invalid DB pool path only if it can be exercised through public HTTP behavior without brittle internals.
- GREEN 5: Implement `ApiError` to `IntoResponse` with consistent JSON. If no existing endpoint can produce an HTTP error without fake internals, add this as a focused public unit test for `ApiError::into_response`.

## Boundary Cleanup Plan

- Use `improve-code-boundaries` during final review.
- Keep all HTTP DTOs, route handlers, OpenAPI annotations, and server bootstrap in `opentk-api`.
- Keep SQLx pool construction in `opentk-db`; do not duplicate `PgPoolOptions` setup in `opentk-api`.
- Avoid a separate generated/static OpenAPI JSON file. The Rust definitions are the source of truth; tests assert the served spec includes registered routes.
- Keep CLI parsing in the binary only. Library APIs should accept typed config, not environment variables.
- Remove the placeholder `Future HTTP API boundary` wording once real API code exists.

## Verification

- During implementation, run each new failing test before writing its slice when practical and record the RED result in progress.
- Required final checks:
  - `make check`
  - `make test`
  - `make lint`
- Do not run `make test-long` for this task unless implementation unexpectedly changes long-test selection or the task file is expanded to require it.
- After checks pass, run a final `improve-code-boundaries` review:
  - verify HTTP, DB, CLI, and OpenAPI responsibilities are not muddled;
  - flatten any duplicated config/error shapes;
  - remove any legacy placeholder docs/code created obsolete by the implementation.

## Completion Steps

- Tick the task acceptance checkboxes.
- Set `<passes>true</passes>` in the task file only after `make check`, `make test`, and `make lint` pass.
- Run `/bin/bash .ralph/task_switch.sh`.
- Add and commit all changed files, including `.ralph`, with message `task finished task-01-add-http-server-and-openapi: ...`.
- Push the commit.

NOW EXECUTE
