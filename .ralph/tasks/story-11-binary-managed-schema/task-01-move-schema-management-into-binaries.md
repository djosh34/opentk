## Task: Move Schema Management Into Binaries <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Remove the repository-level SQL migration-file workflow and make schema handling an automatic binary responsibility. The project is greenfield with no users and no backwards compatibility requirement, so the implementation should remove the existing `migrations/` SQL files, migration-specific tests, Docker migration mounting, and manual init-script migration behavior instead of preserving legacy paths.

The sync binary must own automatic schema creation/migration at startup using SQLx-native integration from Rust code rather than checked-in `.sql` files. When `opentk-sync` starts normally, it must validate/create/update the PostgreSQL schema as needed and then resume syncing to the SyncFeed from durable sync state. Restarting `opentk-sync` must not wipe tables, truncate data, reset cursors, rerun a destructive initialization path, or require a human to run migrations manually.

The API binary must not mutate the database schema. When `opentk-api` starts, it must validate that the connected database schema exactly matches the schema expected by the binary. If the schema is missing, stale, newer, or otherwise incompatible, API startup must fail loudly with an actionable error instead of attempting to repair or ignore the mismatch.

All schema management must be driven from the Rust schema definition/code already present in the repository, especially `opentk-db::postgres_schema` and related `SchemaSpec` rendering/validation paths. Do not leave checked-in SQL migration files as source of truth. Do not keep migration tests whose purpose is to run/revert SQLx `.sql` migration files. Replace them with tests for binary-owned schema bootstrapping and API schema validation.

In scope:
- Remove checked-in SQL migration files and any root-level `migrations/` dependency.
- Remove Docker Compose migration mounting and `scripts/init-db.sh` migration execution paths if they are no longer needed.
- Remove tests that specifically exercise SQLx file migrations and their revert/down behavior.
- Add/adjust tests proving `opentk-sync` can initialize or migrate schema on startup without data loss.
- Add/adjust tests proving `opentk-sync` resumes existing sync state after restart rather than resetting or truncating anything.
- Add/adjust tests proving `opentk-api` fails startup against missing or mismatched schema and starts only against the expected schema.
- Keep SQLx as the database access layer and use SQLx-native Rust integration where appropriate, but the schema source of truth must not be checked-in `.sql` migration files.
- Update docs/README only where they currently describe SQL migration files, manual migration commands, or Docker init migration behavior.

Out of scope:
- Backwards compatibility with existing volumes or previously applied migration histories.
- Manual migration procedures.
- Keeping the old SQL files for documentation, rollback, compatibility, or tests.

</description>


<acceptance_criteria>
- [ ] Red/green TDD a sync startup test that starts with an empty PostgreSQL database/schema and proves `opentk-sync` creates the expected schema automatically before syncing.
- [ ] Red/green TDD a sync restart/resume test that seeds durable sync state and entity data, starts `opentk-sync`, and proves startup does not truncate, drop, reset, or rewrite unrelated existing data/cursors.
- [ ] Red/green TDD an API startup validation test that fails when the database schema is missing.
- [ ] Red/green TDD an API startup validation test that fails when the database schema is incompatible with the binary's expected schema.
- [ ] Red/green TDD an API startup validation test that succeeds when the database schema matches the binary's expected schema.
- [ ] Remove `migrations/*.sql` and all production/test references to `sqlx::migrate!(...)` or `sqlx migrate run --source /workspace/migrations`.
- [ ] Remove SQLx migration run/revert tests that exist only to test checked-in migration files, including down/revert coverage.
- [ ] Remove Docker Compose migration volume mounting and init-script migration execution for the old migration directory.
- [ ] Update README/docs so they no longer claim schema changes are handled by checked-in SQL migration files or manual migration commands.
- [ ] `rg -n "migrations|sqlx::migrate!|sqlx migrate|/workspace/migrations|\\.up\\.sql|\\.down\\.sql" .` shows no stale migration-file workflow references except clearly intentional historical notes inside this task file.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
