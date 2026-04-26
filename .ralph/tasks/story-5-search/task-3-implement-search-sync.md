## Task: Story 5 Task 3 - Implement Search Sync Pipeline <status>completed</status> <passes>true</passes>

<plan>
.ralph/tasks/story-5-search/task-3-implement-search-sync_plans/search-sync-pipeline-plan.md
</plan>

NOW EXECUTE

<description>
Must use tdd skill to complete


**Goal:** Implement the indexing and update pipeline for the selected search engine. The pipeline must build the index from PostgreSQL, apply incremental updates, handle deletes/replace updates, persist indexing progress, and report failures explicitly.

In scope: engine client integration, batch indexing, incremental indexing, retry/error handling, progress tables or equivalent state, operational commands, and tests. Out of scope: HTTP search endpoint.

</description>


<acceptance_criteria>
- [x] Red/green TDD: integration test fails when a PostgreSQL update is not reflected in the search index, then passes.
- [x] Full reindex command builds an index from PostgreSQL.
- [x] Incremental index command applies created, updated, and deleted records.
- [x] Indexing progress is durable.
- [x] Failures are explicit and retryable.
- [x] Tests cover documents, extracted text, stored HTML, and entity metadata.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — not impacted and intentionally not run for this non-story-finishing task
</acceptance_criteria>
