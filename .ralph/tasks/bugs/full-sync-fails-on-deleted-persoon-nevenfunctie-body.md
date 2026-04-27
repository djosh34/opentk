## Bug: Full sync fails on deleted PersoonNevenfunctie body content <status>not_started</status> <passes>true</passes> <priority>high</priority>

<description>
Story 010 Task 01 attempted one clean full SyncFeed-to-PostgreSQL sync from an
empty dedicated database:

- Config: `.ralph/reports/full-sync-config-20260426-193554.toml`
- Database target: `postgres://postgres@127.0.0.1:55432/opentk_full_sync_20260426_193554`
- Sync command: `CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-193554.toml run`
- Sync start: `2026-04-26T19:38:46+02:00`
- Git SHA: `3f4348d20278566444d276c6c3d4dd0127214c80`

The full sync did not complete. The public sync runner exited with:

`Error: Parse { category: "PersoonNevenfunctie", source: DeletedEntityHasBody { category: "PersoonNevenfunctie", source_id: 551920a3-7757-467a-b689-9307fc59194c } }`

Durable `ingest_error` evidence recorded:

- phase: `parse`
- source_category: `PersoonNevenfunctie`
- latest_skiptoken: `16662485`
- message: `deleted PersoonNevenfunctie 551920a3-7757-467a-b689-9307fc59194c must not contain scalar or relation body content`
- created_at: `2026-04-26 19:39:42.864051+02`

Evidence files:

- `.ralph/reports/full-sync-run-20260426-193554.log`
- `.ralph/reports/full-sync-failure-status-20260426-193554.log`
- `.ralph/reports/full-sync-failure-db-state-20260426-193554.log`
- `.ralph/reports/full-sync-empty-db-evidence-20260426-193554.log`

Observed status after failure showed `PersoonNevenfunctie` in `error` state and
several other categories still `running`, so the required full sync report and
email could not be completed.
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
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>
.ralph/tasks/bugs/full-sync-fails-on-deleted-persoon-nevenfunctie-body_plans/deleted-body-tombstone-plan.md
</plan>

NOW EXECUTE
