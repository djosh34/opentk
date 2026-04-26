## Bug: Full sync deadlocks writing PersoonNevenfunctie <status>not_started</status> <passes>true</passes> <priority>high</priority>

<description>
While manually verifying
`.ralph/tasks/bugs/full-sync-fails-on-deleted-persoon-nevenfunctie-body.md`,
the saved full-sync command progressed past the original parse failure and then
failed in the store/write phase:

`Error: Store { phase: Write, category: "PersoonNevenfunctie", source: SyncStoreError { message: "database write failed: error returned from database: deadlock detected" } }`

Command:

`timeout 180s env CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-193554.toml run`

Config:

`.ralph/reports/full-sync-config-20260426-193554.toml`

Database target:

`postgres://postgres@127.0.0.1:55432/opentk_full_sync_20260426_193554`

Status after failure showed the original `DeletedEntityHasBody` parse error was
gone and replaced by this durable write error:

- phase: `write`
- source_category: `PersoonNevenfunctie`
- latest_skiptoken: `16662485`
- entity_id: `null`
- message: `database write failed: error returned from database: deadlock detected`

This is a separate blocker from the deleted-body parser bug.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<plan>
.ralph/tasks/bugs/full-sync-deadlocks-writing-persoon-nevenfunctie_plans/concurrent-writer-lock-order-plan.md
</plan>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): not applicable; `make test-long` was not run for this normal bug task
</acceptance_criteria>

<verification_notes>
- Focused writer test failed red with `database write failed: error returned from database: deadlock detected` before the fix.
- The same focused writer test passed after deterministic writer-owned `sync_entity` advisory lock ordering was added.
- Captured full-sync command was rerun with the bug report config under its existing `timeout 180s` wrapper. It emitted no fresh error before the timeout, but did not complete the whole full sync inside 180 seconds; `status` still displayed the previously recorded `PersoonNevenfunctie` deadlock as persisted state.
- `make test-long` was not run because this is a normal bug task and not a story-end validation gate.
</verification_notes>
