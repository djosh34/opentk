## Task: Story 8 Task 2 - Docker Compose Local Development Stack <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Provide `docker-compose.yml` so the entire stack runs locally with `docker compose up`.

Services:
- `postgres`: `postgres:16`, health check, persistent volume, port 5432, runs migrations on first startup via `scripts/init-db.sh`
- `meilisearch`: `getmeili/meilisearch:latest`, health check, persistent volume, port 7700, master key from env
- `sync`: built from `Dockerfile.sync`, depends on postgres healthy, runs `opentk-sync poll`, restart always
- `api`: built from `Dockerfile.api`, depends on postgres and meilisearch healthy, port 3000, restart always
- Shared network `opentk`, consistent service hostnames for config defaults

Files:
- `docker-compose.yml`
- `.env.example` documenting all env vars
- `scripts/init-db.sh` running `sqlx migrate run`
- Optional `docker-compose.override.yml` for local dev tweaks

In scope: compose file, env example, init script. Out of scope: production configs, Kubernetes.

</description>


<acceptance_criteria>
- [ ] `docker compose up` starts postgres, meilisearch, api, sync successfully
- [ ] Migrations run automatically on first postgres startup
- [ ] `curl http://localhost:3000/health` returns OK
- [ ] All services have `depends_on` with `condition: service_healthy`
- [ ] `.env.example` documents every env var
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
