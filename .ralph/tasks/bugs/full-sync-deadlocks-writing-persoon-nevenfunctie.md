## Bug: Full sync deadlocks writing PersoonNevenfunctie <status>not_started</status> <passes>false</passes> <priority>high</priority>

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

<acceptance_criteria>
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
