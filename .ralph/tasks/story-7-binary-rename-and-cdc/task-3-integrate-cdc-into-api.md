## Task: Story 7 Task 3 - Integrate Search CDC Background Task into opentk-api <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Integrate the `SearchCdcListener` from Task 2 as a background task inside `opentk-api` so the inference binary continuously keeps Meilisearch in sync while serving HTTP requests.

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
3. The CDC listener must never crash the API server. Any errors in the listener should be logged but the HTTP server stays up.
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
- [ ] `opentk-api` spawns CDC listener alongside HTTP server
- [ ] SIGTERM gracefully shuts down both HTTP and CDC
- [ ] CDC errors are logged but do not crash the API server
- [ ] `GET /admin/search-sync/status` returns current daemon state
- [ ] `search-sync` binary is removed from Cargo.toml
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
