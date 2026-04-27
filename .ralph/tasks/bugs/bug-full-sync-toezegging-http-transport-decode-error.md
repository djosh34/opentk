## Bug: Full sync fails on Toezegging HTTP transport decode error <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted a fresh one-time clean full sync from an empty
database and the public sync runner exited nonzero on the initial `Toezegging`
SyncFeed request with an HTTP transport body decode failure.

Command:

```bash
env CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260427-042209.toml run
```

Run metadata:

- run id: `20260427-042209`
- database: `opentk_full_sync_20260427_042209`
- database URL: `postgres://postgres@127.0.0.1:55432/opentk_full_sync_20260427_042209`
- sync start: `2026-04-27T04:23:07+02:00`
- sync end: `2026-04-27T04:29:41+02:00`
- duration: `394` seconds
- command status: `1`

Runner error:

```text
Error: Fetch { phase: Fetch, category: "Toezegging", source: HttpTransport { request_url: "https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Toezegging&content=internal", message: "error decoding response body" } }
```

Persisted ingest error:

```text
phase=fetch
source_category=Toezegging
latest_skiptoken=null
message=SyncFeed HTTP transport failed for https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Toezegging&content=internal: error decoding response body
```

Observed partial state after the failure:

- `sync_category`: `caught_up=14`, `running=18`
- `sync_entity` rows: `157551`
- database size at failure: `177779171` bytes / `170 MB`

Evidence files:

- run log: `.ralph/reports/full-sync-run-20260427-042209.log`
- status log: `.ralph/reports/full-sync-failure-status-20260427-042209.log`
- ingest error/state log: `.ralph/reports/full-sync-failure-ingest-errors-20260427-042209.log`
- pre-sync evidence: `.ralph/reports/full-sync-pre-sync-evidence-20260427-042209.log`
- config: `.ralph/reports/full-sync-config-20260427-042209.toml`

The full sync did not complete cleanly, so Story 10 Task 01 stopped before
post-sync measurements, report generation, or email.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with
Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<plan>
.ralph/tasks/bugs/bug-full-sync-toezegging-http-transport-decode-error_plans/transient-syncfeed-body-decode-plan.md
</plan>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — not applicable; this bug does not impact ultra-long test selection
</acceptance_criteria>
