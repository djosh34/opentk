## Task: Story 08 Task 02 - Docker Compose Local Development Stack <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Provide `docker-compose.yml` so the entire stack runs locally with `docker compose up`.

Services:
- `postgres`: `postgres:16`, health check, persistent volume, port 5432, runs migrations on first startup via `scripts/init-db.sh`
- `meilisearch`: `getmeili/meilisearch:latest`, health check, persistent volume, port 7700, configured according to the official image requirements
- `sync`: built from `Dockerfile.sync`, depends on postgres healthy, runs `opentk-sync poll` as the intentional continuous upstream SyncFeed-to-PostgreSQL catch-up worker. This is not search polling; search freshness is handled separately by CDC from PostgreSQL into Meilisearch.
- `api`: built from `Dockerfile.api`, depends on postgres healthy, port 3000, restart always. It may start before or without healthy Meilisearch and must expose search as degraded/unavailable until Meilisearch works.
- Shared network `opentk`, consistent service hostnames for config defaults

Files:
- `docker-compose.yml`
- `config/opentk.compose.toml` or equivalent mounted TOML config file for application settings
- `scripts/init-db.sh` running `sqlx migrate run`
- Optional `docker-compose.override.yml` for local dev tweaks

In scope: compose file, env example, init script. Out of scope: production configs, Kubernetes.

</description>


<acceptance_criteria>
- [ ] `docker compose up` starts postgres, api, and the continuous upstream sync worker successfully; search is available when Meilisearch is healthy and degraded when it is not
- [ ] Compose documentation clearly distinguishes upstream SyncFeed polling from PostgreSQL-to-Meilisearch CDC
- [ ] Migrations run automatically on first postgres startup
- [ ] `curl http://localhost:3000/health` returns healthy or degraded JSON according to dependency state
- [ ] Required services have `depends_on` with `condition: service_healthy`; Meilisearch must not be a hard startup blocker for `api`
- [ ] Application settings are provided by mounted TOML config files, not individual environment variables
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
