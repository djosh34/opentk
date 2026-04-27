# Schema-Backed Post-Measurement Plan

Task: `.ralph/tasks/bugs/bug-full-sync-post-measurement-query-used-invalid-columns.md`

## Goal

Remove the need for future full-sync post-run measurement commands to guess
PostgreSQL column names. The public verification path should expose the
operational evidence needed after a full sync using checked application code
that is already coupled to the current schema.

## Current Evidence

- Failed historical evidence is preserved in
  `.ralph/reports/full-sync-post-sync-evidence-20260427-044908.log`.
- The invalid columns were:
  - `ingest_error.occurred_at`; the current schema uses `ingest_error.created_at`.
  - `document_content.stored_html`; the current schema uses
    `document_content.extracted_html`.
- Corrected measurements are preserved in
  `.ralph/reports/full-sync-corrected-measurements-20260427-044908.log`.
- Existing `opentk-db::sync_verification` already computes document-content
  storage bytes from `document_content.extracted_html`, but the CLI summary only
  prints aggregate storage values and does not expose ingest-error row details.

## Public Interface

Keep the existing command shape:

```bash
cargo run -p opentk-db --bin opentk-sync -- --config <config> verify
```

Extend the output from `verify` so operators can capture post-sync evidence
without ad hoc SQL:

- Keep the current first summary line for category/relation/storage totals.
- Add a schema-backed ingest-error summary line with the current error count.
- Add one row per recent ingest error using the schema-backed fields already
  stored in `ingest_error`: `id`, `phase`, `source_category`, `source_id`,
  `latest_skiptoken`, `message`, and `created_at`.
- Keep document-content byte evidence on the verification report boundary. If
  renaming `stored_html_bytes` to `extracted_html_bytes` is needed to remove a
  misleading boundary, do it boldly across code, tests, docs, and CLI output.
  Do not keep a backwards-compatible alias; this is greenfield.

## TDD Slices

- [x] RED 1: Add one focused behavior test for the verification report public
  API in `crates/opentk-db/tests/sync_verification.rs`.
  - Seed a migrated test schema with one durable `ingest_error` row using the
    real `created_at` column.
  - Call `verify_sync_database`.
  - Expect the report to include `ingest_error_count = 1` and a recent error
    entry containing the seeded phase/category/source/skiptoken/message and
    `created_at`.
  - Confirm this fails because `SyncVerificationReport` has no ingest-error
    evidence yet.
- [x] GREEN 1: Add the minimal schema-backed ingest-error evidence to
  `opentk-db::sync_verification`.
  - Add report types inside `sync_verification`, not a new reporting module.
  - Query only current schema columns; do not use `occurred_at`.
  - Order recent errors by `created_at DESC, id DESC` and cap the detail list to
    a small fixed number such as 20 so full-sync output remains readable.
- [x] RED 2: Add one focused CLI/public-output behavior test in
  `crates/opentk-db/src/bin/opentk-sync.rs` or another existing binary-facing
  test location if the current private functions make a cleaner public test
  possible.
  - The test should verify formatting for the new ingest-error evidence from a
    constructed report or formatting helper.
  - It must assert that output uses `created_at` and does not mention
    `occurred_at`.
- [x] GREEN 2: Extract the verify output formatting behind a small internal
  function if needed, then make the CLI print the new ingest-error lines.
  - Keep `main` thin and keep database querying in `sync_verification`.
  - Do not add a parallel DTO that mirrors the report just for printing.
- [x] RED 3: If `stored_html_bytes` remains confusing after the first two
  slices, add a compile/test slice that expects the report/CLI/docs term to be
  `extracted_html_bytes`.
- [x] GREEN 3: Rename the field and output key from `stored_html_bytes` to
  `extracted_html_bytes` everywhere it is application/reporting truth.
  - Update docs that describe the current verification interface.
  - Historical `.ralph/reports` may keep old text as evidence.

## Manual Verification

- [x] Run a narrow test after each RED and each GREEN slice.
- [x] Run the final `opentk-sync verify` path against an available migrated test
  database/config if feasible; otherwise rely on the integration test that
  exercises `verify_sync_database` against PostgreSQL.
- [x] Grep current source and docs for the invalid schema names:
  `occurred_at` and `stored_html`.
  - They may remain only in historical reports/task text that documents the bug.
  - Current executable code and current operational guidance must not use them.
- [x] If manual verification shows another guessed-column gap, add the next RED
  test before fixing it.

## Boundary Review

- Use `improve-code-boundaries` before finishing:
  - `sync_verification` owns PostgreSQL verification queries and schema-backed
    report data.
  - The CLI owns only presentation of the verification report.
  - Do not create ad hoc SQL scripts, duplicate schema constants, or stringly
    one-off measurement helpers.
  - Do not swallow query or formatting errors; verification errors must surface.

## Required Gates

- [x] `make check`
- [x] `make test`
- [x] `make lint`
- [ ] Do not run `make test-long`; this is not a story-ending validation task
  and the bug does not explicitly require the e2e/ultra-long lane.

## Completion

- [x] Update this plan checklist as slices are completed.
- [x] Update the bug task acceptance criteria with concrete red/green/manual
  evidence.
- [x] Set `<passes>true</passes>` only after all required gates pass.
- [ ] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] `git add --all`.
- [ ] Commit with:
  `task finished bug-full-sync-post-measurement-query-used-invalid-columns: schema-backed post-sync measurements`
  and include test/check evidence plus any implementation notes.
- [ ] `git push`.

NOW EXECUTE
