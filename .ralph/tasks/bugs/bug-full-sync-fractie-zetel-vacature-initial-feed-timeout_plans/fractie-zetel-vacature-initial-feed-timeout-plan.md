# FractieZetelVacature Initial Timeout Plan

## Task

Fix the full-sync blocker where a clean one-time `SyncFeed` run failed on the
initial `FractieZetelVacature` page after the existing timeout retry budget was
exhausted.

The public behavior to protect is `CompleteSyncRunner::run_once`:

- It must not record a durable fetch error for a transient initial-page timeout
  that later succeeds from the same cursor.
- It must keep cursor state unchanged until a page is fetched, parsed, and
  written or marked caught up.
- It must still return and durably record final fetch failures after the bounded
  recovery policy is exhausted.
- It must not special-case `FractieZetelVacature`; the bug is a slow or
  temporarily unavailable initial `SyncFeed` category.

## Current Boundary

The prior `Toezegging` timeout fix already added a runner-level helper in
`crates/opentk-sync/src/runner.rs`:

- `SyncFeedClient::fetch_page` in `crates/opentk-sync/src/syncfeed.rs` handles
  one-page HTTP behavior and the configured per-request retry policy.
- `run_category` calls `fetch_page_with_transient_recovery` and records a
  durable fetch error only after that helper returns an error.
- The helper currently retries `SyncFeedClientError::Timeout` only a small
  fixed number of runner-level attempts.

The new failure lasted through that bounded recovery window while a full run was
processing many categories. The fix should deepen the runner retry policy
without moving HTTP classification into storage or payload parsing.

## Interface Design

Keep the public CLI and config file shape unchanged for this slice unless the
Red test proves that the recovery budget must become configurable.

Preferred implementation shape:

- Replace the hard-coded tiny runner recovery loop with a named retry policy in
  `runner.rs`.
- Keep the policy local to the sync runner boundary:
  - client-level retries remain in `syncfeed.rs`;
  - category-level recovery remains in `runner.rs`;
  - durable error persistence remains behind `SyncStore`.
- Retry only transient fetch errors:
  - `SyncFeedClientError::Timeout`;
  - `SyncFeedClientError::RetryExhausted` only if it represents transient fetch
    exhaustion, and only if the current code shape actually returns that variant
    for timeout paths.
- Use exponential backoff with a bounded total retry window for initial and
  in-progress category fetches. The window must be long enough to outlast the
  captured `FractieZetelVacature` initial feed timeout pattern without making a
  permanently broken category loop forever.
- Keep all errors explicit:
  - intermediate transient failures are not inserted into `ingest_error`;
  - the final exhausted failure is returned and recorded with the current
    category and skiptoken.

## TDD Plan

Use vertical Red-Green cycles. Make one test fail, make that one test pass, then
decide whether another Red test is needed.

- [x] Red 1: Add one behavior test in
  `crates/opentk-sync/tests/opentk_sync_runner.rs`.
  - Behavior: `CompleteSyncRunner::run_once` completes a category when the same
    initial cursor times out more times than the current tiny runner recovery
    budget, then returns a valid empty resume page.
  - Use category `FractieZetelVacature` so the test documents the production
    failure, but assert generic runner behavior rather than category-specific
    branching.
  - Configure the client with `request_timeout = 10ms`, `max_retries = 0`, and
    a tiny runner `poll_interval`.
  - Mock server responses: delayed initial resume page repeated enough times to
    fail the current policy, followed by a successful resume page for the same
    target.
  - Assertions: report marks the category caught up, stored cursor uses the
    resume skiptoken, no durable fetch error was recorded, and the server saw
    repeated requests to exactly the same initial cursor.
  - Expected initial result: fails with `CompleteSyncError::Fetch` and a durable
    fetch error after the existing recovery attempts.
- [x] Green 1: Make only Red 1 pass.
  - Update the runner recovery helper/policy.
  - Do not change payload parsing, PostgreSQL writing, category schemas, or CLI
    config unless the Red result proves they are part of the bug.
  - Keep non-transient fetch errors on the existing fail-fast durable-error
    path.
- [x] Red 2 was not needed: manual verification advanced
  `FractieZetelVacature` to `caught_up`.
- [x] Green 2 was not needed.

## Manual Verification

- [x] Run the focused runner test by exact name after each Red-Green cycle.
- [x] `syncfeed.rs` was unchanged, so nearby `syncfeed_client` retry tests were
  not required by this plan item.
- [x] Manually verify the production bug shape without running `make test-long`:
  - If the original database and service are still available, rerun the captured
    command with the same config:
    `CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260427-033618.toml run`
  - Capture output under `.ralph/reports/`.
  - If the run progresses past the `FractieZetelVacature` initial timeout but
    exposes a different blocker, create a separate bug task and keep this task
    scoped.
  - Result: the captured command was run against the reachable local database
    and interrupted after scoped verification showed `FractieZetelVacature`
    `caught_up` at skiptoken `25174173`. Remaining runtime was other large
    categories still `running`, not the original blocker. Evidence:
    `.ralph/reports/full-sync-fractie-zetel-vacature-timeout-manual-20260427-codex.log`
    and
    `.ralph/reports/full-sync-fractie-zetel-vacature-timeout-manual-status-20260427-codex.log`.

## Boundary Review

Use `improve-code-boundaries` before final checks:

- [x] Verify there is one clear transient fetch recovery boundary in
  `runner.rs`; remove duplicate retry loops or stringly error checks if they
  appear.
- [x] Keep the retry policy expressed through typed `SyncFeedClientError`
  variants, not message substring matching.
- [x] Ensure durable errors are not swallowed: final failures must still be
  recorded through `SyncStore::record_error` and returned to the caller.
- [x] Keep tests behavior-focused through `CompleteSyncRunner::run_once`; do not
  test private helper functions.

## Final Checks

- [x] `make check`
- [x] `make test`
- [x] `make lint`
- [x] Do not run `make test-long`; this is a normal bug task unless a later
  story-end validation explicitly asks for it.

NOW EXECUTE
