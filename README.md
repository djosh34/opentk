# OpenTK

OpenTK is a Rust rebuild of the Tweede Kamer data tooling. The initial source is
SyncFeed XML, applied directly into PostgreSQL through `sqlx`.

## Workspace Layout

- `crates/opentk-core`: source-neutral domain and shared value types.
- `crates/opentk-sync`: SyncFeed fetching, parsing, cursor handling, and importer orchestration.
- `crates/opentk-db`: PostgreSQL access, SQLx query code, transactions, and migrations.
- `crates/opentk-api`: future Axum HTTP API boundary.
- `migrations`: SQLx migration files.
- `docs`: product, architecture, and exploration notes.

The root is a virtual Cargo workspace. There is no umbrella crate; later stories
should depend on the crate that owns the boundary they need.

## Local Tools

Install the Rust stable toolchain with `rustup`, including `rustfmt` and
`clippy`.

Required external services for later importer/database stories:

- PostgreSQL
- Network access to the SyncFeed source

This setup task does not require a running PostgreSQL instance.

For the containerized local development stack, see
[`docs/docker-compose.md`](docs/docker-compose.md).

For static multi-architecture scratch runtime images, see
[`docs/docker-scratch.md`](docs/docker-scratch.md).

## Commands

Run these before finishing normal development tasks:

```bash
make check
make test
```

`make check` is the lint lane and runs:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

`make test` runs the default test suite:

```bash
./scripts/cargo-test-with-postgres.sh
```

`make test-long` is reserved for story-ending or explicitly requested long/e2e
validation. Do not run it as the default end-of-task check.

The durable SyncFeed runner is exposed by the `opentk-sync` binary from
`opentk-db`:

```bash
cargo run -p opentk-db --bin opentk-sync -- --config ./opentk.toml status
cargo run -p opentk-db --bin opentk-sync -- --config ./opentk.toml run
```
