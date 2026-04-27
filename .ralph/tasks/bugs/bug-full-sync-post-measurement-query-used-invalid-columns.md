## Bug: Full Sync Post-Measurement Query Used Invalid Columns <status>not_started</status> <passes>true</passes> <priority>medium</priority>

<description>
During Story 10 Task 01 full-sync post-run measurement for run `20260427-044908`,
the first post-sync evidence command guessed two column names that do not exist
in the current schema:

- `ingest_error.occurred_at`
- `document_content.stored_html`

The failed evidence is preserved in
`.ralph/reports/full-sync-post-sync-evidence-20260427-044908.log`.
Corrected measurements using `ingest_error.created_at` and
`document_content.extracted_html` are preserved in
`.ralph/reports/full-sync-corrected-measurements-20260427-044908.log`.

This did not make the sync result questionable because the corrected queries
completed successfully and `ingest_error` remained empty, but the measurement
workflow should not rely on guessed schema names.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with
Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
  - RED evidence: `cargo test -p opentk-db --bin opentk-sync verify_output_reports_schema_backed_ingest_errors_and_extracted_html_bytes` failed because `format_verification_report` was missing and `StorageVerification` had no `extracted_html_bytes` field.
- [x] I made the test green by fixing
  - GREEN evidence: the CLI formatter test passed, and `./scripts/cargo-test-with-postgres.sh -p opentk-db --test sync_verification verification_reports_recent_ingest_errors_from_schema_columns` passed against local PostgreSQL.
- [x] I manually verified the bug, and created a new Red test if not working still
  - Manual evidence: `rg -n "occurred_at|stored_html" crates docs --glob '!target'` found no current source or docs references.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — not impacted and not run for this normal bug task.
</acceptance_criteria>

Plan: `.ralph/tasks/bugs/bug-full-sync-post-measurement-query-used-invalid-columns_plans/schema-backed-post-measurement-plan.md`

NOW EXECUTE
