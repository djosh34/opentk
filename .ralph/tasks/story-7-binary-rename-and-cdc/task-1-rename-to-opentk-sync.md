## Task: Story 7 Task 1 - Rename complete-sync Binary to opentk-sync <status>not_started</status> <passes>false</passes>

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
- [ ] `cargo run -p opentk-db --bin opentk-sync -- --help` works
- [ ] Old `complete-sync` binary name no longer exists
- [ ] README and docs updated
- [ ] All tests pass
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
