## Task: Story 2 Task 4 - Implement Complete XML Parser and PostgreSQL Writers <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the SyncFeed XML parser and PostgreSQL writers for the complete official model. The parser must convert every modeled entity type, field, attribute, delete marker, relationship, multiplicity, enclosure, datatype, and update timestamp into typed Rust structs and database writes. The writer must upsert/replace updated entities, record delete markers according to the selected sync semantics, and maintain relation tables.

The implementation must use `quick-xml` for strict XML parsing and `sqlx` where it fits. Use PostgreSQL-native bulk write techniques where useful, such as `COPY`, staging tables, `INSERT ... ON CONFLICT`, set-based merges, and transactional swaps, while keeping correctness tests first. If a new official type or field is discovered during implementation or test sync, add it to the model, parser, migration, and tests in the same task.

In scope: complete parser, generated or hand-written entity mappings, PostgreSQL writers, transaction boundaries, relation handling, delete/update replacement semantics, asset metadata rows, and parser fixtures. Out of scope: HTTP API and document text extraction.

</description>


<acceptance_criteria>
- [x] Red/green TDD: parser fixture tests fail for at least one scalar field, relation, repeated relation, delete marker, and enclosure before implementation, then pass.
- [x] Every official modeled entity type can parse from representative XML into typed Rust data.
- [x] Every official modeled field and relation is written into PostgreSQL.
- [x] Updated entities replace prior current rows deterministically.
- [x] Delete markers are recorded consistently according to the sync semantics.
- [x] Relation tables remain queryable by both source and target entity.
- [x] PostgreSQL writes are transactional per page.
- [x] Tests cover parser/writer round trips for all entity categories.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

.ralph/tasks/story-2-complete-sync-postgres/task-4-implement-complete-parser-and-writers_plans/complete-parser-and-writers-plan.md

NOW EXECUTE
