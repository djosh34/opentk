# Plan: Typed Env And Lag Errors

Task: `.ralph/tasks/bugs/bug-remove-swallowed-env-and-duration-errors.md`

## Public behavior

- `complete-sync` must treat an invalid-Unicode fallback `DATABASE_URL` as a real CLI error, not as a missing value.
- `PostgresSyncStore::status` must treat impossible lag calculation, such as a `last_fetch_at` timestamp in the future, as a real `SyncStoreError`, not as an absent lag.

## Boundary design

- Apply `improve-code-boundaries` smell 9, typed error boundary:
  - Add a small `CompleteSyncCliError`/`DatabaseUrlError` enum in `crates/opentk-db/src/bin/complete-sync.rs`, using `thiserror::Error`.
  - Replace `std::env::var("DATABASE_URL").ok()` with an explicit `match` that distinguishes `NotPresent` from `NotUnicode`.
  - Return this typed error through the existing `Box<dyn std::error::Error>` command boundary.
- Keep config reduction local to the CLI binary:
  - The public CLI argument remains `Option<String>` because clap owns `OPENTK_DATABASE_URL`.
  - `database_url(argument)` remains the single resolver for explicit argument, clap-provided env, and fallback `DATABASE_URL`.
- Apply typed store errors in `crates/opentk-db/src/sync_state.rs`:
  - Change lag conversion from `Option<Duration>` to `Result<Option<Duration>, SyncStoreError>`.
  - In `status`, use `transpose()?` or an equivalent explicit match so conversion failures stop the status call.
  - Keep valid missing `last_fetch_at` as `Ok(None)`, not an error.

## TDD execution

Use vertical Red-Green cycles only.

1. Red: add one unit test in `complete-sync.rs` for invalid-Unicode fallback `DATABASE_URL`.
   - On Unix, use `std::os::unix::ffi::OsStringExt` to set `DATABASE_URL` to invalid UTF-8 bytes.
   - Call `database_url(None)`.
   - Assert the error text reports invalid Unicode instead of the missing URL message.
   - Keep env mutation scoped with a local guard that restores/removes `DATABASE_URL`.
2. Green: implement the typed env resolver in `complete-sync.rs`.
   - Consider mirroring `crates/opentk-api/src/bin/opentk-api.rs`, but keep the error type local to `complete-sync`.
   - Run the focused test.
3. Red: add one integration test in `crates/opentk-db/tests/sync_state.rs` for a future `last_fetch_at`.
   - Use the existing migrated PostgreSQL pool helper.
   - Insert a `sync_category` row with `last_fetch_at` in the future.
   - Call `store.status(&["Document".to_owned()]).await`.
   - Assert it returns `Err` and the message mentions that sync lag cannot be negative or that `last_fetch_at` is in the future.
4. Green: change `lag_since` and the `status` call site to propagate a `SyncStoreError`.
   - Preserve current successful lag behavior for past timestamps.
   - Run the focused sync_state test.
5. Manual verification:
   - Search for `std::env::var("DATABASE_URL").ok()` and `.to_std().ok()` in relevant crates.
   - If another swallowed instance remains in the task scope, add one Red test for that behavior and repeat.
6. Refactor boundary review:
   - Remove any string bucket `map_err(|err| err.to_string())` introduced by the fix.
   - Keep helper functions only where they own a real boundary; inline tiny one-off helpers if they only obscure status construction.
7. Final verification:
   - Run `make check`.
   - Run `make test`.
   - Run `make lint`.
   - Do not run `make test-long`; this is not a story-finishing task and the task does not require the long lane.
8. Completion:
   - Set `<passes>true</passes>` in the task only after all required checks pass.
   - Run `/bin/bash .ralph/task_switch.sh`.
   - `git add` all files, commit with `task finished bug-remove-swallowed-env-and-duration-errors: ...`, and push.

NOW EXECUTE
