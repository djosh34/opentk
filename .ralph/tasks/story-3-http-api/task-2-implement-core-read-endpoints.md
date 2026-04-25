## Task: Story 3 Task 2 - Implement Core Read Endpoints <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the first read endpoints over the synced PostgreSQL tables. The API must expose category metadata, sync status, changes by category/skiptoken, entity detail, document detail, activity detail, person detail, and relation lookups. Responses must come directly from PostgreSQL tables produced by the sync story.

In scope: endpoint handlers, typed response models, pagination by cursor/skiptoken, source metadata in responses, relation expansion controls, OpenAPI coverage, and database-backed tests. Out of scope: document text endpoint and search.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: add failing endpoint tests backed by fixture PostgreSQL rows, then make them pass.
- [ ] Endpoints expose category progress and sync status.
- [ ] Changes endpoint pages by category and skiptoken.
- [ ] Entity detail endpoint returns source metadata and typed fields.
- [ ] Document, activity, and person endpoints return database-backed data.
- [ ] Relation lookup endpoint supports outgoing and incoming relations.
- [ ] OpenAPI spec includes every implemented endpoint and response schema.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
