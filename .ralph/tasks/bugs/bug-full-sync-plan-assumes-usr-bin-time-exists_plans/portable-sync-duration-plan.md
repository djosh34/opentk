# Portable Sync Duration Plan

Task: `.ralph/tasks/bugs/bug-full-sync-plan-assumes-usr-bin-time-exists.md`

## Goal

Fix the Story 10 full-sync operational plan so it no longer depends on
`/usr/bin/time` being installed. The operational command must measure start
time, end time, duration, and command status using shell facilities available in
the execution environment, while preserving the rule that sync failures are
visible and become add-bug tasks.

## Existing Interface

- Public sync interface remains:
  `cargo run -p opentk-db --bin opentk-sync -- --config <config> run`
- The affected boundary is the one-time Ralph operational plan:
  `.ralph/tasks/story-10-full-sync-backup/task-01-run-one-time-clean-full-sync-report_plans/one-time-clean-full-sync-report-plan.md`
- No product code, CLI command, helper script, or test that scans `.ralph` files
  should be added. `AGENTS.md` says not to test `.ralph/` files, and this is a
  planning defect, not application behavior.

## TDD / Verification Approach

Use red-green at the operational boundary without adding a checked-in test for
`.ralph` content:

1. Red evidence: cite and preserve the existing failed run evidence from
   `.ralph/reports/full-sync-20260426-213953-run.log`, where the wrapper exited
   127 before starting cargo because `/usr/bin/time` was absent.
2. Red reproduction: before editing, run a tiny shell reproduction that proves
   the current absolute-path assumption is invalid in this environment:
   `/bin/bash -lc '/usr/bin/time true'`
   It should fail with status 127 when `/usr/bin/time` is missing. If it does not
   fail because the environment changed, keep the historical log as the bug
   evidence and continue because the plan still must not assume that path.
3. Green: replace the Story 10 plan's sync timing step with explicit POSIX-ish
   shell timestamp arithmetic using `date +%s`, `date -Is`, `COMMAND_STATUS`,
   and an explicit `exit "$COMMAND_STATUS"` after recording all evidence.
4. Green verification: run the new wrapper around a harmless command such as
   `true` and a failing command such as `false` to prove it records start/end,
   computes non-negative `DURATION_SECONDS`, preserves the command status, and
   does not call `/usr/bin/time`.
5. Manual bug verification: grep the edited plan and relevant active task text
   for `/usr/bin/time` and `time cargo run`; both must be absent from current
   operational instructions. Historical reports and old bug descriptions may
   still mention the failure as evidence.

## Planned Operational Snippet

Replace the single `time cargo run ...` instruction with this shape:

```bash
SYNC_START=$(date -Is)
SYNC_START_EPOCH=$(date +%s)
echo "SYNC_START=${SYNC_START}"

env CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config <config> run
COMMAND_STATUS=$?

SYNC_END=$(date -Is)
SYNC_END_EPOCH=$(date +%s)
DURATION_SECONDS=$((SYNC_END_EPOCH - SYNC_START_EPOCH))

echo "SYNC_END=${SYNC_END}"
echo "DURATION_SECONDS=${DURATION_SECONDS}"
echo "COMMAND_STATUS=${COMMAND_STATUS}"
exit "${COMMAND_STATUS}"
```

Keep this in the plan as an operator command block, not a new script. The command
must not mask the sync exit status, and any non-zero status still follows the
existing Story 10 failure path: file add-bug with command, stdout/stderr,
database target, timestamps, and observed state, then stop.

## Execution Checklist

- [x] Read this plan, the bug task, `tdd`, and `improve-code-boundaries`.
- [x] Red: inspect `.ralph/reports/full-sync-20260426-213953-run.log` and record
  the `/usr/bin/time` status-127 failure in progress.
- [x] Red: run `/bin/bash -lc '/usr/bin/time true'` and record whether the
  current environment still reproduces the missing binary.
- [x] Green: edit
  `.ralph/tasks/story-10-full-sync-backup/task-01-run-one-time-clean-full-sync-report_plans/one-time-clean-full-sync-report-plan.md`
  so the sync run step uses explicit timestamp arithmetic and preserves
  `COMMAND_STATUS`.
- [x] Green: update
  `.ralph/tasks/bugs/bug-full-sync-plan-assumes-usr-bin-time-exists.md`
  acceptance criteria checkboxes as evidence is collected.
- [ ] Verify the wrapper shape with harmless commands:
  - [x] Success case around `true` exits 0 and prints `SYNC_START`, `SYNC_END`,
    non-negative `DURATION_SECONDS`, and `COMMAND_STATUS=0`.
  - [x] Failure case around `false` exits 1 while still printing all evidence and
    `COMMAND_STATUS=1`.
- [x] Grep current operational instructions for forbidden assumptions:
  - [x] `rg -n "/usr/bin/time|time cargo run" .ralph/tasks/story-10-full-sync-backup/task-01-run-one-time-clean-full-sync-report_plans/one-time-clean-full-sync-report-plan.md`
    returns no matches.
- [ ] Final improve-code-boundaries review:
  - [x] No product code, helper script, or one-off abstraction was added.
  - [x] No error is swallowed; the wrapper records status and exits with that
    same status.
  - [x] The operational plan owns the operational command; the sync binary still
    owns sync behavior.
- [ ] Run required gates:
  - [x] `make check`
  - [x] `make test`
  - [x] `make lint`
  - [x] Do not run `make test-long`; this is not a story-ending validation task.
- [ ] Finish only after all checks pass:
  - [x] Set `<passes>true</passes>` in the bug task.
  - [x] Run `/bin/bash .ralph/task_switch.sh`.
  - [ ] `git add --all`
  - [ ] Commit as
    `task finished bug-full-sync-plan-assumes-usr-bin-time-exists: use portable sync duration wrapper`
    with evidence for red reproduction, wrapper verification, and required
    checks.
  - [ ] `git push`

## Boundary Notes

- Do not create a reusable duration helper unless another real caller appears.
  This is one operational plan command block, so a helper would make the boundary
  worse.
- Do not add tests that assert over `.ralph/` file contents; repo instructions
  explicitly forbid testing `.ralph` files.
- If execution discovers that this actually needs product behavior or a new
  public interface, switch this plan and the task back to `TO BE VERIFIED` and
  stop immediately.

NOW EXECUTE
