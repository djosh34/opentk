## Task: Story 8 Task 4 - Docker Health Checks and Graceful Shutdown <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add health checks and graceful shutdown so Docker Compose can orchestrate startup order and stop/restart cleanly.

Requirements:
1. `opentk-api` `/health` already checks PostgreSQL; extend it to also check Meilisearch (lightweight stats call). Return 503 if either is down. Response includes search_sync state.
2. Add `HEALTHCHECK` to all Dockerfiles:
   - `opentk-api`: `curl -f http://localhost:3000/health || exit 1`
   - `opentk-sync`: since it's not an HTTP server, add a `--health-check` CLI flag that exits 0 if the process is healthy (checks DB connection)
3. Graceful shutdown:
   - `opentk-api`: `tokio::select!` on `ctrl_c`, finish in-flight requests (Axum supports graceful shutdown)
   - `opentk-sync` poll mode: finish current page on SIGTERM, then exit
4. Document in `docs/operations.md`

In scope: health endpoint, Dockerfile HEALTHCHECK, graceful shutdown, docs. Out of scope: K8s probes.

</description>


<acceptance_criteria>
- [ ] `/health` returns 503 when Meilisearch is down
- [ ] All Dockerfiles contain HEALTHCHECK instructions
- [ ] `docker compose ps` shows all services as healthy
- [ ] `opentk-api` shuts down gracefully within 10s of `docker stop`
- [ ] `opentk-sync` exits cleanly on SIGTERM after current page
- [ ] `docs/operations.md` documents health and shutdown
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
