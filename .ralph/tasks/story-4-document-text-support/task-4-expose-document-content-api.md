## Task: Story 4 Task 4 - Expose Document Content API <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Expose stored document text and HTML through the HTTP API and OpenAPI spec. The API must return asset metadata, content source type, official-source indicator, extraction status, extracted text, stored HTML where available, provenance, and hashes. Responses must be backed directly by PostgreSQL rows from the document content schema.

In scope: endpoints, response models, OpenAPI updates, authorization-free local API behavior matching the rest of the project, and database-backed HTTP tests. Out of scope: search.

</description>


<acceptance_criteria>
- [x] Red/green TDD: HTTP tests for document content endpoints fail before implementation, then pass.
- [x] API returns extracted text for documents with completed extraction.
- [x] API returns stored HTML for HTML assets.
- [x] API returns content source type, official-source indicator, extraction status, and provenance.
- [x] API returns clear errors/status for missing or failed extraction.
- [x] OpenAPI spec includes document content endpoints and schemas.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only) — not impacted; not run per task instructions.
</acceptance_criteria>

<plan>
.ralph/tasks/story-4-document-text-support/task-4-expose-document-content-api_plans/document-content-api-plan.md
</plan>

DONE
