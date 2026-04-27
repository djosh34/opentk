# Startup Dependency Validation Plan

## Goal

Add explicit startup dependency validation for all runtime binaries so deploys fail before doing work when required dependencies are unavailable, while `opentk-api` can keep serving non-search routes when optional search is degraded.

## Current State

Story 06 Tasks 01 and 2 already moved application settings into `opentk-config`, renamed `complete-sync` to `opentk-sync`, removed legacy env/CLI setting reads, and added API search degradation during `build_app`. Remaining gaps for this task:

- Binaries still load config and then enter command-specific work before required dependency checks are modeled as a startup contract.
- `opentk-api` probes search through a full `/indexes/{index}/search` request in API startup, but `--validate-config` requires a lightweight Meilisearch stats-style validation report.
- `opentk-sync` and `search-sync` have no `--validate-config` path that checks dependencies and exits before work.
- Dependency failures are currently surfaced as lower-level `DatabaseError`, `SearchIndexError`, or `SyncFeedClientError` values, not a shared human-readable startup validation boundary.
- Validation timeouts are not consistently pinned to 5 seconds.

## Boundary Design

Use `improve-code-boundaries` to keep dependency reachability checks out of command bodies and avoid duplicate binary bootstrap code:

- Add a focused validation module to `opentk-config` because the validation target is loaded application config, not domain indexing or API routing behavior.
- `opentk-config` should own typed dependency probe errors and report formatting:
  - `DependencyKind::{Database, Meilisearch, SyncFeed}`
  - `DependencyStatus::{Reachable, Degraded { error }}`
  - `DependencyValidationError`
  - `DependencyValidationReport`
- Keep runtime client crates responsible for their own real protocol mechanics:
  - `opentk-db` can expose a small `validate_database(config, timeout)` function or keep `SELECT 1` implementation behind the config validation boundary if dependency direction stays clean.
  - `opentk-search` should expose a lightweight Meilisearch health/stats probe on `MeilisearchClient` rather than making callers fake a search query.
  - `opentk-sync` should expose a small SyncFeed base reachability probe that requests the first feed page URL or a base URL request with configured 5s timeout, whichever produces the clearest operator error while preserving SyncFeed URL construction rules.
- Binaries should only parse CLI intent, load config, call the shared validator for their dependency set, print/log the report, and either exit or continue into existing work.
- Do not put `--validate-config` as a subcommand. It is an operational flag on every binary and must be usable with no work command:
  - `opentk-api --validate-config [--require-search]`
  - `opentk-sync --validate-config`
  - `search-sync --validate-config`
- For `opentk-api`, optional search degradation is explicit:
  - Normal startup: database validation is fatal; search validation failure logs a degraded status and installs unavailable search as already supported.
  - `--validate-config`: exits 0 if database is reachable even when search is degraded; prints the degraded search status.
  - `--validate-config --require-search`: exits non-zero if search is unavailable.
- For `search-sync`, PostgreSQL and Meilisearch are both required because indexing is its primary function.
- For `opentk-sync`, PostgreSQL and SyncFeed base URL are both required. This task file calls the historical binary `complete-sync`; the actual current binary is `opentk-sync` after Task 02, so tests and implementation should use `opentk-sync`.

## Public Interface Shape

Prefer a small public validation API in `opentk-config` or a sibling module re-exported by it:

```rust
pub const STARTUP_VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);

pub enum SearchRequirement {
    Optional,
    Required,
}

pub struct DependencyValidationReport {
    pub database: DependencyCheckStatus,
    pub search: Option<DependencyCheckStatus>,
    pub syncfeed: Option<DependencyCheckStatus>,
}

pub enum DependencyCheckStatus {
    Reachable { target: String },
    Degraded { target: String, error: DependencyValidationError },
}

pub enum DependencyValidationError {
    DatabaseUnreachable { target: String, message: String },
    MeilisearchUnreachable { target: String, message: String },
    SyncFeedUnreachable { target: String, message: String },
}

pub async fn validate_api_dependencies(
    config: &Config,
    search_requirement: SearchRequirement,
) -> Result<DependencyValidationReport, DependencyValidationError>;

pub async fn validate_search_sync_dependencies(
    config: &Config,
) -> Result<DependencyValidationReport, DependencyValidationError>;

pub async fn validate_sync_dependencies(
    config: &Config,
) -> Result<DependencyValidationReport, DependencyValidationError>;
```

The final exact placement can change during implementation if dependency direction demands it, but the boundary must remain deep and shared. Avoid three separate copies of `SELECT 1`, request timeout setup, and report rendering in the binaries.

Human-readable messages should include the dependency kind, configured target, and operator-useful cause, for example:

- `Database at postgres://postgres@postgres:5432/opentk is unreachable: connection refused`
- `Meilisearch at http://meilisearch:7700 returned 401: invalid API key`
- `SyncFeed base URL https://gegevensmagazijn.tweedekamer.nl is unreachable: timeout`

Secret handling:

- Existing redacted config logging must remain redacted.
- Dependency error targets may include configured URLs because the task explicitly asks for target URLs in errors. Do not print search API keys or database passwords if the URL parser can redact credentials without weakening the requested clarity.

## TDD Execution Plan

Use vertical red/green tracer bullets. Do not write the whole suite first.

- [x] RED 1: Add a public-interface validation test for database success using a real migrated PostgreSQL test URL: validator runs `SELECT 1`, returns a reachable database status, and completes within the default 5s timeout contract. Confirm failure because the validation API does not exist.
- [x] GREEN 1: Implement the minimal database dependency validator with `tokio::time::timeout(STARTUP_VALIDATION_TIMEOUT, ...)`, `SELECT 1`, and a typed `DependencyValidationError::DatabaseUnreachable` display.
- [x] RED 2: Add a database failure test using an unreachable local port or malformed endpoint that asserts the typed human-readable message starts with `Database at ... is unreachable:` and does not get swallowed. Confirm failure before error formatting is complete.
- [x] GREEN 2: Normalize database connection/query/timeout errors into the shared dependency error message and ensure the timeout path is explicit.
- [x] RED 3: Add a Meilisearch validation test with a local HTTP fixture returning success for `/stats` or the chosen lightweight stats endpoint. Assert `search-sync` dependency validation requires it and reports reachable. Confirm failure because no stats probe exists.
- [x] GREEN 3: Add a lightweight Meilisearch probe to `opentk-search::MeilisearchClient`, honoring API key auth and the 5s validation timeout, and wire `validate_search_sync_dependencies`.
- [x] RED 4: Add a Meilisearch non-success test with a fixture returning `401 invalid API key`. Assert the error message is `Meilisearch at <url> returned 401: invalid API key`. Confirm failure before status/body formatting is added.
- [x] GREEN 4: Map Meilisearch transport, timeout, and non-success responses into typed validation errors without losing the status/body.
- [x] RED 5: Add SyncFeed reachability tests with a local fixture: success validates the configured base URL; timeout/non-success produces `SyncFeed base URL <url> is unreachable: <cause>`. Confirm failure before SyncFeed base validation exists.
- [x] GREEN 5: Add a small SyncFeed reachability probe that uses the configured 5s timeout and clear typed errors. Reuse existing URL/client construction instead of reimplementing SyncFeed URL logic in a binary.
- [x] RED 6: Add CLI parse tests for all binaries:
  - `opentk-api --validate-config` parses without a command.
  - `opentk-api --validate-config --require-search` parses.
  - `opentk-api --require-search` without `--validate-config` is rejected or produces a clear parse/runtime error.
  - `opentk-sync --validate-config` parses without `run|poll|status|verify`.
  - `search-sync --validate-config` parses without a subcommand.
  Confirm failure against current CLIs.
- [x] GREEN 6: Refactor CLI shapes so `--validate-config` is a global operational mode and command subcommands are optional only when validation mode is present. Keep existing non-validation commands unchanged.
- [x] RED 7: Add binary behavior tests around small startup helpers, not spawned shell processes:
  - API validation succeeds with reachable DB and unavailable search when `SearchRequirement::Optional`, and the returned/printed report includes degraded search.
  - API validation fails with unavailable search when `SearchRequirement::Required`.
  - `opentk-sync` validation requires DB plus SyncFeed.
  - `search-sync` validation requires DB plus Meilisearch.
  Confirm failure before helper wiring exists.
- [x] GREEN 7: Add binary-local validation branches that load config, call the shared validator, print reports, and exit before runtime work for validation mode.
- [x] RED 8: Add API startup test proving normal `serve/build_app` still fails fast for unreachable database but continues with unavailable search for non-search routes, and that search degradation uses the same Meilisearch validation/probe boundary as `--validate-config` instead of a full fake query. Confirm failure if the startup path still uses duplicate probe logic.
- [x] GREEN 8: Route API startup through the shared optional-search validation/probe path or a common lower-level search reachability function, then install `UnavailableSearchClient` on degraded search.
- [x] REFACTOR with `improve-code-boundaries`: scan for duplicate conversion helpers and repeated validation/report rendering in the three binaries. Keep one shared dependency report display path and one small config-to-runtime conversion at each external crate boundary. Remove any obsolete API startup probe helper that only duplicates Meilisearch validation.

## Verification

During execution, run narrow tests after each red/green cycle. Before completion, run in order:

- [x] `cargo fmt`
- [x] `make check`
- [x] `make lint`
- [x] `make test`

Do not run `make test-long` for this task unless a later story-level instruction explicitly requires it.

## Completion Steps

After verification passes:

- [x] Check every acceptance criterion in `task-03-startup-validation.md`.
- [x] Set `<passes>true</passes>` in `task-03-startup-validation.md`.
- Run `/bin/bash .ralph/task_switch.sh`.
- Commit all files, including `.ralph`, with `task finished task-03-startup-validation: ...` and include implementation summary plus `make check`, `make lint`, and `make test` evidence in the commit message.
- Push.
- Quit immediately.

NOW EXECUTE
