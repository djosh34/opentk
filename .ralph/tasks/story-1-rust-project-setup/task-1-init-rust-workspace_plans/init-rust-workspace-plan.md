# Plan: Initialize Rust Workspace

## Task

Task file: `.ralph/tasks/story-1-rust-project-setup/task-1-init-rust-workspace.md`

Goal: create the initial Rust workspace for the OpenTK rebuild with strict formatting, linting, testing, documentation, and a crate layout that cleanly separates core domain, SyncFeed ingestion, database access, and future HTTP API responsibilities.

## Public Interface Design

Use a workspace with four library crates:

- `crates/opentk-core`: owns source-neutral domain and shared value types. This crate exposes the first stable public smoke-test interface.
- `crates/opentk-sync`: owns SyncFeed ingestion boundaries. It may depend on `opentk-core`.
- `crates/opentk-db`: owns PostgreSQL/sqlx database boundaries and migrations. It may depend on `opentk-core`.
- `crates/opentk-api`: owns future HTTP API boundaries. It may depend on `opentk-core`.

The first callable public interface should be intentionally small:

```rust
pub fn workspace_name() -> &'static str
```

in `opentk_core`.

This keeps the smoke test behavior-focused: callers can link the workspace and call a public core API. It avoids inventing premature domain objects before Story 2 designs the official information model.

## Dependency Strategy

Define common dependency versions in `[workspace.dependencies]` at the root:

- `tokio`
- `reqwest`
- `quick-xml`
- `sqlx` with PostgreSQL/runtime/migration support
- `axum`
- `serde`
- `serde_json`
- `thiserror`
- `tracing`
- `tracing-subscriber`
- `clap`

Crates should only opt into dependencies they need immediately. For this setup task, keep implementation minimal while making the chosen stack available consistently for later tasks.

## TDD Plan

Follow vertical red-green cycles. Do not write all tests first.

- [x] RED 1: create one integration-style smoke test that imports `opentk_core::workspace_name()` and expects `"opentk"`. Run `cargo test --workspace` and confirm it fails because the workspace/crate does not exist yet or the function is missing.
- [x] GREEN 1: create the root workspace, `opentk-core` crate, and minimal implementation so the smoke test passes.
- [x] RED 2: add the developer-command expectation by creating `Makefile` targets and running `make check` or `make lint` before all required files/config are complete, confirming the command path reports the missing setup.
- [x] GREEN 2: add `Makefile`, `rustfmt.toml`, Clippy lint configuration, and any minimal crate files needed so `make check`, `make lint`, and `make test` run the required commands.
- [x] GREEN 3: add placeholder crate boundaries for `opentk-sync`, `opentk-db`, and `opentk-api`, each with a minimal library module and crate-level docs describing ownership. Keep placeholders behavior-free unless a compile target needs a tiny public marker.
- [x] GREEN 4: add `migrations/.gitkeep` or equivalent empty migration location for sqlx migrations.
- [x] GREEN 5: add `README.md` documenting workspace layout, required local tools, and the exact default commands.

## Code Boundary Plan

Use the `improve-code-boundaries` skill before implementation and again before completion.

Boundary choices:

- No root crate or umbrella facade. Each crate owns its own boundary.
- No duplicate DTO/domain shapes in setup. Domain types wait for Story 2.
- No stringly helper layers beyond the single `workspace_name()` smoke interface.
- No private helper sprawl in placeholder crates.
- Keep visibility as narrow as possible. Export only what a crate must expose to compile and prove the workspace.
- Avoid unused dependencies in crate manifests; centralize versions at the root and opt in per crate.

Final boundary review:

- [x] Check whether any crate/module exists only as ceremony and can be merged or removed without weakening the accepted task layout.
- [x] Check whether any public item exists only for tests. If yes, remove it or make it a real public setup contract.
- [x] Check for avoidable conversions, duplicate shapes, or premature config/schema abstractions.

## Execution Checklist

- [x] Read `tdd` skill again before coding.
- [x] Run one red test first and preserve the observed failure in progress notes.
- [x] Implement the smallest green slice.
- [x] Iterate through remaining setup slices.
- [x] Run `cargo fmt --check`.
- [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Run `cargo test --workspace`.
- [x] Run `make check`.
- [x] Run `make lint`.
- [x] Run `make test`.
- [x] Do not run `make test-long`; this is not a story-ending task and the task does not require it.
- [x] Update acceptance checkboxes and set `<passes>true</passes>` only after all required checks pass.
- [x] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Commit all task files and code with message `task finished task-1-init-rust-workspace: initialize Rust workspace`.
- [ ] Push the commit.

NOW EXECUTE
