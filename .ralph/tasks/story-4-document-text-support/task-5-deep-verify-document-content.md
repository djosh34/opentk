## Task: Story 4 Task 5 - Deep Verify Document Content Extraction <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Prove official text/HTML preference, document content fetching, binary fallback extraction, storage, and HTTP exposure work correctly end to end. Verification must include fixture documents, live controlled samples, exact output comparisons for supported formats, database integrity checks, HTTP response checks, retry behavior, and storage measurement.

The quality bar is exact matching for supported official-source and extraction fixtures. Official text/HTML/transcript versions from Tweede Kamer or other official government sources must be preferred when available. Any word mismatch, missing text, ordering error, encoding error, or layout artifact in supported extraction output is a failing test. Live samples must be recorded with hashes and expected outputs when promoted to fixtures.

In scope: end-to-end test pipeline, golden fixture verification, live smoke tests in `make test-long`, extraction provenance checks, HTTP checks, storage reports, and retry/error behavior. Out of scope: search.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: end-to-end fixture test fails on a one-word mismatch, then passes with exact extraction.
- [ ] End-to-end test proves official text/HTML/transcript source wins over binary extraction when both exist.
- [ ] End-to-end test fetches, classifies, extracts, stores, and serves PDF fixture content exactly.
- [ ] End-to-end test fetches, classifies, extracts, stores, and serves DOCX fixture content exactly.
- [ ] End-to-end test stores and serves HTML fixture content exactly.
- [ ] Live controlled samples run under `make test-long` and record hashes/provenance.
- [ ] Storage report separates links/metadata, extracted text, stored HTML, and indexes/constraints.
- [ ] API responses are checked against PostgreSQL rows.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
