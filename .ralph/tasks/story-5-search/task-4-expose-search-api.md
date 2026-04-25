## Task: Story 5 Task 4 - Expose Search API <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Expose fuzzy search through the HTTP API and OpenAPI spec. The endpoint must support instant fuzzy search over documents, extracted document text, stored HTML, and core entity metadata. Results must include entity type, entity id, title/label, snippet/highlight data when supported by the engine, score/ranking info when available, and source URLs for API follow-up.

In scope: HTTP endpoint, query parameters, response types, OpenAPI updates, error handling, pagination/limit behavior, and integration tests. Out of scope: choosing the engine; that is completed by the investigation task.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: HTTP search tests fail before endpoint implementation, then pass.
- [ ] Search endpoint returns document results.
- [ ] Search endpoint returns entity metadata results.
- [ ] Fuzzy typo queries return expected relevant results.
- [ ] Response includes enough data for clients to fetch the full entity/document.
- [ ] OpenAPI spec covers search endpoint parameters and response schema.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
