## Task: Story 5 Task 2 - Design Search Indexing Pipeline <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Design the indexing pipeline for the selected search engine. The design must cover documents, extracted text, stored HTML, entity metadata, updates, deletes, backfill indexing, incremental indexing, failure recovery, and API result shapes.

In scope: index schemas, source-to-index mappings, update cursor/state tables if needed, retry/error handling, storage estimates, and tests for mapping correctness. Out of scope: final endpoint implementation.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: mapping tests fail when a document/entity fixture misses an expected indexed field, then pass.
- [ ] Index schema covers documents, extracted text, stored HTML, and core entity metadata.
- [ ] Update/delete propagation is designed and tested with fixtures.
- [ ] Backfill and incremental indexing paths are specified.
- [ ] Failure recovery and retry behavior are specified.
- [ ] Storage cost estimate is updated from real investigation data.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
