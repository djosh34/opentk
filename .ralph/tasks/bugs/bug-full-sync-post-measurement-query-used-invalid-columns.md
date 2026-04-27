## Bug: Full Sync Post-Measurement Query Used Invalid Columns <status>not_started</status> <passes>false</passes> <priority>medium</priority>

<description>
During Story 10 Task 1 full-sync post-run measurement for run `20260427-044908`,
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
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
