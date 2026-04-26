# Database URL Redaction Result Boundary Plan

Task: `.ralph/tasks/bugs/database-url-redaction-ignores-set-password-error.md`

## Current State

- `crates/opentk-db/src/startup_validation.rs` has a private `redacted_database_url(input: &str) -> String`.
- It parses with `reqwest::Url`, returns the raw input when parsing fails, and calls `let _ = url.set_password(Some("***"));` when a password exists.
- `validate_database_config` uses this helper once to build the database validation target used in reachable and unreachable outcomes.
- Ignoring `set_password` violates the repository rule that errors must not be swallowed.

## Boundary Design

- Replace the stringly private helper with a small explicit boundary in `startup_validation`:
  - `pub fn redact_database_url(input: &str) -> Result<String, DatabaseUrlRedactionError>`
  - `DatabaseUrlRedactionError::InvalidUrl { message: String }`
  - `DatabaseUrlRedactionError::PasswordRedactionFailed`
- Keep URL parsing and password redaction complexity inside that helper.
- `validate_database_config` should call the helper and convert any redaction failure into `DependencyValidationError::DatabaseUrlRedactionFailed { message: String }` before attempting a database connection.
- Preserve existing startup validation public behavior for normal valid URLs:
  - database target has the password replaced with `***`
  - passwordless URLs remain usable as targets
- Do not introduce compatibility shims for the old private helper.

## TDD Execution

- [ ] RED: Add one behavior test in `crates/opentk-db/tests/startup_validation.rs` through the public startup validation boundary. The test should call `validate_api_dependencies` with an unreachable PostgreSQL URL containing a password and assert that the returned database error contains `postgres://postgres:***@127.0.0.1:1/opentk` and does not contain the original password.
- [ ] Run the narrow test command for that test and confirm it fails before implementation.
- [ ] GREEN: Change `redacted_database_url` into `redact_database_url` returning `Result`, handle `Url::set_password` with `map_err`, and wire the error through `validate_database_config`.
- [ ] Run the same narrow test and confirm it passes.
- [ ] Manually verify the bug no longer exists by searching for ignored redaction results in `startup_validation.rs` and confirming there is no `let _ = url.set_password(...)`.
- [ ] If verification finds another swallowed redaction error, add the next RED test before changing implementation.
- [ ] Refactor after GREEN only:
  - keep redaction errors close to URL redaction
  - avoid duplicate target construction
  - keep `DependencyValidationError` responsible for user-facing startup messages
- [ ] Run `make check`.
- [ ] Run `make test`.
- [ ] Run `make lint`.
- [ ] Apply the `improve-code-boundaries` review: verify no new DTO/type duplication, no private/public split with duplicate redaction shapes, and no swallowed errors.
- [ ] Update the bug task acceptance checklist and set `<passes>true</passes>` only after all required checks pass.
- [ ] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Commit all files, including `.ralph` updates, with `task finished database-url-redaction-ignores-set-password-error: ...`.
- [ ] Push the commit.

## Notes For Execution

- The first test intentionally exercises the public dependency validation behavior rather than the private helper.
- If the red test unexpectedly passes because current behavior already masks the password for this URL, switch this plan back to `TO BE VERIFIED` and redesign the test/interface before implementing.
- Execution check on 2026-04-26: the planned public-boundary red test
  `database_validation_redacts_password_from_unreachable_error` passed against
  the existing implementation when run with an isolated target directory:
  `CARGO_TARGET_DIR=/tmp/opentk-codex-target cargo test -p opentk-db --test startup_validation database_validation_redacts_password_from_unreachable_error`.
  The current bug is therefore not captured by that behavior; the plan needs a
  redesigned RED test before production changes.

TO BE VERIFIED
