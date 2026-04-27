## Bug: full sync fails on FractieZetelVacature initial SyncFeed timeout <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 010 Task 01 attempted one clean full SyncFeed-to-PostgreSQL sync from a
fresh migrated database on 2026-04-27. The actual sync command failed after
540 seconds with exit status 1:

```bash
CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260427-033618.toml run
```

Database target:

```text
postgres://postgres@127.0.0.1:55432/opentk_full_sync_20260427_033618
```

Run timestamps:

```text
SYNC_START=2026-04-27T03:37:20+02:00
SYNC_END=2026-04-27T03:46:20+02:00
DURATION_SECONDS=540
COMMAND_STATUS=1
```

Command failure:

```text
Error: Fetch { phase: Fetch, category: "FractieZetelVacature", source: Timeout { request_url: "https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=FractieZetelVacature&content=internal", message: "error sending request for url (https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=FractieZetelVacature&content=internal)" } }
```

The database recorded the failure in `ingest_error`:

```text
id=1
phase=fetch
source_category=FractieZetelVacature
latest_skiptoken=null
message=SyncFeed request timed out for https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=FractieZetelVacature&content=internal: error sending request for url (https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=FractieZetelVacature&content=internal)
created_at=2026-04-27 03:46:20.910344+02
```

Observed database state immediately after failure:

```text
FractieZetelVacature state=error latest_skiptoken=null last_error=fetch timeout
Several large categories remained state=running.
Toezegging remained state=not_started.
```

Evidence:

- Run log: `.ralph/reports/full-sync-run-20260427-033618.log`
- Status log: `.ralph/reports/full-sync-failure-status-20260427-033618.log`
- Ingest error log: `.ralph/reports/full-sync-failure-ingest-errors-20260427-033618.log`
- Config: `.ralph/reports/full-sync-config-20260427-033618.toml`
- Pre-sync evidence: `.ralph/reports/full-sync-pre-sync-evidence-20260427-033618.log`

The first ingest-error evidence query used the wrong column name
`skiptoken` and failed visibly; the same evidence log contains the corrected
query using `latest_skiptoken`.

The full sync did not complete cleanly, so Story 010 Task 01 was stopped before
post-sync measurement, final report generation, or email.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<plan>
.ralph/tasks/bugs/bug-full-sync-fractie-zetel-vacature-initial-feed-timeout_plans/fractie-zetel-vacature-initial-feed-timeout-plan.md
NOW EXECUTE
</plan>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — not applicable; this normal bug task did not require the ultra-long lane
</acceptance_criteria>
