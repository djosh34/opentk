## Bug: Database URL redaction ignores set_password error <status>not_started</status> <passes>true</passes> <priority>medium</priority>

<description>
During the boundary review for the SyncFeed startup validation body-read fix,
inspection found `crates/opentk-db/src/startup_validation.rs` ignoring the
result of `url.set_password(Some("***"))` in `redacted_database_url` with
`let _ = ...`. Repository instructions forbid swallowing or ignoring errors.
The redaction helper must make password redaction failures explicit instead of
silently returning a potentially unredacted URL.
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
- [x] `make check` - passes cleanly
- [x] `make test` - passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` - passes cleanly
- [x] Not applicable: this bug does not impact ultra-long tests or their selection, so `make test-long` was not run
</acceptance_criteria>

<plan>
.ralph/tasks/bugs/database-url-redaction-ignores-set-password-error_plans/redaction-helper-error-boundary.md
</plan>
