## Task: Story 07 Task 01 - Rename complete-sync Binary to opentk-sync <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Rename the `complete-sync` binary to `opentk-sync` to match the workspace naming convention and clearly indicate its role as the dedicated SyncFeed-to-PostgreSQL sync worker.

Current state:
- Binary is defined in `crates/opentk-db/Cargo.toml` as `complete-sync`
- Source file is `crates/opentk-db/src/bin/complete-sync.rs`
- README and docs reference `cargo run -p opentk-db --bin complete-sync`

Changes required:
1. In `crates/opentk-db/Cargo.toml`, rename `[[bin]]` name from `complete-sync` to `opentk-sync`
2. Rename `src/bin/complete-sync.rs` to `src/bin/opentk-sync.rs`
3. Update the `#[command(name = "complete-sync")]` clap attribute to `"opentk-sync"`
4. Update README.md references
5. Update Makefile if it references the binary name
6. Update any docs that mention `complete-sync`
7. Ensure all tests still pass

No behavior changes. Pure rename.

In scope: rename binary, update all references, update tests. Out of scope: functional changes.

</description>


<acceptance_criteria>
- [x] `cargo run -p opentk-db --bin opentk-sync -- --help` works
- [x] Old `complete-sync` binary name no longer exists
- [x] README and docs updated
- [x] All tests pass
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite)
- [x] `make lint` — passes cleanly
</acceptance_criteria>

<plan>
.ralph/tasks/story-07-binary-rename-and-cdc/task-01-rename-to-opentk-sync_plans/rename-binary-plan.md
</plan>

NOW EXECUTE
