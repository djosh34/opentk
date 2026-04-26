## Bug: Remove Swallowed Env And Duration Errors <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
Final boundary review for Story 3 Task 1 detected pre-existing swallowed errors outside the new API path:

- `crates/opentk-db/src/bin/complete-sync.rs` uses `std::env::var("DATABASE_URL").ok()` while resolving database configuration, which hides invalid Unicode environment variable errors.
- `crates/opentk-db/src/sync_state.rs` uses `.to_std().ok()` while converting lag duration, which hides negative/invalid duration conversion errors.

Repo policy says errors must not be swallowed or ignored. These should be converted to explicit typed errors or explicit domain states through public behavior.
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
