## Task: Story 6 Task 1 - Create Centralized Configuration Crate <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Create a centralized configuration system that unifies how all binaries receive their settings. Currently each binary has its own ad-hoc clap argument definitions with inconsistent env var names and fallback chains.

The current mess:
- `complete-sync` uses `--database-url` / `OPENTK_DATABASE_URL` / `DATABASE_URL`, plus hardcoded `https://gegevensmagazijn.tweedekamer.nl`, timeouts, retries, `max_connections(5)`
- `opentk-api` uses `--database-url` / `OPENTK_DATABASE_URL` / `DATABASE_URL`, plus `--bind-address` / `OPENTK_API_BIND_ADDRESS`, plus `OPENTK_SEARCH_URL`, `OPENTK_SEARCH_API_KEY`, `OPENTK_SEARCH_INDEX` read directly from `std::env::var`
- `search-sync` uses `--database-url` / `OPENTK_DATABASE_URL`, plus `OPENTK_MEILISEARCH_URL` / `OPENTK_MEILISEARCH_API_KEY` (note the INCONSISTENT naming with opentk-api!)

The new system must be a new crate `opentk-config` with typed structs:

```rust
pub struct Config {
    pub database: DatabaseConfig,
    pub sync: SyncConfig,
    pub search: SearchConfig,
    pub api: ApiConfig,
    pub log: LogConfig,
}
```

With sections:
- `database.url`, `database.max_connections` — env: `OPENTK_DATABASE_URL`, `OPENTK_DATABASE_MAX_CONNECTIONS`
- `sync.base_url`, `sync.request_timeout_secs`, `sync.connect_timeout_secs`, `sync.max_retries`, `sync.max_concurrent_requests`, `sync.poll_interval_secs`, `sync.categories`
- `search.url`, `search.api_key`, `search.index_name`, `search.batch_size`, `search.retry_limit`, `search.poll_interval_secs`
- `api.bind_address`, `api.cors_origins`
- `log.format` (`json` | `pretty`), `log.level`

Loading precedence (highest to lowest):
1. CLI args via clap derive
2. Environment variables
3. TOML config file (`./opentk.toml` or `/etc/opentk/config.toml`)
4. Compiled defaults (Docker-friendly: `postgres://postgres@postgres:5432/opentk`, `http://meilisearch:7700`, `0.0.0.0:3000`)

Must validate at startup and fail fast with clear messages. No `dotenvy` dependency needed — Docker and systemd handle env injection.

In scope: `opentk-config` crate, TOML parsing, env var loading, CLI integration, validation. Out of scope: live reload, remote config.

</description>


<acceptance_criteria>
- [ ] `opentk-config` crate exists with typed `Config` struct
- [ ] Config loads from CLI args, env vars, and TOML with correct precedence
- [ ] Docker-friendly defaults for all network endpoints
- [ ] Invalid config fails fast with clear error on startup
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
