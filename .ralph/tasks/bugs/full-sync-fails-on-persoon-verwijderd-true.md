## Bug: full sync fails on Persoon verwijderd boolean value True <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Manual verification for `.ralph/tasks/bugs/bug-full-sync-toezegging-initial-feed-timeout.md`
reran the captured full-sync command on 2026-04-27:

```bash
CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-213953.toml run
```

The run progressed past the original initial `Toezegging` SyncFeed timeout, but
then failed on a separate live payload parse error:

```text
Error: Parse { category: "Persoon", source: InvalidValue { category: "Persoon", field: "verwijderd", datatype: "xs:boolean", value: "True", message: "expected true, false, 1, or 0" } }
```

The verification output is recorded in
`.ralph/reports/full-sync-toezegging-timeout-verification-20260427.log`.

The target database recorded the new durable error:

```text
id=2
phase=parse
source_category=Persoon
latest_skiptoken=24579970
message=Persoon.verwijderd has invalid xs:boolean value "True": expected true, false, 1, or 0
```

Observed database state after the failure:

```text
sync_category states: caught_up=9, running=24
sync_entity rows: 96697
ingest_error rows include the historical Toezegging timeout and this Persoon parse failure
```

Current boolean parsing accepts only `true`, `false`, `1`, and `0`, but the live
SyncFeed payload can emit capitalized `True` for the `verwijderd` xs:boolean
attribute.
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
- [x] If this bug impacts ultra-long tests (or their selection): not applicable; this parser bug does not require `make test-long`
</acceptance_criteria>

NOW EXECUTE

Plan: `.ralph/tasks/bugs/full-sync-fails-on-persoon-verwijderd-true_plans/capitalized-boolean-parser-plan.md`
