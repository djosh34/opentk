# Refactor Binaries to Unified Config Plan

## Goal

Finish the binary-side config unification that Task 1 partially prepared. All runtime binaries should load one typed `opentk-config` TOML file, build runtime dependencies from that config, log the effective config with secrets redacted, and remove old hardcoded/default/bootstrap behavior from binary crates.

## Current State

`opentk-config` already exists and provides typed defaults for database, sync, search, API, and logging. The binaries already accept `--config` and reject many old per-setting CLI flags. The remaining task-specific gaps are:

- `complete-sync` still exists as the binary name and clap command name instead of `opentk-sync`.
- The binaries do not initialize tracing consistently with `tracing_subscriber::fmt::init()`.
- The binaries do not log the loaded effective config.
- `opentk-api` still has a library-local `DEFAULT_SEARCH_URL`/`DEFAULT_SEARCH_INDEX` default search client for `router(pool)`.
- `opentk-api::build_app` always builds a live Meilisearch client and does not verify startup reachability or explicitly mark search unavailable while keeping non-search endpoints available.
- README still documents old `complete-sync` commands and `DATABASE_URL` examples.

## Boundary Design

Use `improve-code-boundaries` to remove the bootstrap/config boundary problems:

- Keep `opentk-config` as the only place for compiled application defaults.
- Add a small redacted effective-config representation in `opentk-config`, so all binaries log the same sanitized shape and no binary hand-rolls secret masking.
- Keep binary entrypoints responsible only for CLI mode selection, loading config, logging the effective config, and mapping typed config into runtime crate boundary types.
- Remove the public `opentk_api::router(pool)` convenience that smuggles search defaults into the API crate. Tests and callers should use `router_with_search` for explicit search behavior or `build_app`/`serve` for config-driven startup.
- Model unavailable search as an explicit `SearchQueryClient` implementation in the API boundary, not as `Option` checks spread across handlers.

## Public Interface Shape

`opentk-config`:

```rust
impl Config {
    pub fn redacted(&self) -> RedactedConfig;
}

#[derive(Debug, Serialize)]
pub struct RedactedConfig { ... }
```

The redacted search config should render `api_key: "***"` when a key is configured and `None` when absent. It should preserve non-secret values such as database max connections, sync URLs/timeouts/categories, search URL/index/batch/retry, API bind address, and log format/level. It should not expose `database.url` because it may contain credentials.

`opentk-api`:

```rust
pub async fn build_app(config: ApiConfig) -> Result<ApiServer, ApiError>;
pub fn router_with_search(pool: PgPool, search_client: Arc<dyn SearchQueryClient + Send + Sync>) -> Router;
```

Remove `router(pool)` unless a test still needs it after migrating to `router_with_search`.

Startup search behavior:

- Build a configured `MeilisearchClient`.
- Probe search at startup through the public search client behavior with a minimal query.
- If the probe succeeds, use the configured client.
- If the probe fails, log a clear error and install an unavailable search client that returns `SearchIndexError` for search requests.
- Non-search routes keep serving because the router still builds after a search probe failure.

Binary names:

- Rename `crates/opentk-db/src/bin/complete-sync.rs` to `opentk-sync.rs`.
- In `crates/opentk-db/Cargo.toml`, expose the binary as `opentk-sync`.
- Update clap command names, tests, README, and generated-source comments that mention cargo invocation when relevant.

## TDD Execution Plan

Use vertical red/green tracer bullets. Do not write all tests first.

- [x] RED 1: Add a config behavior test proving `Config::redacted()` hides `database.url` and masks `search.api_key` as `***` while keeping non-secret values visible. Confirm it fails because the redacted view does not exist.
- [x] GREEN 1: Implement `Config::redacted()` plus serializable redacted structs in `opentk-config`.
- [x] RED 2: Add narrow CLI/source contract tests proving the sync binary is now named `opentk-sync`, old `complete-sync` invocation is gone, and no production binary source contains the old command name except historical task docs. Confirm failure while the file/command are still `complete-sync`.
- [x] GREEN 2: Rename the binary file and Cargo target to `opentk-sync`; update clap tests, README examples, and any production references.
- [x] RED 3: Add binary tests for each application binary startup helper proving it logs or exposes the same redacted config view and uses consistent logging initialization without env-filter application behavior. Confirm failure because logging/config rendering is not shared.
- [x] GREEN 3: Add shared startup helpers where appropriate or direct calls to `tracing_subscriber::fmt::init()` plus `tracing::info!(config = ?config.redacted(), ...)` in all three binaries. Keep log level parsing out of per-setting env behavior.
- [x] RED 4: Add an API startup behavior test with a reachable database but unreachable search endpoint. Build the app and assert `/health` or `/categories` still succeeds while `/search` returns `503 search_unavailable`. Confirm failure because search startup is not probed/marked unavailable explicitly.
- [x] GREEN 4: Add a search availability probe during `build_app`, clear error logging on probe failure, and an unavailable search client implementation behind the existing search trait.
- [x] RED 5: Add a boundary/source test proving `opentk-api` no longer owns search defaults (`DEFAULT_SEARCH_URL`, `DEFAULT_SEARCH_INDEX`, or a default-search router path) and no production code contains `127.0.0.1:7700`, `OPENTK_MEILISEARCH_*`, `OPENTK_SEARCH_*`, `DATABASE_URL`, `dotenvy`, or `std::env::var` application setting reads. Confirm failure while API defaults and README/source remnants remain.
- [x] GREEN 5: Delete `opentk_api::router(pool)` and local API search defaults; update tests to call `router_with_search` or config-driven `build_app`; remove old README/env docs while keeping test harness environment variables only in tests/scripts.
- [x] REFACTOR with `improve-code-boundaries`: remove useless conversion helpers or duplicate runtime config structs where the target crate can accept the typed config cleanly. Prefer one helper per external crate boundary over repeated ad hoc field copying in each command branch.
- [x] FINAL VERIFY: run `cargo fmt`, `make check`, `make lint`, and `make test`. Do not run `make test-long` for this non-story-ending task.

## Completion Steps

After verification passes:

- Check every acceptance criterion in `task-2-refactor-binaries-to-config.md`.
- Set `<passes>true</passes>`.
- Run `/bin/bash .ralph/task_switch.sh`.
- Commit all files, including `.ralph`, with `task finished task-2-refactor-binaries-to-config: ...` and include test evidence in the commit message.
- Push.
- Quit immediately.

NOW EXECUTE
