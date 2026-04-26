## Bug: Full sync fails on unknown Toezegging toegezegdAan field <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 1 attempted one clean full SyncFeed-to-PostgreSQL sync from an
empty dedicated database after the previous PersoonNevenfunctie deleted-body
bug was marked passing:

- Config: `.ralph/reports/full-sync-config-20260426-205545.toml`
- Database target: `postgres://postgres@127.0.0.1:55432/opentk_full_sync_20260426_205545`
- Sync command: `time -p env CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-205545.toml run`
- Sync start: `2026-04-26T20:56:30+02:00`
- Runtime before failure: `real 72.77`
- Git SHA: `0fd233a6360118708361aadcb5f87319e6582a1f`

The full sync did not complete. The public sync runner exited with:

`Error: Parse { category: "Toezegging", source: UnknownField { category: "Toezegging", field: "toegezegdAan" } }`

Durable `ingest_error` evidence recorded:

- phase: `parse`
- source_category: `Toezegging`
- latest_skiptoken: `24757218`
- message: `Toezegging.toegezegdAan is unknown`
- created_at: `2026-04-26 20:57:43.64229+02`

Evidence files:

- `.ralph/reports/full-sync-run-20260426-205545-actual.log`
- `.ralph/reports/full-sync-failure-status-20260426-205545.log`
- `.ralph/reports/full-sync-failure-db-state-20260426-205545.log`
- `.ralph/reports/full-sync-empty-db-evidence-20260426-205545.log`
- `.ralph/reports/full-sync-config-validation-20260426-205545.log`

Observed status after failure showed `Toezegging` in `error` state and
nineteen other categories still `running`, so the required full sync report and
email could not be completed.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<plan>
.ralph/tasks/bugs/full-sync-fails-on-unknown-toezegging-toegezegd-aan_plans/toezegging-toegezegd-aan-plan.md
</plan>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): not applicable; this normal bug task did not require `make test-long`
</acceptance_criteria>

NOW EXECUTE
