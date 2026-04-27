## Task: Story 07 Task 03 - Integrate Search CDC Background Task into opentk-api <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Integrate the `SearchCdcListener` from Task 02 as a background task inside `opentk-api` so the inference binary continuously keeps Meilisearch in sync while serving HTTP requests.

Current state:
- `opentk-api` only hosts the HTTP API and queries Meilisearch
- `search-sync` is a separate manual batch binary
- There is no automatic search index maintenance

Implementation:
1. In `opentk-api/src/bin/opentk-api.rs`, spawn the `SearchCdcListener` as a tokio task alongside the Axum server.
2. Use a `tokio::sync::broadcast` shutdown channel. On SIGTERM/SIGINT:
   - Send shutdown signal to CDC listener
   - Gracefully stop the Axum server
   - Wait for CDC listener to finish current batch
3. The CDC listener must not crash the API server. Any errors in the listener must be logged, recorded in shared CDC state, exposed through `/health` and `/admin/search-sync/status`, and reflected by search endpoints returning a clear unavailable/degraded response. The HTTP server stays up for non-search routes.
4. Add a lightweight admin endpoint `GET /admin/search-sync/status` that returns:
   ```json
   {
     "state": "running",
     "last_notification_at": "2026-04-26T12:00:00Z",
     "pending_count": 0,
     "last_batch": {
       "indexed": 42,
       "deleted": 3,
       "failed": 0,
       "duration_ms": 1500
     }
   }
   ```
5. Remove the `search-sync` binary from `crates/opentk-db/Cargo.toml` after integration is complete.

In scope: background task integration, graceful shutdown, admin endpoint, error isolation, removing old binary. Out of scope: Prometheus metrics, pause/resume controls.

</description>


<acceptance_criteria>
- [x] `opentk-api` spawns CDC listener alongside HTTP server
- [x] SIGTERM gracefully shuts down both HTTP and CDC
- [x] CDC errors are logged, recorded in status state, exposed via health/admin endpoints, and do not crash non-search API routes
- [x] `GET /admin/search-sync/status` returns current daemon state
- [x] `search-sync` binary is removed from Cargo.toml
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite)
- [x] `make lint` — passes cleanly
</acceptance_criteria>

<plan>
.ralph/tasks/story-07-binary-rename-and-cdc/task-03-integrate-cdc-into-api_plans/integrate-cdc-into-api-plan.md
</plan>

NOW EXECUTE
