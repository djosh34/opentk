## Task: Story 2 Task 6 - Deep Verify Complete PostgreSQL Sync <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Prove the entire official SyncFeed model can be synced into PostgreSQL real tables and directly queried for HTTP use. This task is complete only when the worker can perform a full sync or a documented representative full-model verification run, validate schema coverage against the official model, validate row counts and relations, and demonstrate direct SQL queries over the resulting tables without additional transformation.

The verification must use the official informatiemodel and XSD-derived metadata as the coverage oracle. It must include live API smoke tests against the real SyncFeed with controlled concurrency, plus deterministic fixture tests for CI. Any newly encountered entity type, field, relation, datatype, namespace, or delete/update edge case must be added to model, migrations, parser, writer, and tests before this task passes.

In scope: deep sync verification, coverage reports, schema assertions, live controlled smoke tests, relation integrity checks, idempotency checks, restart checks, storage measurements, and query-readiness checks. Out of scope: HTTP endpoint implementation.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add coverage assertions that fail when any official entity, field, or relation lacks parser/writer/schema coverage, then make them pass.
- [x] Verification proves every official entity type is represented in PostgreSQL.
- [x] Verification proves every official relation is queryable by direct SQL joins or relation-table lookups.
- [x] Verification proves full sync/catch-up reaches `resume` for all selected official categories under controlled concurrency.
- [x] Verification proves rerunning sync is idempotent for already-applied pages.
- [x] Verification proves updates replace current data deterministically.
- [x] Verification records storage size for database tables, indexes, HTML assets, and linked binary asset metadata.
- [x] Verification includes representative direct SQL queries that an HTTP API can use immediately.
- [x] Live API tests are clearly separated into `make test-long`.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

.ralph/tasks/story-2-complete-sync-postgres/task-6-deep-verify-complete-sync_plans/deep-verify-complete-sync-plan.md

NOW EXECUTE
