## Task: Story 3 Task 1 - Add HTTP Server and OpenAPI Spec <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a simple Rust HTTP API server using an established Rust library and generate/serve an OpenAPI specification. Use `axum` for HTTP and an OpenAPI library such as `utoipa` or another maintained Rust OpenAPI library selected during implementation. The API must connect to the PostgreSQL database through `sqlx`.

In scope: server crate/module, configuration, database pool setup, health endpoint, OpenAPI generation, OpenAPI serving endpoint, request/response error shape, and tests. Out of scope: document text extraction and advanced search.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: add an HTTP integration test for `/health` and OpenAPI document serving before implementation, then make it pass.
- [ ] API starts with configurable bind address and PostgreSQL connection string.
- [ ] OpenAPI spec is generated from Rust endpoint/type definitions or kept in sync through automated tests.
- [ ] Error responses use a consistent JSON shape.
- [ ] Database pool initialization is tested.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
