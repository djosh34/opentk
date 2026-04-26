# Database URL Redaction Boundary Plan

Task: `.ralph/tasks/bugs/database-url-redaction-ignores-set-password-error.md`

## Current State

- `crates/opentk-db/src/startup_validation.rs` has a private `redacted_database_url(input: &str) -> String`.
- The helper parses with `reqwest::Url`, returns the raw input when parsing fails, and calls `let _ = url.set_password(Some("***"));` when a password exists.
- `Url::set_password` can fail for URLs that cannot accept password mutation.
- Because the helper returns only `String`, callers cannot distinguish a safely redacted target from a failed redaction attempt.
- The previous public dependency-validation test with an unreachable PostgreSQL URL was not RED: PostgreSQL URLs allow password mutation, so the current implementation already masks that normal case.
- Execution finding on 2026-04-26: `file://postgres:secret@localhost/opentk` is not a valid fixture. `reqwest::Url::parse` returns `InvalidUrl { message: "invalid international domain name" }`, so it never reaches `Url::set_password`.
- Verification against `url` 2.5.8 source showed `set_password` returns `Err(())` when the URL has no host, has an empty domain host, or uses `file`. The parser rejects the attempted credential-bearing fixtures for those cases before `password()` can be `Some`, so the reliable behavior to test is the new explicit redaction boundary, followed by source verification that the mutation result is handled instead of swallowed.

## Boundary Design

- Replace the private string helper with an explicit public boundary in `startup_validation`:
  - `pub fn redact_database_url(input: &str) -> Result<String, DatabaseUrlRedactionError>`
  - `pub enum DatabaseUrlRedactionError { InvalidUrl { message: String }, PasswordRedactionFailed }`
- Keep URL parsing and password mutation details inside that helper.
- `validate_database_config` should call `redact_database_url(&config.url)?` before connecting.
- Add `DependencyValidationError::DatabaseUrlRedactionFailed { message: String }` and convert helper errors into that variant at the startup validation boundary.
- Do not preserve the old private `redacted_database_url` helper or add compatibility shims.
- This is a boundary cleanup, not a compatibility change: callers get a typed failure before any database connection attempt if the configured URL cannot be safely represented for diagnostics.

## TDD Execution

- [x] RED: Add one behavior test in `crates/opentk-db/tests/startup_validation.rs` through the new public redaction boundary. The test should call `redact_database_url("postgres://postgres:secret@127.0.0.1:1/opentk")` and assert `Ok("postgres://postgres:***@127.0.0.1:1/opentk")`.
- [x] Run the narrow test command for that test and confirm it fails before implementation because the public redaction boundary does not exist yet.
- [x] GREEN: Introduce `DatabaseUrlRedactionError`, change redaction to `redact_database_url(input: &str) -> Result<String, DatabaseUrlRedactionError>`, and map `Url::parse` failures to `InvalidUrl { message }`.
- [x] In the same GREEN slice, replace `let _ = url.set_password(Some("***"));` with `url.set_password(Some("***")).map_err(|_| DatabaseUrlRedactionError::PasswordRedactionFailed)?;` so the result is explicit even though current `url` fixtures do not expose the failure path after `password().is_some()`.
- [x] Wire `validate_database_config` through `redact_database_url(&config.url)?` and add the `DependencyValidationError` conversion needed for `?`.
- [x] Run the same narrow test and confirm it passes.
- [x] Manual bug verification: search `crates/opentk-db/src/startup_validation.rs` for `let _ =` and `set_password` and confirm no password-redaction result is ignored.
- [x] If verification finds another swallowed redaction error, add the next RED test before changing implementation.
- [x] Refactor after GREEN only:
  - keep the redaction error type next to the redaction helper
  - keep connection validation focused on validation, not string surgery
  - avoid duplicate redaction DTOs or stringly error translation layers
- [x] Run `make check`.
- [x] Run `make test`.
- [x] Run `make lint`.
- [x] Apply the `improve-code-boundaries` review: verify no duplicate helper/error shapes, no swallowed errors, and no extra compatibility layer for the old private helper.
- [x] Update this task acceptance checklist and set `<passes>true</passes>` only after all required checks pass.
- [x] Run `/bin/bash .ralph/task_switch.sh`.
- [x] Commit all files, including `.ralph` updates, with `task finished database-url-redaction-ignores-set-password-error: ...`.
- [ ] Push the commit.

## Notes For Execution

- The RED test intentionally targets the redaction boundary directly. A full dependency-validation test with an unreachable PostgreSQL URL cannot prove the bug because normal PostgreSQL URLs already allow password mutation.
- The helper should redact normal password-bearing PostgreSQL URLs and should return passwordless valid URLs unchanged.
- `InvalidUrl` makes parse failures explicit as part of the same boundary cleanup; if this broader parse behavior needs a separate RED test, add it only after the password-redaction failure test is GREEN.
- `PasswordRedactionFailed` may be hard to trigger through public `Url::parse` with a password-bearing URL in the current `url` crate. It still belongs in the typed boundary because the called API returns `Result` and repository instructions forbid discarding it.

NOW EXECUTE
