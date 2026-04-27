# Docker Compose Local Development

Run the local stack from the repository root:

```bash
docker compose up --build
```

The stack mounts `config/opentk.compose.toml` into both application containers at
`/etc/opentk/config.toml`. Runtime settings stay in that TOML file instead of
being split across per-setting environment variables.

## Services

- `postgres`: runs `postgres:16`, persists data in `opentk-postgres-data`,
  and exposes `localhost:5432`. Application binaries own schema lifecycle:
  `opentk-sync` creates or updates schema at startup, while `opentk-api`
  validates compatibility without mutating the database.
- `meilisearch`: runs the official `getmeili/meilisearch:latest` image in
  development mode, persists data in `opentk-meilisearch-data`, and exposes
  `localhost:7700`.
- `sync`: runs `opentk-sync poll` as the continuous upstream SyncFeed-to-PostgreSQL
  catch-up worker. This worker fetches Tweede Kamer
  SyncFeed data and writes durable rows into PostgreSQL.
- `api`: runs the HTTP API on `localhost:3000`. It waits for healthy PostgreSQL
  but does not hard-block startup on Meilisearch.

Search freshness is a separate PostgreSQL-to-Meilisearch CDC path owned by the
API process. Do not treat the `sync` service as search polling; it only moves
upstream SyncFeed data into PostgreSQL.

## Health

Check API dependency state with:

```bash
curl http://localhost:3000/health
```

When PostgreSQL and search CDC are healthy, the endpoint reports healthy JSON.
When Meilisearch is unavailable, the API may still start and reports degraded
dependency state rather than making Meilisearch a Compose startup blocker.
