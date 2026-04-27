# Operations

## Health

`opentk-api` exposes `GET /health` on the API port. The endpoint checks PostgreSQL as the core dependency and returns `503` with `database_unavailable` when PostgreSQL cannot be reached.

When PostgreSQL is reachable, `/health` returns `200` with a dependency snapshot:

```json
{
  "status": "ok",
  "postgres": "ok",
  "meilisearch": "ok",
  "search_sync": {
    "state": "running",
    "last_notification_at": null,
    "pending_count": 0,
    "last_batch": null,
    "last_error": null
  }
}
```

`status` becomes `degraded` while the API can still serve core read routes but search-side dependencies are not fully healthy. That includes Meilisearch being unavailable or `search_sync` reporting an error, stopped state, or pending degraded state. Degraded health remains HTTP `200` so Docker Compose does not restart the API only because optional search functionality is temporarily unavailable.

`opentk-sync --health-check` loads the normal configuration and verifies that PostgreSQL accepts a lightweight query. It exits `0` on success and exits non-zero on configuration or database errors.

## Docker Healthchecks

Docker healthchecks are declared for the local runtime images and in `docker-compose.yml`:

- `opentk-api`: `curl -f http://localhost:3000/health || exit 1`
- `opentk-sync`: `opentk-sync --config /etc/opentk/config.toml --health-check`
- PostgreSQL and Meilisearch use service-native health probes in Compose.

The generic scratch Dockerfile declares `HEALTHCHECK NONE` because it has no shell or curl and is shared by both API and sync binaries. Use the dedicated API and sync Dockerfiles for the local Compose stack.

After starting the stack, inspect health with:

```bash
docker compose ps
curl http://localhost:3000/health
```

## Graceful Shutdown

`opentk-api` handles Ctrl-C and SIGTERM through Axum graceful shutdown. Docker `stop` sends SIGTERM, after which the API stops accepting new work and lets in-flight HTTP requests finish before the process exits.

`opentk-sync poll` handles Ctrl-C and SIGTERM by requesting graceful shutdown from the sync runner. The runner does not abort an active SyncFeed page. It writes the current page through the durable store boundary, commits the cursor, and then exits cleanly before fetching the next page or before the next poll sleep completes.
