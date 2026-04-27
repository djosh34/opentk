# Plan: Integrate Search CDC into opentk-api

## Current Read

Relevant existing surfaces:

- `crates/opentk-api/src/bin/opentk-api.rs` loads config, validates optional dependencies, and calls `opentk_api::serve(api_config(config))`.
- `crates/opentk-api/src/lib.rs` owns `ApiConfig`, `build_app`, `serve`, `router_with_search`, `/health`, `/search`, and OpenAPI construction.
- `ApiState` currently contains only `PgPool` and an `Arc<dyn SearchQueryClient>`.
- `configured_search_client` probes Meilisearch at startup and swaps in `UnavailableSearchClient` if the probe fails.
- `crates/opentk-db/src/search_cdc.rs` exposes `SearchCdcListener::connect(database_url, pool, client, search_config, batch_config)`, `receive_once`, and `flush`, but it does not yet expose an owned run loop or shared runtime status.
- `SearchCdcListener` borrows its search index client, which is fine for tests but awkward for a spawned `'static` API background task.
- `MeilisearchClient` implements both `SearchQueryClient` and `SearchIndexClient`.
- `crates/opentk-db/src/bin/search-sync.rs` still exists and is auto-discovered by Cargo as a binary even though `Cargo.toml` only explicitly declares `opentk-sync`.

## Public Interface

Keep the public API small and behavior-oriented:

- Add `SearchCdcRuntimeState` in `opentk-db::search_cdc` with stable JSON-friendly values:
  - `starting`
  - `running`
  - `degraded`
  - `stopped`
- Add `SearchCdcBatchReport` in `opentk-db::search_cdc` with:
  - `indexed: u64`
  - `deleted: u64`
  - `failed: u64`
  - `duration_ms: u64`
- Add `SearchCdcStatus` in `opentk-db::search_cdc` with:
  - `state: SearchCdcRuntimeState`
  - `last_notification_at: Option<DateTime<Utc>>`
  - `pending_count: usize`
  - `last_batch: Option<SearchCdcBatchReport>`
  - `last_error: Option<String>`
- Add `SearchCdcRuntimeStatus` as the shared concurrency boundary:
  - `new() -> Self`
  - `snapshot() -> SearchCdcStatus`
  - methods for transition events such as `mark_running`, `record_notification`, `record_batch`, `record_error`, `mark_stopped`
- Add a CDC daemon run function in `opentk-db::search_cdc`:
  - `run_search_cdc_listener(database_url, pool, client, search_config, batch_config, shutdown, status) -> Result<(), SearchCdcError>`
  - It owns the loop, records status events, uses `tokio::sync::broadcast::Receiver<()>` for shutdown, and flushes a current batch before returning on shutdown.
- Keep `SearchCdcListener` focused on LISTEN/NOTIFY, batching, and targeted indexing. Do not move Axum, routing, or API response types into `opentk-db`.
- In `opentk-api`, add an API response DTO for `GET /admin/search-sync/status`; do not expose internal Rust status structs directly if the endpoint needs API-specific field naming.

## TDD Plan

Use the `tdd` skill exactly as a vertical red-green loop. One behavior test first, then the smallest implementation for that behavior, then repeat.

1. RED: add an `opentk-db` unit test that a fresh `SearchCdcRuntimeStatus` snapshots as `starting` with no notification, no batch, no error, and zero pending records.
   GREEN: implement `SearchCdcRuntimeState`, `SearchCdcStatus`, `SearchCdcRuntimeStatus`, and a `snapshot` method behind `Arc<Mutex<_>>` or `Arc<RwLock<_>>`.

2. RED: add an `opentk-db` unit test that status transitions record `running`, a notification timestamp, pending count, last batch metrics, and `degraded` with an error string.
   GREEN: implement narrow event methods. Do not let callers mutate fields directly.

3. RED: add an `opentk-db` async test for the daemon loop using a migrated PostgreSQL schema and `MemoryIndexClient`: start `run_search_cdc_listener`, insert a `sync_entity` row, wait until status shows a successful batch, then send shutdown and assert the task exits.
   GREEN: implement the daemon loop around `SearchCdcListener::connect`, `tokio::select!`, `receive_once`, status updates, and broadcast shutdown.

4. RED: add an `opentk-db` async test where the index client returns an error from `apply_batch`; assert the daemon records `degraded` plus `last_error` and stays alive until shutdown instead of returning the indexing error.
   GREEN: isolate per-notification/per-batch errors in the loop: log them, update status, and continue listening. Startup connection errors should still return `Err` because the daemon never started.

5. RED: add an `opentk-api` router test for `GET /admin/search-sync/status` using a manually supplied status handle; assert JSON shape matches the task contract including `state`, `last_notification_at`, `pending_count`, and `last_batch`.
   GREEN: extend `ApiState`, add route handler, response DTOs, route registration, and OpenAPI path/component entries.

6. RED: add an `opentk-api` health test that sets CDC status to `degraded`; assert `/health` returns `503` with a clear JSON error code such as `search_sync_degraded`.
   GREEN: update health to check database first, then CDC status. Non-search routes must still be served by the same router.

7. RED: add an `opentk-api` search test that sets CDC status to `degraded`; assert `/search?q=fixture` returns `503` with a clear code such as `search_sync_degraded` without calling the query client.
   GREEN: guard `search::search` through the shared status handle before hitting Meilisearch.

8. RED: add an `opentk-api` startup/server behavior test that builds the API with CDC enabled and proves `serve` can be driven by an injected shutdown future or helper without relying on OS signals.
   GREEN: refactor `serve` into a testable helper, for example `serve_with_shutdown(config, shutdown_future)`, that creates a broadcast channel, starts the CDC daemon task, serves Axum with graceful shutdown, sends CDC shutdown, and awaits the CDC task.

9. RED: add or update a CLI/startup test proving `opentk-api` uses the database URL, search config, sync categories, search batch size, and retry limit to create CDC search sync config.
   GREEN: extend `ApiConfig` with CDC/search-sync fields or pass enough config into `serve` so the daemon does not reconstruct config from globals.

10. RED: add a repository-level assertion or adjust an existing manifest/binary test so `search-sync` is no longer a Cargo binary.
    GREEN: delete `crates/opentk-db/src/bin/search-sync.rs` and remove any stale manifest entry if one appears during execution.

## API Runtime Design

`opentk-api` startup should produce one Meilisearch client instance that can be shared by query and indexing paths:

- Build `Arc<MeilisearchClient>` for the configured backend.
- If startup search validation fails, record CDC/search status as degraded and expose `UnavailableSearchClient` for queries, but keep non-search routes alive.
- Pass an `Arc<dyn SearchQueryClient + Send + Sync>` to `ApiState`.
- Pass an owned `Arc<MeilisearchClient>` or similar concrete `SearchIndexClient` owner into the CDC task. If the trait shape blocks this cleanly, prefer changing `SearchCdcListener` from borrowing `&C` to owning `Arc<C>` over adding adapter layers.

Shutdown flow:

- `serve` binds TCP, creates `tokio::sync::broadcast::channel(1)`, and spawns CDC with a receiver.
- `axum::serve(...).with_graceful_shutdown(signal_or_injected_shutdown)` handles HTTP shutdown.
- When the shutdown future resolves, send one broadcast message.
- Await the HTTP server result and the CDC join handle.
- If the CDC task reports an isolated runtime error, it should already be in status and logs. Do not crash the API for per-batch CDC errors.
- If the CDC task panics or startup join handling fails, return an `ApiError` variant after HTTP shutdown in tests. Do not ignore `JoinError`.

Signal handling:

- The production entrypoint should use a helper that waits for `tokio::signal::ctrl_c()` and, on Unix, `SIGTERM`.
- Keep signal code isolated in the API binary or a small `shutdown_signal` helper so tests can inject a future without sending process signals.

## Improve Code Boundaries Plan

Use the `improve-code-boundaries` skill during execution and final review:

- Put shared CDC status and daemon loop in `opentk-db::search_cdc`, where CDC concepts already live.
- Keep HTTP JSON DTOs, status-code mapping, and OpenAPI registration in `opentk-api`.
- Do not let handlers inspect raw listener errors or raw notification payloads.
- Do not duplicate status shapes as unrelated API/database structs unless the API DTO needs serialization naming; map from one domain snapshot into one response DTO at the boundary.
- Avoid stringly state checks in handlers; use `SearchCdcRuntimeState` methods like `is_search_usable()` or match the enum.
- Delete the old `search-sync` binary rather than keeping an unreferenced manual path. Greenfield means no backwards compatibility.
- Final boundary review: search for duplicated CDC state enums, raw `serde_json::Value` CDC status construction, swallowed `JoinError`, ignored send/receive errors, and any `let _ =` around fallible shutdown or task results.

## Acceptance Mapping

- `opentk-api` spawns CDC listener alongside HTTP server: covered by startup/server behavior test and implementation in `serve`.
- SIGTERM/SIGINT gracefully shut down HTTP and CDC: production signal helper plus injected-shutdown test for the same serve path.
- CDC errors are logged, recorded in status, exposed via health/admin, and do not crash non-search routes: covered by daemon error isolation test, health test, admin endpoint test, and search degraded test.
- `GET /admin/search-sync/status` returns current daemon state: covered by router JSON test.
- Search endpoints return clear degraded/unavailable response: covered by search degraded test before calling query client.
- `search-sync` binary removed: covered by deleting `crates/opentk-db/src/bin/search-sync.rs` and a repository check if there is an existing binary inventory test.
- Required final gates: run `make check`, `make test`, and `make lint`. Do not run `make test-long` for this normal task.

## Execution Checklist

- [x] Add RED status initial snapshot test.
- [x] Implement CDC status types and snapshot.
- [x] Add RED status transition test.
- [x] Implement event methods.
- [x] Add RED daemon success/shutdown test.
- [x] Implement daemon loop and graceful flush on shutdown.
- [x] Add RED daemon indexing-error isolation test.
- [x] Implement logging, degraded status, and continue-on-runtime-error behavior.
- [x] Add RED admin status endpoint test.
- [x] Implement admin endpoint, route state, DTOs, and OpenAPI entries.
- [x] Add RED health degraded test.
- [x] Implement health degraded response.
- [x] Add RED search degraded test.
- [x] Implement search degraded guard without calling query backend.
- [x] Add RED serve-with-injected-shutdown/startup test.
- [x] Implement broadcast shutdown orchestration and CDC task spawning.
- [x] Add RED config/search-sync wiring test.
- [x] Implement API config extensions for CDC search sync config.
- [x] Add RED binary removal assertion if needed.
- [x] Delete `crates/opentk-db/src/bin/search-sync.rs` and stale manifest entries if present.
- [x] Run `make check`.
- [x] Run `make test`.
- [x] Run `make lint`.
- [x] Final improve-code-boundaries review and cleanup.

NOW EXECUTE
