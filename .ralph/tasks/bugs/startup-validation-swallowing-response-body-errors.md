## Bug: Startup validation swallows response body errors <status>done</status> <passes>true</passes> <priority>medium</priority>

<description>
During Story 10 Task 1 operational sync preparation, inspection found
`crates/opentk-db/src/startup_validation.rs` using
`response.text().await.unwrap_or_default()` when validating SyncFeed HTTP
status errors. If reading the response body fails, the real error is silently
replaced with an empty message. Repository instructions forbid swallowing or
ignoring errors.
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

Plan: `.ralph/tasks/bugs/startup-validation-swallowing-response-body-errors_plans/syncfeed-body-read-error-plan.md`

Completion notes:
- Targeted RED: `CARGO_INCREMENTAL=0 ./scripts/cargo-test-with-postgres.sh -p opentk-db --test startup_validation sync_validation_reports_syncfeed_error_body_read_failures -- --nocapture` failed with an empty SyncFeed status message.
- Targeted GREEN: same targeted command passed.
- Manual verification: `CARGO_INCREMENTAL=0 ./scripts/cargo-test-with-postgres.sh -p opentk-db --test startup_validation -- --nocapture` passed 8 tests.
- Required gates: `make check`, `make test`, and `make lint` passed.
- `make test-long` was not run because this normal bug task does not impact the long/e2e lane.
