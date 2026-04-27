# Plan: Binary-Owned Schema Lifecycle

## Current State

- The schema source of truth already exists in Rust as `opentk_db::postgres_schema::schema()` plus SQL rendering helpers.
- The old migration-file workflow is still active through:
  - `migrations/20260426000000_sync_schema.{up,down}.sql`
  - `sqlx::migrate!("../../migrations")` in DB/API test helpers
  - `docker-compose.yml` mounting `./migrations:/workspace/migrations:ro`
  - `scripts/init-db.sh` running SQLx/psql migration files
  - docs that describe checked-in SQL migrations as the schema workflow
- `opentk-sync` currently connects and then uses `PostgresSyncStore` without first creating or validating schema.
- `opentk-api` currently validates database reachability only; it does not reject missing or incompatible schema.

## Boundary Design

Use `improve-code-boundaries`: put schema lifecycle ownership in `opentk-db`, not in binaries, Docker scripts, or test helpers.

Create one public Rust boundary in `opentk-db`, tentatively `schema_lifecycle`:

- `ensure_schema(pool: &PgPool) -> Result<(), SchemaLifecycleError>`
  - Sync-only path.
  - Creates missing schema objects from `postgres_schema::schema()` using SQL rendered from Rust.
  - Is idempotent and non-destructive.
  - Verifies the resulting live database schema matches the binary's expected `SchemaSpec`.
- `validate_schema(pool: &PgPool) -> Result<(), SchemaLifecycleError>`
  - API-only path.
  - Does not mutate the database.
  - Fails loudly when required tables, columns, indexes, constraints, trigger/function, or column nullability/type shape are missing or incompatible.
- `SchemaLifecycleError`
  - Typed, actionable errors such as missing table, missing column, incompatible column, missing index/constraint, execution failure.
  - No ignored errors and no silent fallbacks.

Move SQL execution details behind this boundary. Keep `postgres_schema` focused on declaring/rendering schema; keep binaries focused on startup orchestration.

## TDD Execution

Follow vertical red-green slices. Do not write all tests first.

1. Red: add a DB integration test proving `ensure_schema` creates expected schema in an empty PostgreSQL test schema.
   Green: implement minimal exported `ensure_schema` by executing the Rust-rendered create SQL and validating a sentinel table/index.
   Refactor: expose only the lifecycle API; keep render helpers private where possible.

2. Red: add a sync startup test around the sync runner/binary startup path proving an empty schema is initialized before syncing/writing status.
   Green: call `ensure_schema` from `opentk-sync` startup before constructing `PostgresSyncStore` for `run`, `poll`, `status`, and `verify` as applicable.

3. Red: add a restart/resume test that seeds `sync_category`, `sync_entity`, and one entity row, calls sync startup schema preparation again, and proves existing cursor/data is unchanged.
   Green: make schema creation idempotent and non-destructive by relying on `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`, and `CREATE OR REPLACE FUNCTION` only; no truncation/drop paths.

4. Red: add an API startup validation test that uses an empty PostgreSQL schema and proves API dependency validation fails with a schema-specific error.
   Green: call `validate_schema` from the API validation/startup path after database reachability succeeds.

5. Red: add an API incompatible-schema test that creates a conflicting table/column shape and proves validation fails.
   Green: compare live PostgreSQL catalog metadata against `SchemaSpec` for table presence, column type/nullability, primary/unique/check/foreign key presence, and expected indexes.

6. Red: add an API valid-schema test that first calls `ensure_schema`, then proves API validation/startup accepts the database.
   Green: wire `validate_schema` through `validate_api_dependencies` and normal `serve(api_config(config))` startup before serving.

7. Remove old workflow:
   - Delete root `migrations/`.
   - Delete `crates/opentk-db/tests/postgres_migrations.rs`.
   - Replace all test helper `sqlx::migrate!("../../migrations").run(...)` calls with `ensure_schema(&pool).await?`.
   - Remove Docker Compose migration mount.
   - Simplify or delete `scripts/init-db.sh` migration behavior if it is only a migration-file shim.
   - Update README/docs references from migration files/manual migration commands to binary-owned schema lifecycle.

8. Boundary cleanup pass:
   - Search with `rg -n "migrations|sqlx::migrate!|sqlx migrate|/workspace/migrations|\\.up\\.sql|\\.down\\.sql" .`.
   - Only this task/plan may retain historical references.
   - Remove any duplicate schema bootstrap helpers in tests; tests should call the same public `opentk-db` lifecycle API the binaries use.

9. Required checks:
   - `make check`
   - `make test`
   - `make lint`
   - Do not run `make test-long` unless execution discovers this task is finishing the whole story or the task file is changed to explicitly require it.

## Interface Decisions

- Public interface lives in `opentk-db`; binaries call it but do not know catalog details.
- Sync mutates schema by calling `ensure_schema`; API only validates by calling `validate_schema`.
- Schema source of truth remains `SchemaSpec`; no checked-in generated SQL remains.
- Startup validation errors are fatal and printable; they must include the object that is missing or incompatible.
- Tests use public behavior through `ensure_schema`, `validate_schema`, binary dependency validation, and sync store/runner behavior rather than private render functions.

NOW EXECUTE
