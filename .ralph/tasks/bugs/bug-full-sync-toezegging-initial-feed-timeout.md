## Bug: full sync fails on Toezegging initial SyncFeed timeout <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
Story 10 Task 1 attempted one clean full SyncFeed-to-PostgreSQL sync from a
fresh migrated database on 2026-04-26. The actual sync command failed after
281 seconds with exit status 1:

```bash
cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-213953.toml run
```

Database target:

```text
postgres://postgres@127.0.0.1:55432/opentk_full_sync_20260426_213953
```

Run timestamps:

```text
SYNC_START=2026-04-26T21:42:16+02:00
SYNC_END=2026-04-26T21:46:57+02:00
DURATION_SECONDS=281
COMMAND_STATUS=1
```

Command failure:

```text
Error: Fetch { phase: Fetch, category: "Toezegging", source: Timeout { request_url: "https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Toezegging&content=internal", message: "error sending request for url (https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Toezegging&content=internal)" } }
```

The database recorded the failure in `ingest_error`:

```text
id=1
phase=fetch
source_category=Toezegging
latest_skiptoken=null
message=SyncFeed request timed out for https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Toezegging&content=internal: error sending request for url (https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Toezegging&content=internal)
created_at=2026-04-26 21:46:57.407769+02
```

Observed database state immediately after failure:

```text
sync_category states: caught_up=4, running=27
sync_entity rows: 53480
ingest_error rows: 1
```

The full sync did not complete cleanly, so Story 10 Task 1 was stopped before
status/verify/final measurement/report/email. The run log is
`.ralph/reports/full-sync-20260426-213953-actual-run.log`.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<plan>
.ralph/tasks/bugs/bug-full-sync-toezegging-initial-feed-timeout_plans/transient-syncfeed-timeout-plan.md
</plan>

<acceptance_criteria>
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
