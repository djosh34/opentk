## Task: Story 11 Task 01 - Enhance Health Endpoint and Add Admin Search Sync Status <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Make `/health` comprehensive and add `/admin/search-sync/status` so operators can see search CDC state, while keeping the API usable when search is unavailable.

Requirements:
1. Enhanced `/health`:
   ```json
   {
     "status": "ok",
     "database": "ok",
     "search": "ok",
     "search_sync": {
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
   }
   ```
   When PostgreSQL is healthy but Meilisearch is unreachable, return 200 with `"status": "degraded"`, `"search": "unavailable"`, and a clear search error field. The API is still healthy enough to serve non-search routes. Return 503 only when PostgreSQL or another required core API dependency is unreachable.
2. `GET /admin/search-sync/status` — returns detailed CDC state from the background task.
3. The health check must be fast (< 100ms). Use lightweight queries: `SELECT 1` for DB, `GET /health` on Meilisearch for search.

In scope: health endpoint, admin status endpoint, tests. Out of scope: metrics endpoint (Task 02).

</description>


<acceptance_criteria>
- [ ] `/health` returns search sync state and 200 degraded when only search is unhealthy
- [ ] `/health` returns 503 when PostgreSQL or another required core dependency is unhealthy
- [ ] `GET /admin/search-sync/status` returns CDC daemon state
- [ ] Health check completes in < 100ms
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
