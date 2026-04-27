## Task: Story 06 Task 01 - Create Centralized Configuration Crate <status>done</status> <passes>true</passes>

<plan>
.ralph/tasks/story-06-config-unification/task-01-create-config-crate_plans/centralized-config-crate-plan.md
</plan>

NOW EXECUTE

<description>
Must use tdd skill to complete

**Goal:** Create a centralized configuration-file system that unifies how all binaries receive their settings. Currently each binary has its own ad-hoc clap argument definitions with inconsistent env var names and fallback chains.

The current mess:
- `complete-sync` uses `--database-url` / `OPENTK_DATABASE_URL` / `DATABASE_URL`, plus hardcoded `https://gegevensmagazijn.tweedekamer.nl`, timeouts, retries, `max_connections(5)`
- `opentk-api` uses `--database-url` / `OPENTK_DATABASE_URL` / `DATABASE_URL`, plus `--bind-address` / `OPENTK_API_BIND_ADDRESS`, plus `OPENTK_SEARCH_URL`, `OPENTK_SEARCH_API_KEY`, `OPENTK_SEARCH_INDEX` read directly from `std::env::var`
- `search-sync` uses `--database-url` / `OPENTK_DATABASE_URL`, plus `OPENTK_MEILISEARCH_URL` / `OPENTK_MEILISEARCH_API_KEY` (note the INCONSISTENT naming with opentk-api!)

This story must delete that entire model. Application config is loaded only from a TOML config file. Do not support individual environment variables for application settings. Do not support CLI overrides for individual settings. The only CLI config option allowed is selecting the config file path, for example `--config /etc/opentk/config.toml`; operational flags such as `--validate-config` may remain separate.

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
- `database.url`, `database.max_connections`
- `sync.base_url`, `sync.request_timeout_secs`, `sync.connect_timeout_secs`, `sync.max_retries`, `sync.max_concurrent_requests`, `sync.poll_interval_secs`, `sync.categories`
- `search.url`, `search.api_key`, `search.index_name`, `search.batch_size`, `search.retry_limit`
- `api.bind_address`, `api.cors_origins`
- `log.format` (`json` | `pretty`), `log.level`

Loading:
1. If `--config <path>` is supplied, load that TOML file.
2. Otherwise load `./opentk.toml` if present.
3. Otherwise load `/etc/opentk/config.toml` if present.
4. Fill only omitted optional fields from compiled defaults.

There must be no `OPENTK_*`, `DATABASE_URL`, `.env`, or other single-setting environment-variable support in `opentk-config` or the binaries. Docker and systemd should mount or write config files, not inject application settings through environment variables.

Must validate at startup and fail fast with clear messages for required dependencies. No `dotenvy` dependency is allowed.

In scope: `opentk-config` crate, TOML parsing, `--config` path integration, validation. Out of scope: live reload, remote config, individual environment-variable config, per-setting CLI overrides.

</description>


<acceptance_criteria>
- [x] `opentk-config` crate exists with typed `Config` struct
- [x] Config loads from TOML config files only, with optional `--config <path>` selecting the file
- [x] No application setting can be configured via individual environment variables
- [x] No application setting can be overridden via individual CLI args
- [x] Docker-friendly defaults for all network endpoints
- [x] Invalid config fails fast with clear error on startup
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite)
- [x] `make lint` — passes cleanly
</acceptance_criteria>
