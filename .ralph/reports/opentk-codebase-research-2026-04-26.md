# OpenTK codebase research report

Date: 2026-04-26  
Workspace: `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk`  
Research mode: codebase inspection, local setup, linting, normal tests, long tests, real PostgreSQL startup, real API process, OpenAPI-first HTTP calls, and a bounded real SyncFeed run.

## Executive summary

OpenTK is a Rust workspace for ingesting Tweede Kamer SyncFeed XML into PostgreSQL, deriving search records for Meilisearch, and serving read APIs through Axum. The implementation is materially further along than the root README suggests: `opentk-api` is no longer merely a "future" boundary, and there are real read endpoints, generated OpenAPI, database migrations, document asset/content extraction, search indexing code, a complete-sync runner, and a sizable test suite.

The project can be spun up locally without Docker. The repo does not contain a Dockerfile or Compose file. The provided path is a native Rust and PostgreSQL workflow: `make test` runs `scripts/cargo-test-with-postgres.sh`, which initializes a local PostgreSQL cluster under `target/test-postgres`, starts it on `127.0.0.1:55432`, exports `OPENTK_TEST_DATABASE_URL`, and runs `cargo test --workspace`.

The code quality gates passed:

- `make check` passed: `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`.
- `make test` initially failed because `pg_ctl` timed out while starting PostgreSQL, but PostgreSQL became ready shortly afterward. Rerunning the same target passed the full normal workspace test suite.
- `make test-long` passed and executed the ignored live SyncFeed smoke tests.

The API can be started and queried for real once PostgreSQL is migrated. Fetching `/openapi.json` from the running server worked and reported OpenAPI `3.1.0`, title `OpenTK API`, version `0.1.0`, 12 paths, and 19 schemas. Using only that served OpenAPI surface as a client, basic endpoints worked: `/health`, `/categories`, `/sync/status`, `/changes/{category}`, error responses for invalid categories/limits, and 404s for missing resources. `/search` did not work in the manual run because no Meilisearch instance was running or configured; it returned a clean JSON `503` with `search_unavailable`.

A bounded real `complete-sync run --category Document` against the manual database was attempted with a 45 second timeout. It ingested 57,000 `sync_entity` rows, all deletion markers, advanced the Document cursor to skiptoken `15782211`, and left state `running`. It did not reach non-deleted document rows within that bound. This is useful evidence: the ingest path reaches the live upstream and writes PostgreSQL state, but the CLI lacks an obvious bounded "fetch N pages then stop successfully" operator mode.

Disk footprint is dominated by build and test artifacts, not source. Apparent file bytes measured during this run:

- `target`: about 3357.0 MiB.
- `target/test-postgres/data`: about 1770.0 MiB.
- `.ralph`: about 23.9 MiB.
- `.git`: about 1.3 MiB.
- `crates`: about 0.8 MiB.
- `migrations`: about 0.1 MiB.
- vendored official sources under `crates/opentk-sync/official_sources`: about 0.1 MiB.
- `docs`: less than 0.1 MiB by this rough file-byte method.
- Repository files excluding `target` and `.git`: roughly 28 MiB from an earlier `du` pass.

The most important gap is operational packaging and data bootstrapping. There is no Docker/Compose file, no one-command dev stack for API plus PostgreSQL plus Meilisearch, no served Swagger UI, and no documented bounded sample-ingest command for creating an immediately interesting API dataset. The tests are strong, but a human newcomer still has to infer too much from Makefile, scripts, CLI help, and docs.

## What this project is

OpenTK is a greenfield Rust rebuild of Tweede Kamer data tooling. Its declared source is SyncFeed XML. The intended production shape is:

1. Fetch SyncFeed pages from `https://gegevensmagazijn.tweedekamer.nl`.
2. Parse Atom entries and embedded official entity XML.
3. Write source metadata, typed entity tables, repeated scalar tables, relation tables, document asset metadata, and cursor state into PostgreSQL.
4. Fetch and extract document assets/content where applicable.
5. Build a Meilisearch read index from PostgreSQL.
6. Serve HTTP read endpoints from PostgreSQL and search endpoints from Meilisearch.

The workspace crates are:

- `crates/opentk-core`: source-neutral official schema/domain metadata.
- `crates/opentk-sync`: SyncFeed HTTP client, XML payload parser, official schema sources, document asset fetcher, document content extraction, and complete-sync runner logic.
- `crates/opentk-db`: PostgreSQL connection, migration/schema generation verification, sync writer, sync state, document asset persistence, read model, search sync, verification, and CLI binaries.
- `crates/opentk-search`: search mapping and Meilisearch client/search contract.
- `crates/opentk-search-eval`: search quality fixtures, scoring, and benchmark-style evaluation.
- `crates/opentk-api`: Axum HTTP API, generated OpenAPI, CLI binary, read endpoints, and search endpoint wrapper.

The project is a single Rust workspace with a virtual root. There is no root application crate.

## Repository contents

Top-level files and directories visible from a newcomer view:

- `Cargo.toml`: workspace members and shared dependencies/lints.
- `Cargo.lock`: pinned Rust dependency graph.
- `Makefile`: `check`, `lint`, `test`, and `test-long` targets.
- `README.md`: high-level description, workspace layout, local tools, and basic commands.
- `AGENTS.md`: contributor rules for this workspace.
- `migrations/`: SQLx migration pair for the complete sync schema.
- `scripts/cargo-test-with-postgres.sh`: local PostgreSQL test harness.
- `docs/`: architecture, storage, sync, search, and verification notes.
- `crates/`: all Rust crates.
- `.ralph/`: task/workflow metadata and now this report.
- `target/`: generated build/test/PostgreSQL data.

The codebase contains 1364 files outside `target` by a `find` count. It contains 42 vendored official source files under `crates/opentk-sync/official_sources`.

Rust source line count from `find crates -name '*.rs' | xargs wc -l` was 23,716 total lines across source and tests. This includes a substantial test body, not only production code.

## Public docs quality

The docs are useful but somewhat inconsistent with the implementation.

The root README says:

- `opentk-api` is a "future Axum HTTP API boundary".
- PostgreSQL is required for later importer/database stories.
- The setup task does not require PostgreSQL.

That is stale relative to the current code. The API exists and is test-covered, the normal test command requires PostgreSQL through the local harness, and the database/importer stories are implemented enough to run live smoke tests and write real sync rows.

The docs directory is stronger than the README. Notable docs:

- `docs/architecture.md`: explains intended data flow from SyncFeed to PostgreSQL to API.
- `docs/storage-model.md`: explains direct relational storage, source of truth, document assets/content, relations, repeated scalars, indexes, and migration tests.
- `docs/search-indexing-pipeline.md`: describes Meilisearch schema, mapping, delete/update propagation, cursoring, failure handling, and estimated index footprint.
- `docs/complete-sync-verification.md`: documents the verification CLI and live smoke tests.

Measured docs line counts:

- `docs/storage-model.md`: 193 lines.
- `docs/search-indexing-pipeline.md`: 123 lines.
- `docs/complete-sync-verification.md`: 105 lines.
- `docs/architecture.md`: 107 lines.
- Total docs lines: 1251.

For someone who cannot read the codebase at all, the README should be updated first. It is the entry point, and right now it undersells the project and omits the practical "how do I get an API with data" path.

## Build and dependency model

The workspace uses Rust edition 2021 and forbids unsafe code through workspace lints:

- `[workspace.lints.rust] unsafe_code = "forbid"`.
- `[workspace.lints.clippy] all = "deny"`.
- `[workspace.lints.clippy] pedantic = "deny"`.

Main shared dependencies:

- `axum` for HTTP.
- `tokio` for async runtime.
- `sqlx` for PostgreSQL and migrations.
- `reqwest` for HTTP clients.
- `quick-xml` and `roxmltree` for XML parsing.
- `scraper`, `pdf-extract`, and `zip` for document extraction support.
- `utoipa` for OpenAPI structures.
- `clap` for CLIs.
- `serde`/`serde_json` for serialization.
- `chrono` and `uuid` for domain values.
- `thiserror` for typed errors.
- `tracing` for runtime instrumentation.

There is no Docker dependency in the repo. PostgreSQL client/server binaries need to be available on the host for the provided test harness. This machine had PostgreSQL 16.11 tools available.

## Spinning up the project

### Native toolchain path

The intended local workflow is:

```bash
make check
make test
```

`make check` runs:

```bash
CARGO_INCREMENTAL=0 cargo fmt --check
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings
```

`make test` runs:

```bash
CARGO_INCREMENTAL=0 ./scripts/cargo-test-with-postgres.sh
```

The PostgreSQL helper:

- Creates `target/test-postgres`.
- Initializes `target/test-postgres/data` using `initdb -A trust -U postgres` if missing.
- Starts PostgreSQL via `pg_ctl`.
- Uses host `127.0.0.1`.
- Uses port `55432` by default, overrideable through `OPENTK_TEST_POSTGRES_PORT`.
- Exports `OPENTK_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/postgres`.
- Forces `CARGO_BUILD_JOBS=1` unless already set.
- Executes `cargo test --workspace`.

This is a solid CI-like local harness, but it assumes PostgreSQL server tools are installed. If `initdb` or `pg_ctl` are missing, the script fails rather than skipping database tests, which matches the repo's rule that missing test prerequisites should fail.

### Docker path

There is no Docker path in the repository.

I searched for:

- `Dockerfile`
- `docker-compose.yml`
- `docker-compose.yaml`
- `compose.yml`
- `compose.yaml`
- similarly named Docker files up to reasonable depth

No files were found.

For a newcomer, this means the project is not currently "clone and docker compose up". The host must provide Rust and PostgreSQL, and Meilisearch must be separately installed/configured for search to work.

### API startup path

The API binary help says:

```text
Usage: opentk-api [OPTIONS]

Options:
      --bind-address <BIND_ADDRESS>  [env: OPENTK_API_BIND_ADDRESS=] [default: 127.0.0.1:3000]
      --database-url <DATABASE_URL>  [env: OPENTK_DATABASE_URL=]
  -h, --help                         Print help
```

The API also falls back to `DATABASE_URL` if `--database-url` and `OPENTK_DATABASE_URL` are not supplied. If none are present, it fails with a clear error asking for one of those values.

For my manual run, I:

1. Used the already-running test PostgreSQL on port `55432`.
2. Created a schema named `opentk_manual_api`.
3. Applied `migrations/20260426000000_complete_sync_schema.up.sql` with `psql`.
4. Started the API on `127.0.0.1:3107` with a database URL setting PostgreSQL `search_path` to the manual schema.

The server started and served requests.

## Test results

### Lint lane

Command:

```bash
make check
```

Result: passed.

Important output:

```text
CARGO_INCREMENTAL=0 cargo fmt --check
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
```

No lints were ignored or downgraded.

### Normal test lane

Command:

```bash
make test
```

First result: failed before tests ran.

Important output:

```text
pg_ctl: server did not start in time
make: *** [Makefile:10: test] Error 1
```

This was not ignored. I inspected readiness and logs. PostgreSQL was visible in the process list and later reported ready:

```text
127.0.0.1:55432 - accepting connections
```

The PostgreSQL log showed startup took long enough to cross `pg_ctl`'s wait threshold:

```text
starting PostgreSQL 16.11 ...
listening on IPv4 address "127.0.0.1", port 55432
listening on Unix socket "/tmp/opentk-test-postgres-joshazimullah/.s.PGSQL.55432"
database system is ready to accept connections
```

Second result: passed.

Notable passing areas:

- API health, OpenAPI, startup, core read endpoints, deep HTTP verification, pagination, errors, relation expansion, concurrency, document content serving, search API behavior.
- Core official schema and workspace smoke.
- DB connection, migrations, schema generation, sync writer, sync state, sync verification, document assets, search sync.
- Search mapping, Meilisearch client behavior with fixtures/mocks, schema docs.
- Search evaluation fixtures and scoring.
- SyncFeed client, payload parser, official schema sources, complete sync runner, document asset fetcher, document content extractor.
- Doc-tests for all crates.

Examples from the normal lane:

```text
tests/api.rs: 27 passed
tests/health.rs: 2 passed
tests/openapi.rs: 1 passed
tests/startup.rs: 1 passed
tests/postgres_migrations.rs: 1 passed
tests/postgres_schema.rs: 9 passed
tests/search_sync.rs: 6 passed
tests/sync_verification.rs: 7 passed, 2 ignored
tests/sync_writer.rs: 5 passed
tests/complete_sync_runner.rs: 5 passed
tests/document_asset_fetcher.rs: 5 passed
tests/document_content_extractor.rs: 6 passed
tests/syncfeed_client.rs: 8 passed
```

The two ignored tests in the normal lane are the live long-lane tests. They were not left unrun; I ran the long lane next.

### Long test lane

Command:

```bash
make test-long
```

Result: passed.

Important output:

```text
Running tests/sync_verification.rs
running 2 tests
test live_controlled_document_content_records_hashes_and_provenance ... ok
test live_syncfeed_page_can_be_written_and_verified ... ok
```

The long lane executed the ignored live verification tests and then filtered out the normal deterministic tests. This confirms the live smoke path is selectable and currently passes.

## Data availability

### Source data in the repository

The repository does not contain a full Tweede Kamer dataset. It contains:

- Vendored official XSD/schema sources.
- Parser and document content fixtures.
- Search evaluation fixtures.
- Migration SQL.
- Test fixtures embedded in Rust tests.

It does not contain a production PostgreSQL dump, a prebuilt Meilisearch index, or bulk SyncFeed data.

The official sources count was 42 files under `crates/opentk-sync/official_sources`. These are schema/source-model inputs, not the actual government data corpus.

### Live data availability

The live SyncFeed source was reachable during tests. `make test-long` passed its live SyncFeed page verification. A manual bounded `complete-sync run --category Document` also reached the upstream and wrote PostgreSQL rows.

Before the manual sync run:

```text
Document state=not_started latest_skiptoken=null lag_seconds=null last_fetch_at=null last_error=none
```

After a 45 second externally timed run:

```text
Document state=running latest_skiptoken=15782211 lag_seconds=14 last_fetch_at=2026-04-26T09:27:01.615984+00:00 last_error=none
```

Database counts afterward:

```text
sync_entity_count: 57000
document_count: 0
Document sync_category latest_skiptoken: 15782211
Document state: running
has_next_url: true
has_resume_url: false
```

All 57,000 `sync_entity` rows were deleted `Document` registry rows:

```text
source_category | deleted | count
Document        | true    | 57000
```

This is a real but not immediately satisfying dataset for API browsing. `/changes/Document` returned rows, but typed `/documents/{id}` returned 404 because the early ingested rows were deletions and there were no current `document` rows yet.

### Practical conclusion on data

The data is not "all here". The repo has schemas and fixtures; real data must be fetched from SyncFeed. The live fetch path works, but the CLI is oriented toward ongoing durable sync rather than a bounded demo dataset. For a new person trying the API, this is a gap: there is no obvious "load a small useful dataset and stop" command.

## HTTP API contract and real client testing

### Served OpenAPI

I fetched:

```bash
curl -sS http://127.0.0.1:3107/openapi.json
```

The served OpenAPI document reported:

```json
{
  "openapi": "3.1.0",
  "title": "OpenTK API",
  "version": "0.1.0",
  "path_count": 12,
  "schema_count": 19
}
```

OpenAPI paths:

```text
GET /activities/{source_id}
GET /categories
GET /changes/{category}
GET /documents/{source_id}
GET /documents/{source_id}/content
GET /entities/{category}/{source_id}
GET /health
GET /openapi.json
GET /persons/{source_id}
GET /relations/{category}/{source_id}
GET /search
GET /sync/status
```

The OpenAPI document was enough to discover the route list and basic parameters. However, it is very minimal for a human client:

- No examples.
- No enum schemas/details for query parameters such as `relations`, `direction`, or `entity_kind`.
- Parameter schemas are not very descriptive from the client perspective.
- No Swagger UI or Redoc endpoint is served.
- No "try this first" data-loading guidance exists in the API docs.

### Real API calls using the served contract

All calls below were made with `curl` against the running API, using only the served OpenAPI route list and parameter names as the client contract.

#### Health

Request:

```http
GET /health
```

Response:

```http
HTTP/1.1 200 OK
content-type: application/json

{"status":"ok"}
```

This worked well.

#### Categories

Request:

```http
GET /categories
```

Response: `200 OK` with 35 official categories. Example entries:

```json
{"category":"Activiteit","table":"activiteit","field_count":24,"relation_count":3}
{"category":"Document","table":"document","field_count":15,"relation_count":6}
{"category":"Persoon","table":"persoon","field_count":17,"relation_count":0}
```

This is useful and works on an empty database because it comes from schema metadata.

#### Sync status

Request:

```http
GET /sync/status
```

Before manual sync, every category returned with null sync state. After the bounded Document sync run, `Document` showed progress through the CLI. The API status endpoint is useful because it shows all categories even if no data has been synced yet.

#### Changes

Request before data:

```http
GET /changes/Document?limit=2
```

Response before data:

```json
{"category":"Document","items":[],"next_skiptoken":null,"has_more":false}
```

Request after bounded live sync:

```http
GET /changes/Document?limit=2
```

Response after bounded live sync:

```json
{
  "category": "Document",
  "items": [
    {
      "category": "Document",
      "source_id": "01154b88-42a7-4646-91e7-371bf19e7057",
      "latest_skiptoken": 3484982,
      "deleted": true,
      "source_updated_at": "2019-07-02T23:59:45.287+00:00",
      "atom_updated_at": "2026-04-26T09:26:18.420611+00:00"
    },
    {
      "category": "Document",
      "source_id": "02153b36-a3fb-4f48-a751-9abb9ccb41be",
      "latest_skiptoken": 3484982,
      "deleted": true,
      "source_updated_at": "2019-07-02T10:32:56.413+00:00",
      "atom_updated_at": "2026-04-26T09:26:18.420611+00:00"
    }
  ],
  "next_skiptoken": 3484982,
  "has_more": true
}
```

This endpoint worked and exposed useful cursor metadata. It also made a product issue clear: a client can land on deletion markers before it can discover current document content. That is probably correct for change feeds, but it is not a friendly demo or first-use experience.

#### Search

Request:

```http
GET /search?q=belasting&limit=2
```

Response:

```http
HTTP/1.1 503 Service Unavailable
content-type: application/json

{"code":"search_unavailable","message":"search unavailable"}
```

This is a clean failure. It does not hide the missing search backend. From an operator perspective, the repo does not provide the backend or a Compose file, so this is expected but inconvenient.

#### Missing resources and invalid requests

Request:

```http
GET /documents/01154b88-42a7-4646-91e7-371bf19e7057
```

Response:

```http
HTTP/1.1 404 Not Found
{"code":"not_found","message":"resource not found"}
```

Request:

```http
GET /changes/Nope?limit=1
```

Response:

```http
HTTP/1.1 404 Not Found
{"code":"not_found","message":"resource not found"}
```

Request:

```http
GET /changes/Document?limit=0
```

Response:

```http
HTTP/1.1 400 Bad Request
{"code":"invalid_request","message":"invalid request"}
```

The API error responses are consistent JSON. They are terse, but they do not swallow errors or return HTML.

### Does the OpenAPI-first API experience work?

Partly.

What works:

- `/openapi.json` is served.
- All main routes are discoverable.
- Core metadata endpoints work with an empty database.
- Change pagination works after real sync rows exist.
- Error shape is consistent.
- Health reflects database availability.

What does not work well:

- No interactive docs UI.
- No examples in the OpenAPI document.
- No documented way to populate a small useful dataset before trying typed endpoints.
- Search route is discoverable but unusable unless Meilisearch is separately available and indexed.
- Typed entity endpoints are not rewarding after a bounded initial Document run because the run hit deletion markers first.
- The OpenAPI spec is generated manually enough that parameter schema detail is thin.

## CLI behavior

### `opentk-api`

The API CLI is straightforward. Required database configuration is clear. It can bind to a custom address. It starts cleanly when the schema is migrated.

### `complete-sync`

Top-level help:

```text
Run or inspect durable Tweede Kamer SyncFeed ingestion

Usage: complete-sync <COMMAND>

Commands:
  run
  poll
  status
  verify
  help
```

`complete-sync status --category Document` worked well and produced human-readable status.

`complete-sync run --category Document` performs real work, but in my manual exploration it did not print progress during the 45 second bounded run. It wrote 57,000 registry rows and advanced state, but as a user watching the terminal, there was no visible progress. The command also has no obvious `--max-pages`, `--max-entries`, `--until-skiptoken`, or `--sample` option.

This is the largest operator-experience gap in the CLI: the durable sync architecture is good, but there is no intentionally bounded "demo/import smoke" command for humans.

### `search-sync`

Top-level help:

```text
Build or update the OpenTK Meilisearch index from PostgreSQL

Usage: search-sync <COMMAND>

Commands:
  full-reindex
  incremental
  failures
  retry-failures
  help
```

I did not run a real Meilisearch-backed search sync because the repo does not provide Meilisearch and I did not find a configured local service. Search behavior is test-covered and the API correctly returns `search_unavailable` when the backend is absent.

## Storage model and database shape

The database model is direct relational storage, not JSONB-first storage.

Core ideas:

- `sync_category`: cursor and category-level sync state.
- `sync_entity`: registry row for each `(source_category, source_id)`.
- one typed table per official entity category, such as `document`, `persoon`, `activiteit`.
- generated relation tables named like `<source_entity>__<relation_name>`.
- generated repeated-scalar side tables where needed.
- `document_asset`: durable upstream document fetch metadata and retrieval status.
- `document_content`: selected source, extraction status, validation status, hashes, extracted text/HTML, and provenance.
- `search_index_cursor` and `search_index_failure`: durable search indexing state/failure tracking.

The migration creates a large schema: dozens of official entity tables and hundreds of indexes/constraints. The tests verify the checked-in migration against generated schema metadata, which is strong protection against drift.

The storage design is appropriate for a system that wants:

- typed SQL queryability,
- cursor-safe sync,
- read model endpoints without reparsing XML,
- durable error evidence,
- generated coverage of official schema fields and relations.

The tradeoff is startup/demo complexity. You need PostgreSQL, migrations, and live ingest before most typed endpoints become interesting.

## Search architecture

Search is a derived Meilisearch index named `opentk_entities`, with deterministic primary keys:

```text
<source_category>:<source_id>
```

The docs and tests show:

- documents map title, number, date, content metadata, extracted text/HTML, and relation labels;
- people map display names and metadata;
- activities, dossiers, and other entities use fallback title logic;
- deleted source rows map to delete operations;
- incremental indexing follows `sync_entity`;
- failures are durable and retryable.

The API search endpoint is not raw Meilisearch JSON. It returns a stable DTO with:

- `key`
- `source_category`
- `source_id`
- `entity_kind`
- `title`
- `summary`
- `source_url`
- `api_url`
- `date`
- `document_number`
- `snippets`
- `ranking_score`

This is a good boundary. The missing piece is the local runtime packaging for Meilisearch and the index bootstrap instructions.

## Error handling quality

The codebase appears aligned with the repo instruction not to swallow errors:

- The API maps database, read-model, search, invalid request, bind, and serve failures into explicit `ApiError` variants.
- HTTP responses use typed JSON error responses.
- SyncFeed client tests cover retryable statuses, response failures, invalid resume URLs, and adaptive pacing.
- Document asset/content code persists failures rather than creating fake content.
- Search indexing has durable failure tables and tests proving cursor does not advance on index failure.
- Sync writer tests cover rollback when later entity writes fail.
- Complete-sync runner tests cover crash-before and crash-after cursor commit cases.

I did not find a case during this research where an error was silently swallowed. The main issue I hit was a PostgreSQL startup timeout; it was visible and caused `make test` to fail, which is correct.

## Disk footprint

The source itself is small. The generated/runtime artifacts are large.

Measured apparent file bytes:

```text
.git                                      1.3 MiB
.ralph                                   23.9 MiB
crates                                    0.8 MiB
docs                                     <0.1 MiB
migrations                                0.1 MiB
crates/opentk-sync/official_sources       0.1 MiB
target/test-postgres/data              1770.0 MiB
target                                 3357.0 MiB
```

Earlier `du -sh --exclude=.git --exclude=target .` reported about 28 MiB for the repository excluding `.git` and `target`.

Important interpretation:

- `target` is the main disk consumer because it contains Rust build outputs, test binaries, and local PostgreSQL data.
- `target/test-postgres/data` alone is about 1.7 GiB apparent file bytes after the existing project/test history and this run.
- Source/docs/migrations are tiny by comparison.
- `.ralph` is larger than source because it contains task/archive/progress metadata.

I attempted direct `du -sh` over `target`, but it was slow enough that I switched to bounded apparent file-byte measurements. The numbers above are good enough for orientation, but not a filesystem block-accurate audit.

## Newcomer experience

A person who cannot read the codebase can still get somewhere, but the happy path is not as smooth as it should be.

What is good:

- `README.md` tells them to run `make check` and `make test`.
- `Makefile` is simple.
- The test script provisions PostgreSQL automatically if PostgreSQL tools exist.
- Errors fail visibly.
- The code has strong tests and docs once a person knows where to look.
- API CLI help is clear.
- `/openapi.json` exists.

What is rough:

- README is stale about API/database maturity.
- No Docker or Compose setup.
- No Meilisearch setup for local search.
- No one-command "run API with database and search".
- No one-command "load sample data".
- No Swagger UI or examples.
- `complete-sync run` can do real work without visible progress.
- The first live Document data encountered in a bounded run was deletion markers, so typed document API exploration gave 404s.
- Some docs describe intended future/API route shapes that differ from actual routes, such as older `/meta/categories` and `/changes?category=...` examples in architecture notes.

## Strengths

- Strong Rust workspace boundaries.
- Direct relational schema is generated/verified instead of hand-maintained in many places.
- Tests cover real PostgreSQL migrations and real SQL-visible state.
- Long lane reaches live SyncFeed.
- Document extraction is exact fixture-based for supported formats.
- Error states are durable rather than hidden.
- Search indexing has clear source-of-truth and failure recovery design.
- API returns stable JSON error bodies.
- The code is lint-clean with strict clippy settings.

## Risks and gaps

### 1. Local runtime stack is incomplete

There is no Docker/Compose or documented native equivalent for PostgreSQL plus Meilisearch plus API. Search fails cleanly, but a newcomer cannot evaluate search without extra setup.

Recommendation: add `docker-compose.yml` or a documented `just`/Make target that starts PostgreSQL and Meilisearch, runs migrations, and starts the API.

### 2. README is stale

The README says the API is future and PostgreSQL is only for later stories. That is not true anymore.

Recommendation: update README to reflect current binaries, endpoints, migrations, local PostgreSQL harness, Meilisearch requirement, and long test lane.

### 3. No bounded sample ingest mode

`complete-sync run --category Document` is real but open-ended. In 45 seconds it wrote 57,000 deletion registry rows and did not yield current typed documents.

Recommendation: add one or more of:

- `--max-pages`
- `--max-entries`
- `--until-skiptoken`
- `complete-sync sample`
- fixture-backed seed command for API demos

### 4. No progress output during manual sync

The manual bounded run produced no visible progress before timeout, despite writing many rows.

Recommendation: emit periodic progress lines or tracing logs by default for CLI runs, including category, pages fetched, entries written, latest skiptoken, state, and next URL presence.

### 5. OpenAPI is usable but thin

The contract lists routes, parameters, and schemas, but lacks examples and rich parameter schema detail.

Recommendation: add examples for common responses and error responses, document query enum values, and consider serving Swagger UI or Redoc at `/docs`.

### 6. Search cannot be evaluated from repo alone

Tests cover search behavior, but runtime search needs Meilisearch separately.

Recommendation: document exact Meilisearch startup and indexing commands, including environment variables:

- `OPENTK_SEARCH_URL`
- `OPENTK_SEARCH_API_KEY`
- `OPENTK_SEARCH_INDEX`

### 7. PostgreSQL startup can exceed `pg_ctl` wait threshold

The first `make test` failed because `pg_ctl` timed out, then PostgreSQL became ready shortly afterward.

Recommendation: make `scripts/cargo-test-with-postgres.sh` more robust by following `pg_ctl start` with an explicit bounded `pg_isready` loop and clearer diagnostics. Do not skip tests; fail only after the readiness loop genuinely expires.

## Recommended next work

1. Update `README.md` to match the current project.
2. Add a local dev stack target for PostgreSQL plus Meilisearch plus API.
3. Add a bounded sample ingest or deterministic seed command.
4. Add OpenAPI examples and query enum schemas.
5. Improve CLI progress output for complete-sync.
6. Harden PostgreSQL startup readiness in the test script.
7. Add a short "first API session" doc:

```text
start database
run migrations
load sample/live bounded data
start API
open /openapi.json or /docs
try /health, /categories, /sync/status, /changes/Document
start Meilisearch
run search-sync full-reindex
try /search?q=...
```

## Commands run

Representative commands run during this research:

```bash
make check
make test
make test-long
pg_isready -h 127.0.0.1 -p 55432 -U postgres
psql 'postgres://postgres@127.0.0.1:55432/postgres' ...
cargo run -p opentk-api --bin opentk-api -- --help
cargo run -p opentk-db --bin complete-sync -- --help
cargo run -p opentk-db --bin complete-sync -- run --help
cargo run -p opentk-db --bin search-sync -- --help
target/debug/opentk-api --bind-address 127.0.0.1:3107
curl -sS http://127.0.0.1:3107/openapi.json
curl -sS -i http://127.0.0.1:3107/health
curl -sS -i http://127.0.0.1:3107/categories
curl -sS -i http://127.0.0.1:3107/sync/status
curl -sS -i 'http://127.0.0.1:3107/changes/Document?limit=2'
curl -sS -i 'http://127.0.0.1:3107/search?q=belasting&limit=2'
timeout 45 target/debug/complete-sync run --database-url ... --category Document
```

## Final assessment

This is a serious, test-heavy greenfield codebase with real ingestion, storage, search, and API boundaries. The internal engineering quality is much stronger than the first-run product experience. The tests give high confidence that core behavior is not fake: migrations run, PostgreSQL is exercised, live SyncFeed can be fetched in the long lane, document extraction persists real content/failures, and the HTTP API is hit through real server tests.

For a developer who can read Rust, the project is navigable. For someone who cannot read the codebase, the missing pieces are operational packaging, current README guidance, demo data loading, and richer API docs. Fixing those would make the project feel much more complete without requiring major architectural changes.
