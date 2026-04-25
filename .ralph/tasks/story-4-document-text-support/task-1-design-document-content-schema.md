## Task: Story 4 Task 1 - Design Document Content Schema <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add PostgreSQL schema for document asset retrieval status, official text/HTML source selection, and extracted content. The main database must always keep the upstream link and metadata. Official text/HTML from Tweede Kamer or other official government sources must be preferred when available. Extracted HTML/text must be stored in PostgreSQL with provenance, extraction tool version, content hash, source content type, source content length, extraction timestamp, official-source indicator, and validation status.

In scope: migrations, indexes for document id and asset URL, constraints, extraction status enum/table, and tests. Out of scope: fetching and parsing actual documents.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: schema test fails when extraction provenance/status/hash columns are missing, then passes.
- [ ] Schema stores upstream asset link and metadata.
- [ ] Schema stores official text/HTML source metadata and extracted text/HTML where available.
- [ ] Schema stores extraction provenance and validation status.
- [ ] Indexes support document-id and asset-url lookups.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
