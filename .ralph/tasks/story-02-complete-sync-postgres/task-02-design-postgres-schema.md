## Task: Story 02 Task 02 - Design Complete PostgreSQL Schema <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create PostgreSQL migrations that model the entire official information model directly in database tables. Every official entity type must have a table or formally generated table representation. Every official relation must be represented with foreign-key-capable columns or join tables according to multiplicity. Every table must include source metadata needed for SyncFeed cursor correctness and update replacement.

The chosen database is PostgreSQL. Use `sqlx` migrations. JSONB may be used only for source-adjacent fields whose shape is officially flexible or still settling, while all official fields from the XSD/informatiemodel must be represented directly in the schema. Fields that are frequently queried or joined must be regular typed columns. Relations must be queryable without application-side document parsing.

In scope: migrations, indexes, constraints, source metadata columns, relationship tables, generated schema tests, and documentation explaining the schema conventions. Include indexes for common joins and sync queries: primary UUID lookups, category cursor progress, entity update timestamps, document numbers, dates, relation endpoints, asset owner lookups, and foreign-key paths. Out of scope: HTTP API implementation.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add schema tests that fail when an official entity, field, relation, primary key, or required index is missing, then make them pass.
- [x] Every official entity type has a PostgreSQL table or generated table representation.
- [x] Every official relation is represented in queryable relational form with suitable indexes.
- [x] Source metadata is present for cursor/update handling: source category, source id, latest skiptoken, deleted marker, source updated time, Atom updated time.
- [x] Tables and indexes support direct HTTP query use after sync without schema alteration.
- [x] `sqlx migrate run` succeeds against a fresh PostgreSQL database.
- [x] `sqlx migrate revert` or documented migration reset flow is tested where applicable.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): not applicable; task did not alter long/e2e test selection and long lane was not run per task instruction.
</acceptance_criteria>

.ralph/tasks/story-02-complete-sync-postgres/task-02-design-postgres-schema_plans/complete-postgres-schema-plan.md

NOW EXECUTE
