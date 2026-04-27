## Task: Story 04 Task 01 - Design Document Content Schema <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add PostgreSQL schema for document asset retrieval status, official text/HTML source selection, and extracted content. The main database must always keep the upstream link and metadata. Official text/HTML from Tweede Kamer or other official government sources must be preferred when available. Extracted HTML/text must be stored in PostgreSQL with provenance, extraction tool version, content hash, source content type, source content length, extraction timestamp, official-source indicator, and validation status.

In scope: migrations, indexes for document id and asset URL, constraints, extraction status enum/table, and tests. Out of scope: fetching and parsing actual documents.

</description>


<acceptance_criteria>
- [x] Red/green TDD: schema test fails when extraction provenance/status/hash columns are missing, then passes.
- [x] Schema stores upstream asset link and metadata.
- [x] Schema stores official text/HTML source metadata and extracted text/HTML where available.
- [x] Schema stores extraction provenance and validation status.
- [x] Indexes support document-id and asset-url lookups.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): not impacted; `make test-long` not required for this non-story-finishing schema task
</acceptance_criteria>

<plan>
.ralph/tasks/story-04-document-text-support/task-01-design-document-content-schema_plans/document-content-schema-plan.md
</plan>

NOW EXECUTE
