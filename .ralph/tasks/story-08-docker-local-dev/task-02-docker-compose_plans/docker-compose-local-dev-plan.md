# Story 08 Task 02 Plan: Docker Compose Local Development Stack

## Context Read

- Current task: `.ralph/tasks/story-08-docker-local-dev/task-02-docker-compose.md`.
- Existing image boundaries:
  - `docker/Dockerfile.api` builds scratch image with `/bin/opentk-api`.
  - `docker/Dockerfile.sync` builds scratch image with `/bin/opentk-sync`.
  - Both binaries load TOML config from `--config <path>` or `/etc/opentk/config.toml`.
- Existing config boundary:
  - `opentk-config` is already the public config interface.
  - Config must remain TOML-file based; do not add individual runtime setting env vars.
  - Compose should mount one config file to `/etc/opentk/config.toml`.
- Existing health boundary:
  - `opentk-api` already starts if Meilisearch is unavailable by installing `UnavailableSearchClient` and recording CDC status error.
  - Current `/health` returns `503 search_sync_degraded` when the search CDC status is degraded.
  - Story 08 Task 04 explicitly owns changing `/health` to return `200 {"status":"degraded"}` for Meilisearch-down and adding container `HEALTHCHECK`/shutdown work. Do not pull the full Task 04 health redesign into this compose task unless Task 02 execution proves compose acceptance is impossible without it.

## Public Interfaces To Add

- Add `docker-compose.yml` at repo root as the local stack entrypoint:
  - network: `opentk`
  - volumes: `opentk-postgres-data`, `opentk-meilisearch-data`
  - service hostnames: `postgres`, `meilisearch`, `api`, `sync`
- Add `config/opentk.compose.toml` as the mounted application config:
  - `[database].url = "postgres://opentk:opentk@postgres:5432/opentk"`
  - `[search].url = "http://meilisearch:7700"`
  - `[api].bind_address = "0.0.0.0:3000"`
  - sync settings stay in TOML, including `poll_interval_secs` and categories.
- Add `scripts/init-db.sh` as the PostgreSQL first-start init script:
  - runs inside the official `postgres:16` container from `/docker-entrypoint-initdb.d/`.
  - installs or uses `sqlx-cli` in a bounded, explicit way, then runs `sqlx migrate run` against the compose database.
  - fails loudly on missing tools or migration errors (`set -euo pipefail`), never swallowing migration failures.
- Add concise docs, preferably `docs/docker-compose.md`, and link from `README.md`:
  - document `docker compose up --build`.
  - clearly distinguish upstream SyncFeed polling (`sync` service running `opentk-sync poll`) from PostgreSQL-to-Meilisearch CDC (API background listener).
  - document `/health` expected current behavior and the Task 04 degraded-health follow-up if not changed in this task.

## TDD / Verification Plan

Use vertical slices. Do not write a batch of tests before implementation.

1. RED tracer: run `docker compose config`.
   - Expected first failure before implementation: no compose file or missing referenced files.
   - GREEN: add the smallest valid `docker-compose.yml` with `postgres`, network, volume, and the config mount structure needed for later services.
   - Verify `docker compose config` passes.

2. RED: add a focused repository test for the compose contract through a public file interface.
   - Add an integration-style test under a real crate test directory, not under `.ralph/`.
   - The test should parse `docker-compose.yml` as YAML and assert observable local-stack contract:
     - `postgres` uses `postgres:16`, exposes `5432:5432`, has a health check, has persistent data, and mounts `scripts/init-db.sh` into `/docker-entrypoint-initdb.d/`.
     - `api` and `sync` build from the existing Dockerfiles, mount the same TOML config path, and depend on healthy `postgres`.
     - `api` does not depend on healthy `meilisearch`.
     - `sync` command is `opentk-sync --config /etc/opentk/config.toml poll`.
     - `meilisearch` has persistent data, exposes `7700:7700`, and has health check/config required by the official image.
   - GREEN: complete compose fields until this one behavior test passes.

3. RED: add a focused config-file behavior test through `opentk_config::ConfigLoader::with_path`.
   - It should load `config/opentk.compose.toml`.
   - It should verify service hostnames and ports reduce to the runtime config (`postgres`, `meilisearch`, `0.0.0.0:3000`) without env expansion.
   - GREEN: add/fix `config/opentk.compose.toml`.

4. RED: add a focused init-script contract test only if there is an existing suitable shell/script contract test pattern. If no such pattern exists, prefer direct execution verification over brittle script unit tests.
   - Public behavior to verify: `bash -n scripts/init-db.sh` passes and the compose service mounts it as a Postgres init script.
   - GREEN: add `scripts/init-db.sh` with strict shell mode and an explicit `sqlx migrate run` call.

5. Manual compose verification, only after the file-level contract is green:
   - `docker compose config`
   - `docker compose up --build`
   - `curl http://localhost:3000/health`
   - Stop Meilisearch or start without it only if needed to verify the non-hard dependency acceptance.
   - Do not run `make test-long`.

6. Required end checks:
   - `make check`
   - `make test`
   - `make lint`

## Code Boundary Checks

Apply `$improve-code-boundaries` during execution:

- Keep compose orchestration in `docker-compose.yml`; do not add ad hoc Makefile wrappers unless the task demands them.
- Keep runtime settings reduced through `opentk-config`; do not create duplicate env-var config paths in API or sync.
- Avoid wrong-place validation: no extra Rust validation outside config/startup validation just for compose.
- Avoid string soup for documentation-heavy behavior; config values should live in TOML, not in multiple script-rendered variants.
- If tests need to read compose YAML, keep parsing in the test boundary. Do not add production Rust types solely to model Docker Compose.
- If the health acceptance cannot be met with current API behavior, switch this plan back for verification instead of quietly expanding into all Task 04 work.

## Acceptance Mapping

- `docker compose up` starts local stack:
  - Compose services, network, volumes, mounted config, and commands cover this.
- Upstream SyncFeed polling vs CDC distinction:
  - `sync` command and docs explicitly say `opentk-sync poll` is upstream SyncFeed-to-PostgreSQL, while API-owned CDC mirrors PostgreSQL changes into Meilisearch.
- Migrations run automatically:
  - `scripts/init-db.sh` mounted into `postgres` init directory and calls `sqlx migrate run`.
- `/health` returns dependency-state JSON:
  - Verify current JSON. If Task 02 acceptance requires `200 degraded` now, pause and move plan to verification because this overlaps Task 04.
- `depends_on`:
  - `api` and `sync` depend on healthy `postgres`; `api` does not hard-block on `meilisearch`.
- TOML settings:
  - Mounted config file is the single application settings interface.

NOW EXECUTE
