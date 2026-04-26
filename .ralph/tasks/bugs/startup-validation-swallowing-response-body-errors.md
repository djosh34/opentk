## Bug: Startup validation swallows response body errors <status>not_started</status> <passes>false</passes> <priority>medium</priority>

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
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
