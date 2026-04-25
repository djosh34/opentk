## Task: Story 3 Task 3 - Deep Verify HTTP API <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Prove the HTTP API is correct, documented, and directly backed by the synced PostgreSQL database. Verification must start a test server, seed or sync a PostgreSQL fixture database, call every endpoint, validate response bodies, validate OpenAPI completeness, and validate pagination/relation behavior.

In scope: end-to-end HTTP tests, OpenAPI validation, response schema checks, database query cross-checks, pagination edge cases, error cases, and concurrency smoke tests. Out of scope: document text extraction and search.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: add an end-to-end API test that fails on missing OpenAPI coverage or response mismatch, then make it pass.
- [ ] Every implemented endpoint is exercised through HTTP.
- [ ] Every response is checked against database source rows.
- [ ] OpenAPI spec includes all routes, parameters, and response schemas.
- [ ] Cursor pagination is tested for first page, next page, empty page, and invalid cursor.
- [ ] Error responses are tested for invalid IDs and invalid parameters.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
