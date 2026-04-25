## Task: Story 4 Task 2 - Fetch and Classify Document Assets <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement document asset fetching and classification for linked Tweede Kamer assets. The fetcher must retrieve metadata, discover official text/HTML/transcript alternatives from Tweede Kamer or other official government sources where available, classify content type, validate content length/hash, store retrieval status, and retain the upstream link in PostgreSQL. Source binary files such as PDFs and DOCX files are processing inputs only when an official text/HTML source is unavailable; official or extracted text/HTML is the persisted content.

In scope: HTTP asset fetcher, content type classification, size checks, hash checks, retry/error handling, retrieval status writes, and tests with local fixture server. Out of scope: text extraction implementation.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: fixture-server tests fail on wrong content-type, length, hash, or status handling, then pass.
- [ ] Fetcher stores upstream URL and metadata.
- [ ] Fetcher discovers and records official text/HTML/transcript alternatives when present.
- [ ] Fetcher prefers official text/HTML/transcript alternatives over binary extraction inputs.
- [ ] Fetcher validates content length when available.
- [ ] Fetcher records failures durably with enough detail to retry.
- [ ] Fetcher supports bounded concurrency and timeout configuration.
- [ ] Tests cover PDF, DOCX, HTML, missing content length, wrong length, and HTTP error responses.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
