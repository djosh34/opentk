## Task: Story 5 Task 4 - Expose Search API <status>done</status> <passes>true</passes>

<plan>
.ralph/tasks/story-5-search/task-4-expose-search-api_plans/search-api-plan.md
</plan>

NOW EXECUTE

<description>
Must use tdd skill to complete


**Goal:** Expose fuzzy search through the HTTP API and OpenAPI spec. The endpoint must support instant fuzzy search over documents, extracted document text, stored HTML, and core entity metadata. Results must include entity type, entity id, title/label, snippet/highlight data when supported by the engine, score/ranking info when available, and source URLs for API follow-up.

In scope: HTTP endpoint, query parameters, response types, OpenAPI updates, error handling, pagination/limit behavior, and integration tests. Out of scope: choosing the engine; that is completed by the investigation task.

</description>


<acceptance_criteria>
- [x] Red/green TDD: HTTP search tests fail before endpoint implementation, then pass.
- [x] Search endpoint returns document results.
- [x] Search endpoint returns entity metadata results.
- [x] Fuzzy typo queries return expected relevant results.
- [x] Response includes enough data for clients to fetch the full entity/document.
- [x] OpenAPI spec covers search endpoint parameters and response schema.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — not impacted by this task; not run.
</acceptance_criteria>
