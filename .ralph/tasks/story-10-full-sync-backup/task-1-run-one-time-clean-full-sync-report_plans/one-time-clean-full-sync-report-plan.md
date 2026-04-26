# One-Time Clean Full Sync Report Plan

## Task

Run one clean full SyncFeed-to-PostgreSQL sync from an empty database, measure the result, write `.ralph/reports/full-sync-report-YYYYMMDD-HHMMSS.md`, and email that report.

This is an operational task. Do not add application code, a CLI command, a script, or backup/restore product behavior.

## Existing Interfaces

- Sync runner: `cargo run -p opentk-db --bin opentk-sync -- --config <config> run`
- Sync status: `cargo run -p opentk-db --bin opentk-sync -- --config <config> status`
- Sync verification: `cargo run -p opentk-db --bin opentk-sync -- --config <config> verify --required-relation-samples 1`
- Configuration boundary: TOML file loaded by `opentk-config`.
- Database setup boundary: PostgreSQL tools plus the existing checked-in SQL migration.
- Email boundary from `email-me`: `/home/joshazimullah.linux/work_mounts/patroni_rewrite/receive_mail/reply.sh user@toffemail.nl <subject> <body-or-stdin>`.

## TDD / Verification Mindset

Because this task is operational and explicitly forbids new product code, the red-green loop is applied as executable validation gates rather than new tests:

1. Red/precondition: prove the selected database is empty after reset, migrated, and has no SyncFeed rows before running sync.
2. Green/tracer: run the existing `opentk-sync run` public interface once and require clean completion for all configured official categories.
3. Green/measurement: use SQL-visible database state and `opentk-sync status` / `verify` output to collect the required evidence.
4. Refactor/boundary check: ensure the report is a report only, no new app code or helper script was introduced, and no errors were hidden or normalized away.
5. Final gates: run `make check`, `make lint`, and `make test`. Do not run `make test-long`.

If any sync or measurement command fails, or any result is questionable, stop execution and immediately file an add-bug task with command, stderr/stdout, database target, timestamps, and observed state.

## Execution Checklist

- [ ] Record initial metadata:
  - [ ] `date -Is`
  - [ ] `git rev-parse HEAD`
  - [ ] `git status --short`
  - [ ] exact chosen config path and redacted database target
  - [ ] `cargo run -p opentk-db --bin opentk-sync -- --config <config> --validate-config`
- [ ] Create or reset a dedicated empty PostgreSQL database for the run, for example `opentk_full_sync_YYYYMMDD_HHMMSS`.
- [ ] Write a temporary local TOML config under `.ralph/reports/` for the run if no suitable config exists. Keep it report-adjacent, not product code. It must point to the dedicated database and either omit `sync.categories` to use all official categories or explicitly list the official categories discovered from the existing config/model.
- [ ] Apply the existing migration to the empty database using the existing migration artifact. Prefer `sqlx migrate run --source migrations --database-url <url>` if `sqlx` CLI is available; otherwise use `psql <url> -v ON_ERROR_STOP=1 -f migrations/20260426000000_sync_schema.up.sql` and record that schema version as `20260426000000_sync_schema`.
- [ ] Prove the database is empty of SyncFeed data before sync:
  - [ ] database identity: `SELECT current_database(), current_user, inet_server_addr(), inet_server_port(), version();`
  - [ ] schema/migration version: `_sqlx_migrations` if present, otherwise the applied migration filename and checksum evidence from `sha256sum migrations/20260426000000_sync_schema.up.sql`
  - [ ] storage size: `SELECT pg_database_size(current_database()), pg_size_pretty(pg_database_size(current_database()));`
  - [ ] pre-sync row counts for `sync_category`, `sync_entity`, `ingest_error`, generated category tables, relation tables, `document_asset`, and `document_content`
- [ ] Start the full sync:
  - [ ] record `SYNC_START=$(date -Is)`
  - [ ] run `time cargo run -p opentk-db --bin opentk-sync -- --config <config> run`
  - [ ] capture stdout/stderr to report evidence under `.ralph/reports/`
  - [ ] record `SYNC_END=$(date -Is)` and wall-clock duration
- [ ] Confirm clean completion:
  - [ ] run `cargo run -p opentk-db --bin opentk-sync -- --config <config> status`
  - [ ] every configured category must report caught-up state with no last error
  - [ ] run `cargo run -p opentk-db --bin opentk-sync -- --config <config> verify --required-relation-samples 1`
  - [ ] query `ingest_error`; if any rows exist, treat as a failure/questionable result and file an add-bug task
- [ ] Measure post-sync state:
  - [ ] post-sync database size and pretty size
  - [ ] storage growth in bytes and pretty units
  - [ ] row count by synced category/table
  - [ ] row count by relation/support table
  - [ ] final cursor/skiptoken state by category from `sync_category`
  - [ ] `document_asset` and `document_content` row/byte evidence if populated
  - [ ] final schema/migration version
  - [ ] final commit SHA and dirty/clean worktree state
- [ ] Write `.ralph/reports/full-sync-report-YYYYMMDD-HHMMSS.md` with:
  - [ ] command log
  - [ ] exact timestamps and duration
  - [ ] database identity and redacted connection target
  - [ ] pre/post sizes and storage growth
  - [ ] row counts by category/table
  - [ ] final skiptokens/cursors by category
  - [ ] schema/migration version
  - [ ] git commit and worktree state
  - [ ] caveats, including whether the worktree was dirty before this task
- [ ] Email the completed report with the `email-me` skill:
  - [ ] `/home/joshazimullah.linux/work_mounts/patroni_rewrite/receive_mail/reply.sh "user@toffemail.nl" "OpenTK full sync report" < .ralph/reports/full-sync-report-YYYYMMDD-HHMMSS.md`
- [ ] Run final checks:
  - [ ] `make check`
  - [ ] `make lint`
  - [ ] `make test`
  - [ ] Do not run `make test-long`.
- [ ] Final improve-code-boundaries review:
  - [ ] Confirm no application code, CLI command, script, backup/restore behavior, or unnecessary abstraction was added.
  - [ ] Confirm no errors were swallowed. Any failure must be represented as an add-bug task, not hidden in the report.
- [ ] Mark the task complete only after all acceptance criteria and checks pass:
  - [ ] set `<passes>true</passes>` in the task file
  - [ ] run `/bin/bash .ralph/task_switch.sh`
  - [ ] `git add --all`
  - [ ] commit as `task finished task-1-run-one-time-clean-full-sync-report: ran clean full sync and reported measurements`
  - [ ] include evidence for sync completion, report email, `make check`, `make lint`, and `make test` in the commit message
  - [ ] `git push`

## Boundary Notes

- The public operational interface is already deep enough: the sync binary owns config loading, runner construction, status printing, and verification printing.
- This task should not introduce a reporting module because the report is one-time operational evidence.
- This task should not introduce a database lifecycle helper because the task explicitly says to use existing sync binary and database tools manually.
- If execution reveals that required measurements cannot be obtained without product code or a new public interface, switch this plan and the task file back to `TO BE VERIFIED` and stop.

NOW EXECUTE
