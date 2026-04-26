## Task: Story 3 Task 1 - Add HTTP Server and OpenAPI Spec <status>not_started</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a simple Rust HTTP API server using an established Rust library and generate/serve an OpenAPI specification. Use `axum` for HTTP and an OpenAPI library such as `utoipa` or another maintained Rust OpenAPI library selected during implementation. The API must connect to the PostgreSQL database through `sqlx`.

In scope: server crate/module, configuration, database pool setup, health endpoint, OpenAPI generation, OpenAPI serving endpoint, request/response error shape, and tests. Out of scope: document text extraction and advanced search.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add an HTTP integration test for `/health` and OpenAPI document serving before implementation, then make it pass.
- [x] API starts with configurable bind address and PostgreSQL connection string.
- [x] OpenAPI spec is generated from Rust endpoint/type definitions or kept in sync through automated tests.
- [x] Error responses use a consistent JSON shape.
- [x] Database pool initialization is tested.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only; not run because this task did not impact long-test selection and is not story-ending)
</acceptance_criteria>

<plan>
.ralph/tasks/story-3-http-api/task-1-add-http-server-and-openapi_plans/http-server-openapi-plan.md

NOW EXECUTE
</plan>
