# Story 11 Task 01 Plan: Enhanced Health and Admin Search Sync Status

## Context Read

- Current task: `.ralph/tasks/story-11-search-observability/task-01-enhance-health-endpoint.md`.
- Required skills read:
  - `$tdd`: execute as vertical red-green slices through public behavior, one test then one implementation step.
  - `$improve-code-boundaries`: watch for duplicate DTO shapes, wrong-place logic, stringly errors, over-public fields, and helper-only boundaries.
- Existing API code in `crates/opentk-api/src/lib.rs` already has:
  - `GET /health`
  - `GET /admin/search-sync/status`
  - `SearchCdcRuntimeStatus` in API state
  - a search health trait call via `state.search.health().await`
  - `SELECT 1` for PostgreSQL health
- Existing health response currently uses `postgres` and `meilisearch` fields, but the Story 11 task contract requires `database` and `search`.
- Existing health response reports search unavailability as `"meilisearch": "unavailable"` but does not expose a clear search error field in the `/health` JSON.
- Existing docs mention older `meilisearch` field naming. Because this is greenfield with no backwards compatibility, the Story 11 public contract should win.

## Public Interface

Make `/health` return this stable shape:

```json
{
  "status": "ok",
  "database": "ok",
  "search": "ok",
  "search_error": null,
  "search_sync": {
    "state": "running",
    "last_notification_at": "2026-04-26T12:00:00+00:00",
    "pending_count": 0,
    "last_batch": {
      "indexed": 42,
      "deleted": 3,
      "failed": 0,
      "duration_ms": 1500
    },
    "last_error": null
  }
}
```

Rules:

- `database` is `"ok"` when `SELECT 1` succeeds.
- `search` is `"ok"` when the configured search client health check succeeds.
- `search` is `"unavailable"` when the search health check fails.
- `search_error` is `null` when search is healthy and a clear string when search is unavailable.
- `status` is `"ok"` only when database, search, and search sync are all usable.
- `status` is `"degraded"` with HTTP `200` when database is healthy but search or search sync is unhealthy.
- HTTP `503` is reserved for PostgreSQL or another required core API dependency being unreachable.
- `GET /admin/search-sync/status` keeps returning the daemon snapshot directly:
  - `state`
  - `last_notification_at`
  - `pending_count`
  - `last_batch`
  - `last_error`

## TDD Execution Plan

Use vertical slices. Do not write all tests first.

1. RED: update the existing API health test for a reachable database plus unavailable search to assert the Story 11 field names:
   - HTTP `200`
   - `status == "degraded"`
   - `database == "ok"`
   - `search == "unavailable"`
   - `search_error` is a non-empty string
   - `search_sync.state == "starting"`
   Expected failure: current code returns `postgres` and `meilisearch`, not `database` and `search`, and has no `search_error`.
   GREEN: change `HealthResponse` and `health` mapping only enough to pass this test.

2. RED: add or adjust the healthy-search health test using a fake search health client that returns `Ok(())` and a running CDC status:
   - HTTP `200`
   - `status == "ok"`
   - `database == "ok"`
   - `search == "ok"`
   - `search_error == null`
   - `search_sync.state == "running"`
   GREEN: reuse the existing search health trait and CDC status snapshot; do not add a second search client path.

3. RED: keep the unreachable database behavior pinned:
   - HTTP `503`
   - JSON error body `{ "code": "database_unavailable", "message": "database unavailable" }`
   GREEN: preserve the existing `SELECT 1` core dependency gate.

4. RED: add a health test where search is healthy but CDC status is degraded:
   - HTTP `200`
   - `status == "degraded"`
   - `search == "ok"`
   - `search_error == null`
   - `search_sync.last_error` contains the CDC error
   GREEN: keep `SearchCdcRuntimeState::is_search_usable()` as the domain decision and map it once at the HTTP boundary.

5. RED: keep or strengthen `GET /admin/search-sync/status` test:
   - it returns the daemon snapshot without requiring a database connection
   - it includes batch counts and duration
   - it includes `last_error`
   GREEN: no duplicate DTO mapping beyond the existing `search_sync_daemon_status_response`.

6. RED: add a fast-health assertion without depending on a real slow network:
   - Use a fake search health client that completes immediately and a reachable test database.
   - Assert elapsed wall time is under 100 ms for the handler path.
   - If CI/database variance makes a hard timing assertion flaky, switch plan back to `TO BE VERIFIED` and redesign this as a local unit around the dependency probes.
   GREEN: keep the handler lightweight: `SELECT 1`, search `health()`, and in-memory CDC snapshot only.

7. RED: update OpenAPI assertions if they currently describe old health field names.
   GREEN: regenerate or adjust schema registrations by changing the response structs, not by hand-building raw JSON.

8. RED: update docs that mention `/health` dependency fields:
   - `docs/operations.md`
   - `docs/docker-compose.md` if needed
   GREEN: document the new `database` / `search` / `search_error` contract and degraded semantics.

## Improve Code Boundaries Plan

Apply `$improve-code-boundaries` while executing:

- Prefer one `HealthResponse` DTO at the HTTP boundary; do not build ad hoc `serde_json::json!` health bodies in production.
- Keep search dependency probing behind `SearchHealthClient`; do not mix health behavior into search query handling.
- Reuse `search_sync_daemon_status_response` for both `/health` and `/admin/search-sync/status` so the search sync shape is not duplicated.
- Preserve typed CDC state decisions through `SearchCdcRuntimeState::is_search_usable()` instead of string comparisons.
- Avoid compatibility aliases such as keeping both `postgres` and `database` or both `meilisearch` and `search`. Greenfield means the required contract replaces the older one.
- If a clear search error string requires formatting `SearchIndexError`, do it once at the health boundary and store it as `Option<String>` in the response DTO.
- Final boundary review: search for stale `postgres`, `meilisearch`, duplicate health response structs, ignored search health errors, `let _ =` around fallible work, and raw JSON construction in production health code.

## Acceptance Mapping

- `/health` returns search sync state and `200 degraded` when only search is unhealthy:
  - Covered by degraded health tests with unavailable search and CDC snapshot assertions.
- `/health` returns `503` when PostgreSQL or another required core dependency is unhealthy:
  - Covered by unreachable database health test.
- `GET /admin/search-sync/status` returns CDC daemon state:
  - Covered by existing/strengthened admin status endpoint test.
- Health check completes in `< 100ms`:
  - Covered by a focused fast handler-path test or redesigned before execution if timing proves invalid.
- `make check`, `make test`, `make lint`:
  - Run after implementation. Do not run `make test-long`; this is not a story-finishing task.

## Execution Checklist

- [ ] RED/GREEN: degraded search health uses Story 11 field names and clear `search_error`.
- [ ] RED/GREEN: fully healthy health response returns `status: "ok"`.
- [ ] RED/GREEN: database failure remains `503 database_unavailable`.
- [ ] RED/GREEN: degraded CDC status keeps HTTP `200` and embeds search sync error.
- [ ] RED/GREEN: admin search sync status returns daemon snapshot.
- [ ] RED/GREEN or re-plan: health handler fast-path is verified under 100 ms.
- [ ] RED/GREEN: OpenAPI contract follows the new response shape.
- [ ] RED/GREEN where appropriate: docs match the new health contract.
- [ ] Run `make check`.
- [ ] Run `make test`.
- [ ] Run `make lint`.
- [ ] Final `$improve-code-boundaries` review and cleanup.

NOW EXECUTE
