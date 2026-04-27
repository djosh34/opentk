# Transient SyncFeed Body Decode Plan

Task: `.ralph/tasks/bugs/bug-full-sync-toezegging-http-transport-decode-error.md`

## Problem

A clean full sync run failed while fetching the initial `Toezegging` SyncFeed page:

```text
SyncFeed HTTP transport failed for https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Toezegging&content=internal: error decoding response body
```

The runner already has a high-level transient fetch recovery loop for initial live SyncFeed instability, but `is_transient_fetch_error` only treats `SyncFeedClientError::Timeout` as transient. Body read/decode failures from `response.text().await` are currently surfaced as `SyncFeedClientError::HttpTransport`, so a single transient truncated/broken HTTP body is recorded durably and stops the whole full sync.

## Public Interface

Keep the public interfaces unchanged:

- `SyncFeedClient::fetch_page(cursor) -> Result<SyncFeedPage, SyncFeedClientError>`
- `CompleteSyncRunner::run_once() -> Result<CompleteSyncReport, CompleteSyncError>`
- `SyncFeedClientError::HttpTransport { request_url, message }`
- durable fetch error recording through `SyncStore::record_error`

Do not add a new public error variant unless execution proves that the existing `HttpTransport` variant cannot express the failure. The desired behavior belongs in the runner's transient recovery policy, not in callers or database code.

## TDD Plan

Use vertical Red-Green TDD from the `tdd` skill: one behavior test, make it green, then verify whether the real bug still holds.

- [x] RED 1: Add one integration test in `crates/opentk-sync/tests/opentk_sync_runner.rs` through the public `CompleteSyncRunner::run_once` boundary.
  - Scenario: category `Toezegging` starts from no stored cursor.
  - Fixture first response for `/SyncFeed/2.0/Feed?category=Toezegging&content=internal` must be an HTTP 200 Atom response with a deliberately broken body transfer, for example `Content-Length` larger than the bytes actually written before closing.
  - Fixture second response for the same cursor must be a valid empty resume Atom page.
  - Configure `SyncFeedClientConfig.max_retries = 0` so only runner-level transient recovery can make the test pass.
  - Assert `run_once()` succeeds, the category is caught up, the resume cursor is stored, no durable errors were recorded, and the same initial cursor was requested twice.
  - Confirm the test fails first because `SyncFeedClientError::HttpTransport` is not considered transient.
- [x] GREEN 1: Extend the runner fetch recovery classifier so body/transport failures are retried in the same recovery loop as timeouts.
  - Keep this private to `crates/opentk-sync/src/runner.rs`.
  - Prefer matching `SyncFeedClientError::Timeout { .. } | SyncFeedClientError::HttpTransport { .. }` over string-matching the error message.
  - Do not retry parser errors, invalid content type, cursor errors, non-retryable HTTP status errors, or write/parse/store errors.
- [x] Manual verification: run the targeted runner test. If it still records a durable error or needs a broader interface/type change, switch this plan back to `TO BE VERIFIED` and stop before changing the design further.

## Boundary Review

Use `improve-code-boundaries` after green:

- [x] Keep transient fetch policy in the runner, because the runner owns durable error timing and cursor retry behavior.
- [x] Keep `SyncFeedClient` responsible for one HTTP request plus parsing only; do not hide retry semantics inside low-level body reads.
- [x] Avoid string buckets: classify by `SyncFeedClientError` variants, not by the text `error decoding response body`.
- [x] Keep the test through public runner behavior, not private helper behavior.
- [x] If the test fixture needs broken-response support, add the smallest private test-only shape to `TestResponse` instead of introducing production abstractions.

## Verification

- [x] Run the new targeted test in red and green.
- [x] Run `make check`.
- [x] Run `make test`.
- [x] Run `make lint`.
- [x] Do not run `make test-long`; this is a normal bug task and not a story-end validation gate.
- [x] Final boundary check with `improve-code-boundaries`.
- [x] Update `.ralph/tasks/bugs/bug-full-sync-toezegging-http-transport-decode-error.md` acceptance checkboxes and set `<passes>true</passes>` only after all required checks pass.
- [ ] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Commit all files, including `.ralph`, with `task finished bug-full-sync-toezegging-http-transport-decode-error: ...`, including test evidence and implementation notes.
- [ ] Push.

EXECUTED
