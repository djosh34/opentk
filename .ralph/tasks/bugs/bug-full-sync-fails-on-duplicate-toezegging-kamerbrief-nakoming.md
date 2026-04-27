## Bug: full sync fails on duplicate Toezegging kamerbriefNakoming field <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted a clean full SyncFeed-to-PostgreSQL sync from a fresh
database and the public sync runner exited nonzero after 305 seconds.

Run evidence:

- run id: `20260427-022641`
- database: `opentk_full_sync_20260427_022641`
- config: `.ralph/reports/full-sync-config-20260427-022641.toml`
- command: `CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260427-022641.toml run`
- start: `2026-04-27T02:27:33+02:00`
- end: `2026-04-27T02:32:38+02:00`
- duration: `305` seconds
- log: `.ralph/reports/full-sync-run-20260427-022641.log`

The runner failed with:

`Error: Parse { category: "Toezegging", source: DuplicateSingleField { category: "Toezegging", field: "kamerbriefNakoming" } }`

Post-failure `opentk-sync status` recorded `Toezegging` in `error` state:

- source category: `Toezegging`
- latest skiptoken: `24757475`
- error skiptoken: `24757735`
- message: `Toezegging.kamerbriefNakoming appeared more than once but is single-occurrence`
- status log: `.ralph/reports/full-sync-failure-status-20260427-022641.log`
- ingest error log: `.ralph/reports/full-sync-failure-ingest-errors-20260427-022641.log`

The full sync did not complete cleanly, so Story 10 Task 01 stopped before
post-sync measurement, final report generation, or email.
</description>

<plan>
.ralph/tasks/bugs/bug-full-sync-fails-on-duplicate-toezegging-kamerbrief-nakoming_plans/plan.md
</plan>

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
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only; not required for this default-lane parser/schema bug)
</acceptance_criteria>
