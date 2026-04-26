## Task: Story 5 Task 2 - Design Search Indexing Pipeline <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Design the indexing pipeline for the selected search engine. The design must cover documents, extracted text, stored HTML, entity metadata, updates, deletes, backfill indexing, incremental indexing, failure recovery, and API result shapes.

In scope: index schemas, source-to-index mappings, update cursor/state tables if needed, retry/error handling, storage estimates, and tests for mapping correctness. Out of scope: final endpoint implementation.

</description>


<acceptance_criteria>
- [x] Red/green TDD: mapping tests fail when a document/entity fixture misses an expected indexed field, then pass.
- [x] Index schema covers documents, extracted text, stored HTML, and core entity metadata.
- [x] Update/delete propagation is designed and tested with fixtures.
- [x] Backfill and incremental indexing paths are specified.
- [x] Failure recovery and retry behavior are specified.
- [x] Storage cost estimate is updated from real investigation data.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): not applicable; this task did not change long/e2e selection, and `make test-long` was intentionally not run.
</acceptance_criteria>

<plan>
.ralph/tasks/story-5-search/task-2-design-search-indexing_plans/search-indexing-pipeline-plan.md
</plan>

NOW EXECUTE
