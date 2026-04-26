# Plan: Rename complete-sync Binary to opentk-sync

## Current Read

The repository already appears to have the requested rename in production code:

- `crates/opentk-db/Cargo.toml` exposes `[[bin]] name = "opentk-sync"` with `path = "src/bin/opentk-sync.rs"`.
- `crates/opentk-db/src/bin/opentk-sync.rs` exists and uses `#[command(name = "opentk-sync")]`.
- `crates/opentk-db/src/bin/complete-sync.rs` does not appear in the bin directory.
- README command examples already use `--bin opentk-sync`.
- `crates/opentk-config/tests/production_source_contract.rs` already has a public repository contract test for the binary rename.

The execution step should therefore validate the current state, fix any remaining references or missing checks, and mark the task complete only after the required verification gates pass.

## TDD Plan

Use the `tdd` skill as the execution mindset:

- Public interface: the cargo binary exposed by `opentk-db` must be `opentk-sync`; the old `complete-sync` public binary must be absent.
- Behavior to test: users can discover/run the new binary via cargo help, docs point to the new binary, and the old source/manifest name is removed.
- Existing contract coverage: keep or improve `production_source_contract::sync_binary_is_named_opentk_sync_not_complete_sync` as the focused public contract for repository state.
- If that contract is missing or too weak during execution, first make the focused contract fail for the missing behavior, then update code/docs to pass it.
- Do not add broad implementation-coupled tests for internal parsing or command enum shape; the rename is a public binary/docs contract.

Execution loop:

1. Run the focused contract test for the rename.
2. If it fails, make the smallest rename/reference fix needed.
3. Run `cargo run -p opentk-db --bin opentk-sync -- --help`.
4. Confirm `cargo run -p opentk-db --bin complete-sync -- --help` fails because the old binary no longer exists.
5. Run `rg -n "complete-sync" README.md docs Makefile Cargo.toml crates --glob '!target/**'` and decide whether remaining occurrences are legitimate historical doc filenames/test messages or must be renamed.
6. Run `make check`, `make test`, and `make lint`.

## Improve Code Boundaries Plan

Use the `improve-code-boundaries` skill during final review:

- Keep the CLI boundary as the binary file `opentk-sync.rs`; do not introduce wrapper aliases or compatibility shims for `complete-sync`.
- Avoid duplicate command names split across docs/tests/manifests; the manifest path, source filename, clap command name, and README examples should agree on `opentk-sync`.
- Remove any remaining obsolete alias source, manifest bin entry, or Makefile target if found.
- Do not add compatibility behavior. This greenfield repo allows breaking the old binary name.

## Acceptance Mapping

- `cargo run -p opentk-db --bin opentk-sync -- --help` works: verify directly.
- Old `complete-sync` binary name no longer exists: verify via focused contract and failed cargo run against old name.
- README/docs updated: verify via repository search and update any stale command references.
- All checks pass: run `make check`, `make test`, and `make lint`; do not run `make test-long` unless story-level instructions change.

NOW EXECUTE
