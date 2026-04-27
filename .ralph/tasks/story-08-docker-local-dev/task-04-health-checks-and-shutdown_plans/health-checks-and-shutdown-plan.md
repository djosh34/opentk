# Story 08 Task 04 Plan: Health Checks and Graceful Shutdown

## Context Read

- Current task: `.ralph/tasks/story-08-docker-local-dev/task-04-health-checks-and-shutdown.md`.
- Existing API boundary:
  - `crates/opentk-api/src/lib.rs` already exposes `/health`, `/admin/search-sync/status`, and `serve_with_shutdown`.
  - `serve_with_shutdown` already uses Axum graceful shutdown and the default `serve` path already combines Ctrl-C and SIGTERM in `shutdown_signal`.
  - `/health` currently checks PostgreSQL, then calls `ensure_search_sync_usable`, which makes search CDC degradation a `503`.
  - `HealthResponse` currently only contains `{ "status": "ok" }`.
  - `ApiState` stores `Arc<dyn SearchQueryClient>` and `SearchCdcRuntimeStatus`, but the search query trait has no lightweight health method.
- Existing search boundary:
  - `opentk-search::MeilisearchClient` already has `validate_reachable()` using a lightweight `/stats` request.
  - `SearchQueryClient` only models search requests. Reusing it for health would mix query behavior with dependency health.
- Existing sync boundary:
  - `crates/opentk-db/src/bin/opentk-sync.rs` owns CLI parsing and runner construction.
  - `CompleteSyncRunner::run_forever` loops forever with `run_once` plus sleep and has no shutdown-aware public interface.
  - A graceful poll stop should be controlled through the runner boundary so tests can verify behavior without spawning a process.
- Existing Docker boundary:
  - `docker/Dockerfile.api`, `docker/Dockerfile.sync`, and `docker/Dockerfile.scratch` all use `scratch`.
  - A `scratch` image has no `curl` binary, so the exact API healthcheck command from the task cannot work unless the runtime image includes a healthcheck-capable binary.
  - Since the task explicitly requires `curl -f http://localhost:3000/health || exit 1`, the least surprising path is to switch `docker/Dockerfile.api` and `docker/Dockerfile.sync` runtime stages to a minimal Debian runtime with CA certs and curl installed.
  - `docker/Dockerfile.scratch` is the generic static scratch artifact path from Task 03; it cannot use curl. Its executable can support `HEALTHCHECK` through CLI flags, but Dockerfile syntax cannot know whether `BINARY` is API or sync without selecting by `ARG`.
- Existing test boundary:
  - API health tests already use the public Axum router.
  - Docker/compose/docs contracts already live in `crates/opentk-config/tests/config_loading.rs`.
  - Sync runner behavior tests already use a public `CompleteSyncRunner` with a memory store and local HTTP server.

## Public Interfaces To Add Or Change

- Extend the API health response:
  - `GET /health` returns `200` with `status: "ok"` when PostgreSQL, Meilisearch, and search sync are usable.
  - `GET /health` returns `200` with `status: "degraded"` when PostgreSQL is reachable but optional search-side dependencies are not fully usable.
  - `GET /health` returns `503` only when PostgreSQL or another required core API dependency is unavailable.
  - Include dependency details as stable JSON fields, not stringly concatenated messages:
    - `postgres: "ok" | "unavailable"`
    - `meilisearch: "ok" | "unavailable"`
    - `search_sync`: the same daemon snapshot shape already served by `/admin/search-sync/status`.
  - Existing `{ "status": "ok" }` is not preserved for backwards compatibility; this project has no legacy users.
- Add a focused search health boundary:
  - Introduce a small public trait in `opentk-search`, for example `SearchHealthClient`, with `fn health<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<(), SearchIndexError>> + Send + 'a>>`.
  - Implement it for `MeilisearchClient` by calling the existing `validate_reachable()`.
  - Store a single API client object behind a trait that can both search and health-check, or split `ApiState` into two explicit clients if that keeps the interface cleaner.
  - Avoid adding health behavior to `SearchQueryClient`; query and dependency health are separate boundaries.
- Add sync CLI health check:
  - Add a global `--health-check` flag to `opentk-sync`.
  - It loads config, attempts a PostgreSQL connection through the existing `connect(database_config(config))` path, and exits `0` on success.
  - Any config or database failure is returned and exits non-zero through `main`, with no swallowed errors.
  - `--health-check` must not require a subcommand.
- Add graceful sync shutdown:
  - Add a runner method such as `run_until_shutdown(shutdown: impl Future<Output = ()> + Send)` or a small `ShutdownSignal` abstraction.
  - In `opentk-sync poll`, wait for Ctrl-C or SIGTERM and request shutdown.
  - Shutdown should not abort a page in progress. The runner should check for shutdown between `run_once` cycles and between sleeps, then exit `Ok(())`.
  - If the current implementation processes one page per category inside `run_once`, treat the current `run_once` as the atomic unit. Do not interrupt it mid-page.
- Add Docker healthchecks:
  - `docker/Dockerfile.api`: `HEALTHCHECK CMD curl -f http://localhost:3000/health || exit 1`.
  - `docker/Dockerfile.sync`: `HEALTHCHECK CMD /bin/opentk-sync --config /etc/opentk/config.toml --health-check || exit 1`.
  - `docker/Dockerfile.scratch`: add an `ARG HEALTHCHECK_KIND=none` and generate an appropriate `HEALTHCHECK` for `opentk-api` or `opentk-sync` only if this can be done without undermining the scratch contract. If this conflicts with scratch constraints, move the plan back to `TO BE VERIFIED`.
  - `docker-compose.yml`: add service-level healthcheck entries for `api` and `sync` if Dockerfile healthchecks are insufficient or compose needs explicit intervals/retries.
- Add `docs/operations.md`:
  - Document `/health` status semantics, degraded search behavior, Docker healthchecks, `opentk-sync --health-check`, and graceful shutdown expectations.
  - Link it from existing Docker compose docs if useful.

## TDD / Verification Plan

Use vertical slices. Do not batch all tests before implementation.

1. RED tracer for degraded API health:
   - Change one existing API health test so PostgreSQL reachable plus unavailable Meilisearch returns `200` and a body with `status: "degraded"` and `search_sync` details.
   - Expected first failure: current `/health` returns `503 search_sync_degraded`.
   - GREEN: adjust `HealthResponse` and `/health` to degrade instead of erroring when only search-side state is unhealthy.

2. RED for required database health:
   - Keep or update the existing unreachable database test to assert `503 database_unavailable`.
   - GREEN: preserve PostgreSQL as the core dependency gate.

3. RED for live Meilisearch health boundary:
   - Add an `opentk-search` test that a lightweight health method sends the existing authenticated request and maps non-2xx/transport errors into `SearchIndexError`.
   - GREEN: add the `SearchHealthClient` trait and implement it for `MeilisearchClient` by delegating to `validate_reachable()`.

4. RED for `/health` Meilisearch down while search sync is otherwise usable:
   - Add an API router test with a fake health client that fails Meilisearch health while PostgreSQL succeeds and CDC status is running.
   - GREEN: wire API state to call the search health boundary and report `meilisearch: "unavailable"` with HTTP 200 degraded.

5. RED for `/health` search sync state inclusion:
   - Add assertions that the health body includes the daemon snapshot under `search_sync`.
   - GREEN: reuse `search_sync_daemon_status_response` to avoid duplicate DTO mapping.

6. RED for `opentk-sync --health-check` CLI parsing:
   - Add unit coverage in `crates/opentk-db/src/bin/opentk-sync.rs` parser tests.
   - GREEN: add the global flag and command dispatch that allows no subcommand when health check is set.

7. RED for sync graceful shutdown:
   - Add one runner behavior test in `crates/opentk-sync/tests/opentk_sync_runner.rs`.
   - Use the existing local HTTP fixture with a delayed page response, start `run_until_shutdown`, fire shutdown while the page is in flight, and assert the page was written before the runner exits.
   - GREEN: add shutdown-aware runner method that waits for the current `run_once` to finish, then stops before the next poll cycle.

8. RED for Dockerfile and compose healthcheck contracts:
   - Extend `crates/opentk-config/tests/config_loading.rs` to assert:
     - all Dockerfiles contain `HEALTHCHECK`.
     - API healthcheck uses `curl -f http://localhost:3000/health || exit 1`.
     - sync healthcheck uses `opentk-sync --health-check`.
     - compose `api` and `sync` services expose healthcheck configuration or inherit the Dockerfile healthchecks intentionally.
   - GREEN: update Dockerfiles and compose healthcheck intervals/retries.
   - If `scratch` cannot honestly support the required healthcheck, switch this task and plan back to `TO BE VERIFIED` before modifying production code further.

9. RED for operations docs:
   - Add a repository contract assertion that `docs/operations.md` mentions `/health`, `degraded`, `opentk-sync --health-check`, Docker healthchecks, SIGTERM, and graceful shutdown.
   - GREEN: write the docs.

10. Manual Docker verification after tests are green:
    - `docker compose up --build -d`
    - `docker compose ps` should show services healthy.
    - `curl http://localhost:3000/health` should show ok or degraded dependency JSON.
    - `docker stop` the API container and verify it exits within 10 seconds.
    - Send SIGTERM to the sync container and verify it exits cleanly after its current page or sleep boundary.
    - Do not run `make test-long`; this is not marked as a story-ending task.

11. Required end checks:
    - `make check`
    - `make test`
    - `make lint`

## Code Boundary Checks

Apply `$improve-code-boundaries` during execution:

- Split search query and search health into separate trait boundaries instead of overloading `SearchQueryClient`.
- Reuse the existing search sync status DTO mapper for `/health`; do not create a second duplicate shape by hand.
- Keep CLI health checking in the sync binary boundary and database connection logic in `opentk-db`; do not add ad hoc SQL strings outside the existing connection layer unless the existing layer cannot expose the needed behavior.
- Keep shutdown semantics in the runner public interface so behavior can be tested without process-level signal hacks.
- Remove `ensure_search_sync_usable` if it becomes a misleading boundary after `/health` starts reporting degraded search-side status instead of rejecting the request.
- Do not swallow signal-handler installation errors. API already logs handler setup errors; sync should return or log explicitly according to the signal type.
- If adding curl forces runtime-image changes, keep Docker image responsibilities explicit: builder compiles binaries, runtime provides only binary, CA certs, identity files, and healthcheck tool.
- Avoid adding compatibility shims for the old `/health` body; this project has no users and no backwards compatibility requirement.

## Acceptance Mapping

- `/health` returns 200 degraded when Meilisearch is down and PostgreSQL is healthy:
  - API health tests plus fake search health failure cover this.
- `/health` returns 503 when PostgreSQL or another required core dependency is down:
  - unreachable database health test preserves this.
- All Dockerfiles contain `HEALTHCHECK` instructions:
  - repository contract test covers `docker/Dockerfile.api`, `docker/Dockerfile.sync`, and `docker/Dockerfile.scratch`.
- `docker compose ps` shows all services as healthy:
  - manual Docker verification covers this after implementation.
- `opentk-api` shuts down gracefully within 10s of `docker stop`:
  - existing Axum graceful shutdown remains in place; manual Docker verification covers the container behavior.
- `opentk-sync poll` exits cleanly on SIGTERM after current page:
  - runner test covers current-page completion; manual Docker verification covers signal wiring.
- `docs/operations.md` documents health and shutdown:
  - docs contract test plus file update covers this.
- `make check`, `make test`, `make lint`:
  - run after implementation and before marking task passing.

NOW EXECUTE
