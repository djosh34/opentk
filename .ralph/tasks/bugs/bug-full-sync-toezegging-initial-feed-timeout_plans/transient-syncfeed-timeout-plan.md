# Transient SyncFeed Timeout Plan

## Task

Fix the full-sync blocker where the one-time clean sync failed on the initial
`Toezegging` `SyncFeed` page after the configured request retry budget was
exhausted.

The user-facing behavior is the public sync runner:

- `CompleteSyncRunner::run_once` in `SyncRunMode::UntilCaughtUp` must be able
  to complete a category when an initial `SyncFeed` request times out
  transiently and the same cursor succeeds later.
- Durable `ingest_error` rows must remain reserved for a final failed category,
  not an internal retry attempt that later succeeds.
- Non-transient failures, parse failures, malformed cursors, and write failures
  must still stop the run and record explicit durable errors.

## Current Boundary

The timeout is produced in `crates/opentk-sync/src/syncfeed.rs`:

- `SyncFeedClient::fetch_page` retries retryable HTTP statuses and timeouts
  within the configured `max_retries` budget.
- When all timeout attempts fail, `fetch_page_with_retries` returns
  `SyncFeedClientError::Timeout` directly.

The run-stopping decision lives in `crates/opentk-sync/src/runner.rs`:

- `run_category` calls `client.fetch_page(cursor.clone()).await`.
- Any fetch error is immediately recorded through `SyncStore::record_error`.
- The category worker then returns `CompleteSyncError::Fetch`, which makes
  `run_once` fail the whole full sync.

The code boundary should stay that way: HTTP classification belongs in
`syncfeed.rs`; category-level recovery belongs in `runner.rs`; durable error
storage belongs behind `SyncStore`.

## Interface Design

Keep the external CLI and config file shape unchanged for the first slice:

- Do not add a compatibility flag.
- Do not swallow errors silently.
- Do not special-case `Toezegging`; the problem is transient `SyncFeed`
  availability for any category.
- Keep `SyncFeedClient::fetch_page` as the one-page fetch interface.

Preferred implementation shape:

- Add a small category-level transient fetch retry helper in `runner.rs`.
- Retry only `SyncFeedClientError::Timeout` and possibly
  `SyncFeedClientError::RetryExhausted` when its `last_error` represents a
  timeout, if the implementation first normalizes timeout exhaustion.
- Reuse the same `SyncFeedCursor` for each retry so no cursor state advances
  before a page is successfully fetched and written.
- Use `CompleteSyncConfig::poll_interval` as the existing runner pacing
  interval for category-level transient retry sleeps, unless the Red test shows
  the interface needs an explicit retry interval.
- Record a durable fetch error only after the runner has decided the category
  cannot recover. Do not create `ingest_error` rows for attempts that later
  succeed.

## TDD Plan

Use vertical Red-Green cycles. Do not write all tests first.

- [x] Red 1: Add one public runner integration test in
  `crates/opentk-sync/tests/opentk_sync_runner.rs`.
  - Behavior: `CompleteSyncRunner::run_once` completes `Toezegging` when the
    first fetch attempt for
    `/SyncFeed/2.0/Feed?category=Toezegging&content=internal` times out and a
    later fetch of the same cursor returns a valid empty resume page.
  - Test setup: configure the client with a very short `request_timeout`,
    `max_retries = 0`, and a tiny `poll_interval`; make the mock server return a
    delayed response first, then a valid resume page for the same target.
  - Assertions: the report marks `Toezegging` caught up, the stored cursor is
    caught up with the resume skiptoken, and the memory store has no durable
    fetch error for the transient timeout.
  - Expected initial result: the test fails with `CompleteSyncError::Fetch`
    and a durable fetch error is present.
- [x] Green 1: Make only that test pass.
  - Add the narrow runner retry helper for transient fetch errors.
  - Keep page parsing, writing, cursor extraction, and store code unchanged
    unless the Red result proves a boundary leak.
  - Keep non-timeout fetch errors on the existing fail-fast durable-error path.
- [ ] Red 2, only if manual verification still fails with the same transient
  timeout shape: add the next narrow public test that captures the exact
  remaining behavior.
- [ ] Green 2, only if Red 2 is needed: fix only the behavior captured by that
  Red test.

## Manual Verification

- [x] Run the focused runner test by exact name after each Red/Green cycle.
- [ ] Run any nearby focused client test if `syncfeed.rs` timeout exhaustion
  semantics are changed.
- [ ] Re-run the captured full-sync command from the bug report if the same
  database and local services are still available:
  `cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-213953.toml run`
- [ ] If the rerun progresses past the initial `Toezegging` timeout but exposes
  a new unrelated sync failure, create a separate bug task and keep this fix
  scoped to transient fetch recovery.

## Boundary Review

- [x] Final improve-code-boundaries pass: timeout retry policy must live at the
  `syncfeed`/runner boundary, not in storage, payload parsing, or category
  schema code.
- [x] Remove any duplicate retry loops or ad hoc string checks introduced while
  getting Green.
- [x] Ensure durable errors are not swallowed: final failures must still be
  recorded and returned explicitly.
- [x] Keep the TDD test behavior-focused through `CompleteSyncRunner::run_once`;
  do not test private helper functions.

## Final Checks

- [ ] `make check`
- [ ] `make test`
- [ ] `make lint`
- [ ] Do not run `make test-long`; this is a normal bug task unless manual
  verification proves it must become a story-end validation gate.

NOW EXECUTE
