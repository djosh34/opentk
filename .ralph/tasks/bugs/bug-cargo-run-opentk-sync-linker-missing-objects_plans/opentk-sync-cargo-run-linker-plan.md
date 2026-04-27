# Plan: Make opentk-sync Cargo Invocation Link Reliably

## Context

- The reported failure happened before application startup while running:
  `cargo run -p opentk-db --bin opentk-sync -- --help`.
- The linker searched for missing `target/debug/deps/opentk_sync-*.rcgu.o` files.
  This points at a build artifact boundary problem, not a SyncFeed, database, or
  CLI parsing behavior.
- The normal `make` lanes already set `CARGO_INCREMENTAL=0`, and the test wrapper
  sets `CARGO_BUILD_JOBS=1`. Direct `cargo run` from README/task instructions does
  not currently have a regression test.
- The repo exposes the operational binary from `opentk-db`, while reusable sync
  logic lives in the `opentk-sync` crate. Keep that boundary: do not move runtime
  sync logic to solve a build artifact issue.

## Public Interface

- The public interface under test is the documented Cargo command:
  `cargo run -p opentk-db --bin opentk-sync -- --help`.
- The command must build, link, exit successfully, and print clap help without
  requiring config, PostgreSQL, SyncFeed access, or migrations.

## TDD Execution Plan

- [x] RED 1: Add one focused integration test that spawns the documented Cargo
  command with an isolated `CARGO_TARGET_DIR` under `target/` and `--help`.
  Assert success and that stdout contains `Run or inspect durable
  Tweede Kamer SyncFeed ingestion`.
  - Put this in a workspace-level public-interface test, likely
    `crates/opentk-core/tests/workspace_smoke.rs`, because the behavior is a
    workspace/binary packaging contract rather than database logic.
  - Run only the narrow test first and confirm it fails before any fix if the
    bug is reproducible or if the test exposes a missing build-interface contract.
- [x] GREEN 1: Make the narrow test pass with the smallest build-boundary fix.
  Candidate fixes, in order:
  - Prefer repository build configuration that applies to direct `cargo run`, such
    as profile-level incremental disabling if artifact corruption is tied to
    incremental compilation.
  - If the binary packaging is the actual problem, fix `opentk-db` manifest/source
    layout so the `opentk-sync` binary target is unambiguous and does not duplicate
    crate/package identity accidentally.
  - Do not add retries, swallowed linker errors, or runtime code that hides build
    failures.
- [x] Manual verification 1: Run the originally failing command exactly:
  `cargo run -p opentk-db --bin opentk-sync -- --help`.
  If it still fails, keep the evidence, add the next RED test for the newly observed
  build-system behavior, and continue one red/green slice at a time.
- [x] Refactor/boundary review with `improve-code-boundaries`:
  - Keep the regression at the build/CLI packaging boundary.
  - Remove any duplicate, legacy, or misleading binary/docs references found during
    the fix.
  - Avoid muddying sync runtime modules with build artifact concerns.
- [x] Broad checks:
  - `make check`
  - `make test`
  - `make lint`
  - Do not run `make test-long`; this bug is not a story-ending validation gate and
    does not require the ultra-long/e2e lane.
- [ ] Completion:
  - Mark the bug task acceptance criteria and `<passes>true</passes>` only after
    all required checks pass.
  - Run `/bin/bash .ralph/task_switch.sh`.
  - Stage all files, including `.ralph` changes, commit with
    `task finished bug-cargo-run-opentk-sync-linker-missing-objects: ...`, and push.

NOW EXECUTE
