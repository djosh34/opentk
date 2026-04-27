## Task: Story 03 Task 03 - Deep Verify HTTP API <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Prove the HTTP API is correct, documented, and directly backed by the synced PostgreSQL database. Verification must start a test server, seed or sync a PostgreSQL fixture database, call every endpoint, validate response bodies, validate OpenAPI completeness, and validate pagination/relation behavior.

In scope: end-to-end HTTP tests, OpenAPI validation, response schema checks, database query cross-checks, pagination edge cases, error cases, and concurrency smoke tests. Out of scope: document text extraction and search.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add an end-to-end API test that fails on missing OpenAPI coverage or response mismatch, then make it pass.
- [x] Every implemented endpoint is exercised through HTTP.
- [x] Every response is checked against database source rows.
- [x] OpenAPI spec includes all routes, parameters, and response schemas.
- [x] Cursor pagination is tested for first page, next page, empty page, and invalid cursor.
- [x] Error responses are tested for invalid IDs and invalid parameters.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only) — not applicable; this task does not affect ultra-long/e2e selection and per task instructions the long lane was not run.
</acceptance_criteria>

<plan>
.ralph/tasks/story-03-http-api/task-03-deep-verify-http-api_plans/deep-verify-http-api-plan.md
</plan>

NOW EXECUTE
