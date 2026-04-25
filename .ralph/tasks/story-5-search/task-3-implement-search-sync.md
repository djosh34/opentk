## Task: Story 5 Task 3 - Implement Search Sync Pipeline <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the indexing and update pipeline for the selected search engine. The pipeline must build the index from PostgreSQL, apply incremental updates, handle deletes/replace updates, persist indexing progress, and report failures explicitly.

In scope: engine client integration, batch indexing, incremental indexing, retry/error handling, progress tables or equivalent state, operational commands, and tests. Out of scope: HTTP search endpoint.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: integration test fails when a PostgreSQL update is not reflected in the search index, then passes.
- [ ] Full reindex command builds an index from PostgreSQL.
- [ ] Incremental index command applies created, updated, and deleted records.
- [ ] Indexing progress is durable.
- [ ] Failures are explicit and retryable.
- [ ] Tests cover documents, extracted text, stored HTML, and entity metadata.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
