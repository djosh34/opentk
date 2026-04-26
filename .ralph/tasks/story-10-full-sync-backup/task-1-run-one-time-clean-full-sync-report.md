## Task: Story 10 Task 1 - Run One-Time Clean Full Sync and Report Results <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Run one clean full SyncFeed-to-PostgreSQL sync once, measure the result, write a human-readable report under `.ralph/reports/`, and email that report to the user.

This is an operational task, not a product feature. Do not add a CLI command. Do not add a script. Do not add application code. Do not add backup/restore logic. Use the existing sync binary and existing database tools manually from the task runner.

Required setup:
1. Start from an empty database, not a partially synced database.
2. Record the exact database identity and connection target used.
3. Record the pre-sync database storage size while the database is empty, before any SyncFeed rows are loaded.

Required sync work:
1. Run the existing full sync path once until all configured/official categories are caught up.
2. Record sync start timestamp, completion timestamp, and wall-clock duration.
3. Record any failures explicitly. If the full sync does not complete cleanly, stop and file an add-bug task immediately with the failure details.

Required measurements after successful full sync:
1. PostgreSQL storage size before sync, from the empty database.
2. PostgreSQL storage size after full sync.
3. Storage growth in bytes and human-readable units.
4. Row count for every synced category/table.
5. Final skiptoken/cursor state for every synced category.
6. Schema/migration version at the time of measurement.
7. Git commit SHA and dirty/clean worktree state used for the run.

Required report:
1. Write `.ralph/reports/full-sync-report-YYYYMMDD-HHMMSS.md`.
2. The report must include commands run, timestamps, duration, pre/post database sizes, row counts by category, final skiptokens, schema version, commit SHA, and any caveats.
3. Use the `email-me` skill to email the completed report to the user.

In scope: running the sync once, measuring the fully synced database, writing the report, emailing the report, filing add-bug immediately for any sync/measurement issue. Out of scope: new code, new CLI commands, new scripts, product backup feature, restore feature.
</description>

<acceptance_criteria>
- [ ] Full sync was run once from an empty database to completion
- [ ] Pre-sync empty database storage size was recorded
- [ ] Post-sync fully synced database storage size was recorded
- [ ] Sync start time, completion time, and wall-clock duration were recorded
- [ ] Row counts for every synced category/table were recorded
- [ ] Final skiptoken/cursor state for every synced category was recorded
- [ ] Report file exists at `.ralph/reports/full-sync-report-YYYYMMDD-HHMMSS.md`
- [ ] Report includes exact commands, database target, schema version, commit SHA, dirty/clean worktree state, measurements, and caveats
- [ ] Report was emailed to the user using the `email-me` skill
- [ ] Any failure or questionable result was immediately filed as an add-bug task
- [ ] No application code, CLI command, script, or backup/restore product behavior was added
</acceptance_criteria>
