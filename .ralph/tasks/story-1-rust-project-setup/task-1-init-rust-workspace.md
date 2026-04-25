## Task: Story 1 Task 1 - Initialize Rust Workspace <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Initialize this repository as a Rust workspace for the OpenTK rebuild. The workspace must support a PostgreSQL-backed SyncFeed importer, a future HTTP API, and strict test/lint workflows. Create the base crate structure, dependency strategy, formatting configuration, lint configuration, and project documentation needed for all later stories.

The project decisions already made in this repo are: Rust, `sqlx`, `tokio`, `reqwest`, `quick-xml`, `axum` later for HTTP, PostgreSQL as the primary database, SyncFeed XML as the source, direct apply of fetched pages, category cursors advanced transactionally, binary document assets linked by URL and metadata, HTML assets stored in the database.

In scope: create `Cargo.toml` workspace, crates/modules for importer/core/database/API boundaries, Rust formatting/lint config, initial README setup notes, and an empty migration location. Keep this task focused on setup; importer behavior and HTTP endpoints are separate stories.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add at least one workspace smoke test proving the workspace builds and a core crate function is callable.
- [x] Workspace contains a clear crate/module layout for core domain types, SyncFeed ingestion, database access, and future HTTP API.
- [x] `cargo fmt --check` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] `cargo test --workspace` passes.
- [x] README or developer docs explain the workspace layout and required local tools.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] Not applicable: this task did not add or move ultra-long/e2e tests. `make test-long` was intentionally not run because this is not a story-ending task and the task does not explicitly require it.
</acceptance_criteria>

<plan>
.ralph/tasks/story-1-rust-project-setup/task-1-init-rust-workspace_plans/init-rust-workspace-plan.md
</plan>

NOW EXECUTE
