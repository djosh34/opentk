## Task: Story 6 Task 2 - Refactor All Binaries to Use Unified Config <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Refactor all existing binaries to use `opentk-config` and eliminate all hardcoded defaults and single-setting environment-variable parsing that make deployment behavior inconsistent.

Refactoring targets:
1. `complete-sync` → `opentk-sync` (rename the binary target in Cargo.toml, keep crate as `opentk-db` for now):
   - Replace `DEFAULT_SYNCFEED_BASE_URL` with config default
   - Replace hardcoded `Duration::from_secs(30)` timeout with config
   - Replace `max_connections(5)` with config value
   - Remove `database_url()` helper; use `config.database.url`
2. `opentk-api`:
   - Replace `default_search_client()` hardcoded `127.0.0.1:7700` with config default `http://meilisearch:7700`
   - Replace `DEFAULT_MAX_DATABASE_CONNECTIONS` constant with config
   - Remove all environment-variable parsing from `main()`; use only unified TOML config
   - If Meilisearch cannot be reached at startup, log a clear error, mark search as unavailable, and continue serving every non-search API capability normally
3. `search-sync`:
   - Remove all `OPENTK_MEILISEARCH_*`, `OPENTK_SEARCH_*`, and database environment-variable parsing instead of renaming it
   - Use config defaults for batch_size and retry_limit
4. All binaries:
   - Use `tracing_subscriber::fmt::init()` consistently
   - Log effective config at startup (secrets like api_key redacted to `***`)

Break backward compatibility intentionally. Remove all old environment-variable names completely; do not add replacement environment variables.

In scope: refactoring all binaries, updating tests, updating README, removing old env vars. Out of scope: new features except optional search capability during API startup.

</description>


<acceptance_criteria>
- [ ] `complete-sync` (to be renamed) uses unified config for all settings
- [ ] `opentk-api` uses unified config for all settings
- [ ] `search-sync` uses unified config for all settings
- [ ] `opentk-api` starts and serves non-search endpoints when Meilisearch is unavailable
- [ ] No hardcoded `127.0.0.1` defaults remain
- [ ] Old env var names (`OPENTK_MEILISEARCH_URL`) and all replacement single-setting env vars are completely removed
- [ ] All binaries log effective config at startup with secrets redacted
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
