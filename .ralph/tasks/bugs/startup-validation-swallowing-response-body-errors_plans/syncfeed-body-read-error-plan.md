# SyncFeed Body Read Error Plan

Task: `.ralph/tasks/bugs/startup-validation-swallowing-response-body-errors.md`

## Problem

`crates/opentk-db/src/startup_validation.rs` reads the SyncFeed validation response body with:

```rust
let body = response.text().await.unwrap_or_default();
```

That swallows body read failures. For a non-success SyncFeed response with a broken or truncated body, startup validation currently reports an empty `SyncFeedStatus.message` instead of surfacing that the response body could not be read.

## Public Interface

Keep the existing public startup validation API:

- `validate_sync_dependencies(&Config) -> Result<DependencyValidationReport, DependencyValidationError>`
- `DependencyValidationError::SyncFeedStatus { target, status, message }`
- `DependencyValidationError::SyncFeedUnreachable { target, message }`

No new public enum variant is needed unless implementation proves the current shape cannot represent body read failures clearly. The simplest expected contract is:

- HTTP transport or timeout failures remain `SyncFeedUnreachable`.
- Non-success HTTP status remains `SyncFeedStatus`.
- If reading the body for a non-success response fails, the `SyncFeedStatus.message` must explicitly describe that body read error instead of defaulting to an empty string.

## TDD Plan

Use vertical red-green TDD from the `tdd` skill: one failing behavior test, then the smallest production change.

- [x] RED 1: Add one integration test in `crates/opentk-db/tests/startup_validation.rs` that exercises the public `validate_sync_dependencies` path against a local TCP fixture returning a non-success SyncFeed status with a deliberately truncated body.
  - The fixture should send `Content-Length` larger than the bytes written, then close the connection.
  - The test should assert the error is a `SyncFeedStatus` / rendered string with the actual HTTP status and a non-empty message that mentions a body/read/content-length/error condition.
  - Confirm it fails first because `unwrap_or_default()` turns the read failure into an empty message.
- [x] GREEN 1: Change `validate_syncfeed` so response body read errors are mapped explicitly.
  - Keep the status check behavior stable for normal non-success responses.
  - Avoid reading or classifying body errors for successful status responses unless the existing behavior already requires it.
  - Do not swallow `reqwest::Error`; include `source.to_string()` in the returned message.
- [x] Manual verification: Re-run the new test and inspect that the broken-body case now reports an explicit message. If another swallowed SyncFeed startup validation error remains in this scope, add the next RED test before changing production code again.

## Boundary Review

Use `improve-code-boundaries` after green:

- [x] Keep HTTP status/body classification local to startup dependency validation; do not leak a second ad hoc HTTP response shape into callers.
- [x] If `validate_syncfeed` grows stringly branching, extract a small helper such as `syncfeed_status_error(target, response)` returning `DependencyValidationError`.
- [x] Remove `unwrap_or_default`; no fallback default may hide body read failures.
- [x] Scan this module for adjacent swallowed errors introduced by the fix. If unrelated swallowed errors are found, create a bug task rather than hiding them.

## Verification

- [x] Run the targeted new test first during red and green.
- [x] Run `make check`.
- [x] Run `make test`.
- [x] Run `make lint`.
- [x] Do not run `make test-long`; this is a normal bug task and not a story-end validation gate.
- [x] Final boundary check with `improve-code-boundaries`.
- [x] Update `.ralph/tasks/bugs/startup-validation-swallowing-response-body-errors.md` acceptance checkboxes and set `<passes>true</passes>` only after all required checks pass.
- [ ] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Commit all files, including `.ralph`, with `task finished startup-validation-swallowing-response-body-errors: ...`, including test evidence and any implementation notes.
- [ ] Push.

NOW EXECUTE
