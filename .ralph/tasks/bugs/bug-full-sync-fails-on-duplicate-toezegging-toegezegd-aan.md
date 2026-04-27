## Bug: Full Sync Fails on Duplicate Toezegging toegezegdAan <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 010 Task 01 clean full sync run `20260427-030235` failed while parsing the live SyncFeed `Toezegging` category.

Command:

```bash
CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260427-030235.toml run
```

Failure evidence from `.ralph/reports/full-sync-run-20260427-030235.log`:

```text
SYNC_START=2026-04-27T03:03:50+02:00
Error: Parse { category: "Toezegging", source: DuplicateSingleField { category: "Toezegging", field: "toegezegdAan" } }
SYNC_END=2026-04-27T03:06:48+02:00
DURATION_SECONDS=178
COMMAND_STATUS=1
```

The database was a fresh empty migrated database before the run:

```text
DB_NAME=opentk_full_sync_20260427_030235
DB_URL_REDACTED=postgres://postgres@127.0.0.1:55432/opentk_full_sync_20260427_030235
pre-sync pg_database_size=15544803 bytes / 15 MB
pre-sync table counts: all 107 public base tables had 0 rows
```

Failure state persisted by the sync runner:

```text
id=1
phase=parse
source_category=Toezegging
latest_skiptoken=24758003
message=Toezegging.toegezegdAan appeared more than once but is single-occurrence
created_at=2026-04-27 03:06:48.508926+02
```

Observed commit:

```text
0a680be798483ba7d2a7a5f1d77c1af2bef4dcb0
```

The parser/model should treat live `Toezegging.toegezegdAan` payloads according to the official schema and observed feed data. This may need the field to be modeled as multi-occurrence or moved to a relation table, similar to the previous `kamerbriefNakoming` duplicate-field fix.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] Not applicable: this bug does not require `make test-long`; the default suite covered the parser/schema/migration change
</acceptance_criteria>

Plan: `.ralph/tasks/bugs/bug-full-sync-fails-on-duplicate-toezegging-toegezegd-aan_plans/plan.md`

NOW EXECUTE
