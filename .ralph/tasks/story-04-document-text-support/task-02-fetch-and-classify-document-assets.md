## Task: Story 04 Task 02 - Fetch and Classify Document Assets <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement document asset fetching and classification for linked Tweede Kamer assets. The fetcher must retrieve metadata, discover official text/HTML/transcript alternatives from Tweede Kamer or other official government sources where available, classify content type, validate content length/hash, store retrieval status, and retain the upstream link in PostgreSQL. Source binary files such as PDFs and DOCX files are processing inputs only when an official text/HTML source is unavailable; official or extracted text/HTML is the persisted content.

In scope: HTTP asset fetcher, content type classification, size checks, hash checks, retry/error handling, retrieval status writes, and tests with local fixture server. Out of scope: text extraction implementation.

</description>


<acceptance_criteria>
- [x] Red/green TDD: fixture-server tests fail on wrong content-type, length, hash, or status handling, then pass.
- [x] Fetcher stores upstream URL and metadata.
- [x] Fetcher discovers and records official text/HTML/transcript alternatives when present.
- [x] Fetcher prefers official text/HTML/transcript alternatives over binary extraction inputs.
- [x] Fetcher validates content length when available.
- [x] Fetcher records failures durably with enough detail to retry.
- [x] Fetcher supports bounded concurrency and timeout configuration.
- [x] Tests cover PDF, DOCX, HTML, missing content length, wrong length, and HTTP error responses.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): not applicable; this is not the story-ending deep verification task and did not require `make test-long`.
</acceptance_criteria>

<plan>
.ralph/tasks/story-04-document-text-support/task-02-fetch-and-classify-document-assets_plans/document-asset-fetch-and-classify-plan.md
</plan>

NOW EXECUTE
