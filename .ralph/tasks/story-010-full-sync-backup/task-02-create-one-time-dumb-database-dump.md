## Task: Story 010 Task 02 - Create One-Time Dumb Dump of Fully Synced Database <status>done</status> <passes>true</passes>

<description>
**Goal:** After Story 010 Task 01 has produced a fully synced database, take one plain operational database dump of that exact state so development can revert after accidental wipe/truncate or other destructive mistakes.

This is an operational safeguard, not a product feature. Do not add a CLI command. Do not add a script. Do not add application code. Do not add restore/catch-up logic. Use standard PostgreSQL tooling manually.

Requirements:
1. Confirm Story 010 Task 01 completed successfully and identify the report file it produced.
2. Confirm the database being dumped is the same fully synced database measured in that report.
3. Use standard PostgreSQL dump tooling directly to create a dump file from the fully synced database.
4. Store the dump in a clearly named local path appropriate for Ralph/development artifacts.
5. The dump file must not be committed to git. Store it outside tracked source paths, or under a path that is ignored by git.
6. Record:
   - dump command used
   - dump start timestamp
   - dump completion timestamp
   - dump duration
   - dump file path
   - dump file size
   - dump checksum
   - source database identity
   - source full-sync report path
   - git commit SHA and dirty/clean worktree state
7. Verify the dump is readable using standard PostgreSQL tooling. If practical, restore into a disposable database and verify representative row counts match the full-sync report.
8. Run `git status --short` before completing the task and verify the dump file is not listed as tracked, staged, or untracked.
9. Append dump details and verification results to the `.ralph/reports/full-sync-report-YYYYMMDD-HHMMSS.md` file from Task 01, or create `.ralph/reports/full-sync-dump-YYYYMMDD-HHMMSS.md` that links to the Task 01 report. The report may include dump path, size, checksum, and verification results, but must not embed dump contents.
10. Use the `email-me` skill to email the dump summary/report update to the user.

If dump creation, dump readability verification, or disposable restore verification fails, stop and file an add-bug task immediately with the exact command, output, and affected path.

In scope: one-time manual dump, dump verification, report update, email notification, add-bug filing on issues. Out of scope: new code, new CLI commands, new scripts, product backup feature, restore feature, automated scheduling.
</description>

<acceptance_criteria>
- [x] Story 010 Task 01 completed successfully before the dump was taken
- [x] Dump was created from the exact fully synced database measured in the full-sync report
- [x] Dump file is stored outside tracked source paths or under a git-ignored path
- [x] Dump file is not committed, staged, tracked, or shown as untracked by `git status --short`
- [x] Dump file path, size, and checksum were recorded
- [x] Dump command, timestamps, duration, source database identity, source report path, commit SHA, and worktree state were recorded
- [x] Dump readability was verified with standard PostgreSQL tooling
- [x] If practical, disposable restore verification was performed and representative row counts matched the full-sync report
- [x] Report was updated or a linked dump report was created under `.ralph/reports/` without embedding dump contents
- [x] Dump summary/report update was emailed to the user using the `email-me` skill
- [x] Any failure or questionable result was immediately filed as an add-bug task
- [x] No application code, CLI command, script, or backup/restore product behavior was added
</acceptance_criteria>
