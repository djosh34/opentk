## Task: Story 3 Task 2 - Implement Core Read Endpoints <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the first read endpoints over the synced PostgreSQL tables. The API must expose category metadata, sync status, changes by category/skiptoken, entity detail, document detail, activity detail, person detail, and relation lookups. Responses must come directly from PostgreSQL tables produced by the sync story.

In scope: endpoint handlers, typed response models, pagination by cursor/skiptoken, source metadata in responses, relation expansion controls, OpenAPI coverage, and database-backed tests. Out of scope: document text endpoint and search.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add failing endpoint tests backed by fixture PostgreSQL rows, then make them pass.
- [x] Endpoints expose category progress and sync status.
- [x] Changes endpoint pages by category and skiptoken.
- [x] Entity detail endpoint returns source metadata and typed fields.
- [x] Document, activity, and person endpoints return database-backed data.
- [x] Relation lookup endpoint supports outgoing and incoming relations.
- [x] OpenAPI spec includes every implemented endpoint and response schema.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only) — not applicable; this task does not affect ultra-long/e2e selection and per task instructions the long lane was not run.
</acceptance_criteria>

<plan>
.ralph/tasks/story-3-http-api/task-2-implement-core-read-endpoints_plans/core-read-endpoints-plan.md
</plan>

NOW EXECUTE
