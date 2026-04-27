## Bug: full sync operational plan assumes /usr/bin/time exists <status>done</status> <passes>true</passes> <priority>medium</priority>

<description>
While executing Story 10 Task 01 on 2026-04-26, the sync command wrapper failed
before starting the sync binary because `/usr/bin/time` was not available:

```text
SYNC_START=2026-04-26T21:41:42+02:00
/bin/bash: line 5: /usr/bin/time: No such file or directory
SYNC_END=2026-04-26T21:41:42+02:00
DURATION_SECONDS=0
COMMAND_STATUS=127
```

The operational plan requested `time cargo run ...`, but this environment does
not provide `/usr/bin/time`. The failed wrapper did not run SyncFeed ingestion,
but it is still an execution error and should be captured. Future operational
plans should use the shell `time` builtin or explicit timestamp arithmetic when
the environment has not proven that `/usr/bin/time` exists.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
  - Red operational evidence: `.ralph/reports/full-sync-20260426-213953-run.log`
    shows `/usr/bin/time` missing with `COMMAND_STATUS=127`, and
    `/bin/bash -lc '/usr/bin/time true'` still exits 127 in this environment.
- [x] I made the test green by fixing
  - The Story 10 operational plan now uses `date -Is` / `date +%s` timestamp
    arithmetic, records `COMMAND_STATUS`, and exits with that same status.
- [x] I manually verified the bug, and created a new Red test if not working still
  - Verified the wrapper around `true` exits 0 with all timing evidence, the
    wrapper around `false` exits 1 with `COMMAND_STATUS=1`, and the edited plan
    no longer contains `/usr/bin/time` or `time cargo run`.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
  - Not applicable: this is an operational plan wording fix and is not a
    story-ending validation task, so `make test-long` was intentionally not run.
</acceptance_criteria>

Plan: `.ralph/tasks/bugs/bug-full-sync-plan-assumes-usr-bin-time-exists_plans/portable-sync-duration-plan.md`

NOW EXECUTE
