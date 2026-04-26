# Centralized Config Crate Plan

## Goal

Create `opentk-config` as the only application-settings loader for binaries. The public boundary should be a small, typed file loader that reads TOML from an explicit path or well-known default paths, applies compiled defaults only for optional fields, validates required dependencies up front, and maps cleanly into the existing runtime config structs.

## Boundary Design

Use `improve-code-boundaries` to remove the current bootstrap/config boundary problem:

- Move all file discovery, TOML parsing, defaults, and validation into `crates/opentk-config`.
- Keep binary crates responsible only for parsing command/subcommand intent plus `--config <path>` and calling the new loader.
- Remove duplicate database-url fallback helpers and all per-setting `clap(env = ...)` declarations from application binaries.
- Remove `opentk-api`'s `default_search_client()` environment reads. Server startup should build the real search client from typed config; tests can continue using `router_with_search`.
- Do not put database connection, HTTP client construction, or search indexing behavior into `opentk-config`; it only owns configuration loading/validation and typed conversion helpers.

Public crate shape:

```rust
pub struct Config {
    pub database: DatabaseConfig,
    pub sync: SyncConfig,
    pub search: SearchConfig,
    pub api: ApiConfig,
    pub log: LogConfig,
}

pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

pub struct SyncConfig {
    pub base_url: reqwest::Url,
    pub request_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub max_retries: u32,
    pub max_concurrent_requests: NonZeroUsize,
    pub poll_interval_secs: u64,
    pub categories: Vec<String>,
}

pub struct SearchConfig {
    pub url: String,
    pub api_key: Option<String>,
    pub index_name: String,
    pub batch_size: i64,
    pub retry_limit: i32,
}

pub struct ApiConfig {
    pub bind_address: SocketAddr,
    pub cors_origins: Vec<String>,
}

pub struct LogConfig {
    pub format: LogFormat,
    pub level: String,
}

pub enum LogFormat {
    Json,
    Pretty,
}
```

Loader API:

```rust
pub struct ConfigLoader {
    explicit_path: Option<PathBuf>,
}

impl ConfigLoader {
    pub fn new() -> Self;
    pub fn with_path(path: impl Into<PathBuf>) -> Self;
    pub fn load(self) -> Result<LoadedConfig, ConfigError>;
}

pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: Config,
}
```

Also expose `Config::from_toml_str(source: &str) -> Result<Self, ConfigError>` or a similarly named parsing helper for tests and validation tooling.

## TOML Contract

Example full config:

```toml
[database]
url = "postgres://postgres:postgres@postgres:5432/opentk"
max_connections = 5

[sync]
base_url = "https://gegevensmagazijn.tweedekamer.nl"
request_timeout_secs = 30
connect_timeout_secs = 10
max_retries = 3
max_concurrent_requests = 4
poll_interval_secs = 30
categories = []

[search]
url = "http://meilisearch:7700"
api_key = "development-master-key"
index_name = "opentk_entities"
batch_size = 100
retry_limit = 3

[api]
bind_address = "0.0.0.0:3000"
cors_origins = []

[log]
format = "pretty"
level = "info"
```

Required fields:

- `database.url`

Optional fields filled by compiled defaults:

- `database.max_connections = 5`
- `sync.base_url = "https://gegevensmagazijn.tweedekamer.nl"`
- `sync.request_timeout_secs = 30`
- `sync.connect_timeout_secs = 10`
- `sync.max_retries = 3`
- `sync.max_concurrent_requests = 4`
- `sync.poll_interval_secs = 30`
- `sync.categories = official_schema categories`
- `search.url = "http://meilisearch:7700"` for Docker-friendly local networking
- `search.api_key = None`
- `search.index_name = "opentk_entities"`
- `search.batch_size = 100`
- `search.retry_limit = 3`
- `api.bind_address = "0.0.0.0:3000"`
- `api.cors_origins = []`
- `log.format = "pretty"`
- `log.level = "info"`

Validation rules:

- Explicit `--config` path must exist and be readable; missing explicit files are errors.
- Without `--config`, load `./opentk.toml` if present, otherwise `/etc/opentk/config.toml` if present, otherwise error because `database.url` has no compiled default.
- Invalid TOML, unknown enum values, invalid URLs/socket addresses, zero max connections, zero batch size, zero retry limit, and zero concurrency are explicit `ConfigError` variants with path/field context.
- Unknown TOML fields should fail. This is greenfield and typo-tolerant config silently creates bad operations.
- Do not read `OPENTK_*`, `DATABASE_URL`, `.env`, or any other single-setting environment variables in `opentk-config` or application binaries.

## Dependency Plan

Workspace root:

- Add `crates/opentk-config` to `members`.
- Add workspace dependencies:
  - `opentk-config = { path = "crates/opentk-config" }`
  - `toml`

`opentk-config` dependencies:

- `opentk-core` for official default categories.
- `reqwest` for `Url` type.
- `serde` for TOML DTOs.
- `thiserror` for explicit errors.
- No `clap`, no `dotenvy`, no database or search client dependencies.

Binary crate dependencies:

- Add `opentk-config` to `opentk-api` and `opentk-db`.
- Keep `clap` only for command/subcommand parsing and `--config`.
- Remove `clap`'s `env` feature from workspace dependencies if no remaining crate needs it.

## TDD Execution Plan

Use vertical red/green tracer bullets. Do not write the whole test suite first.

- [x] RED 1: Add `opentk-config` crate with one public-interface test: a TOML string containing only required `database.url` loads and fills compiled defaults for database max connections, sync base URL/timeouts/categories, search URL/index/batch/retry, API bind address, and log settings. Confirm failure because crate/API does not exist.
- [x] GREEN 1: Add crate, workspace member/dependency, TOML DTOs with serde defaults, typed `Config`, and enough parsing/defaulting to pass the first behavior.
- [x] RED 2: Add a loader test using temp directories/files: explicit path wins over default locations and missing explicit path returns a clear path-specific error. Confirm failure before file discovery exists.
- [x] GREEN 2: Implement `ConfigLoader`, discovery order `--config`, `./opentk.toml`, `/etc/opentk/config.toml`, and `LoadedConfig { path, config }`.
- [x] RED 3: Add validation tests through the public parser for missing `database.url`, zero numeric limits, invalid URL/socket address, invalid `log.format`, and unknown TOML fields. Confirm failure before validation and deny-unknown-fields are complete.
- [x] GREEN 3: Add explicit `ConfigError` variants/messages and validation so bad config fails fast with field context.
- [x] RED 4: Add CLI parse tests for `complete-sync`, `search-sync`, and `opentk-api` proving only `--config` remains for application settings and old flags like `--database-url`, `--base-url`, `--meilisearch-url`, and `--bind-address` are rejected. Confirm failure against current CLIs.
- [x] GREEN 4: Refactor binary `Args`/subcommands to carry only `--config` plus operational selectors such as command name, `--required-relation-samples`, and possibly command mode. Load config once at startup and map `opentk_config::Config` into existing runtime structs.
- [x] RED 5: Add startup/build tests proving API/search configuration comes from typed config, not environment variables. For API, expose or adjust a public `build_app`/`serve` config that accepts database config and search config explicitly, then assert a supplied fake/router path is still testable.
- [x] GREEN 5: Replace `opentk_api::ApiConfig { database_url }` with a typed startup config containing `opentk_db::DatabaseConfig` and search settings/client construction. Remove `default_search_client()` environment reads.
- [x] RED 6: Add a source-level or behavior test that application binaries/config crate contain no `OPENTK_`, `DATABASE_URL`, `dotenvy`, or `env::var` application-setting reads outside test-only database harness code. Confirm failure while legacy env strings remain in production binaries.
- [x] GREEN 6: Delete the old env fallback helpers, old CLI override tests, and any `clap(env = ...)` usage in production code. Keep test harness env vars only where `scripts/cargo-test-with-postgres.sh` needs them to supply the test database.
- [x] RED 7: Add a search-sync behavior test or narrow unit test proving config categories default to official categories and user-provided categories flow into `SearchSyncConfig`, with batch/retry validation preserved.
- [x] GREEN 7: Wire `SearchConfig`/`SyncConfig` into `search-sync` and `complete-sync`, preserving existing runtime validation instead of duplicating it in binaries.
- [x] REFACTOR with `improve-code-boundaries`: remove duplicated config structs or conversion helpers that merely rename fields. Preferred shape is one deep loader module in `opentk-config` plus small `From`/builder functions at runtime boundaries only when target types live in other crates.

## Binary Mapping Notes

`complete-sync`:

- Global CLI: `complete-sync --config path run|poll|status|verify`.
- `run` and `poll` use `config.database`, `config.sync.base_url`, `config.sync.categories`, `config.sync.poll_interval_secs`, and SyncFeed timeout/retry/concurrency settings.
- `status` and `verify` use `config.database` and `config.sync.categories`; keep `verify --required-relation-samples` as operational input.

`search-sync`:

- Global CLI: `search-sync --config path full-reindex|incremental|failures|retry-failures`.
- Sync commands use `config.database` and `config.search`.
- Failure listing uses `config.database`.

`opentk-api`:

- CLI: `opentk-api --config path`.
- Startup uses `config.database`, `config.api`, `config.search`, and `config.log`.
- Logging should initialize from `log.format`/`log.level` without falling back to env filters for application behavior. If tracing subscriber cannot parse the level, return an explicit startup error.

## Verification

During execution, run the narrow test after each red/green cycle. Before completion, run in order:

- `cargo fmt`
- `make check`
- `make lint`
- `make test`

Do not run `make test-long`; this task is not story-ending and does not explicitly require e2e/long validation.

After all checks pass, update acceptance boxes and set `<passes>true</passes>` in `task-1-create-config-crate.md`, run `/bin/bash .ralph/task_switch.sh`, commit all files with `task finished task-1-create-config-crate: ...`, push, and quit immediately.

NOW EXECUTE
