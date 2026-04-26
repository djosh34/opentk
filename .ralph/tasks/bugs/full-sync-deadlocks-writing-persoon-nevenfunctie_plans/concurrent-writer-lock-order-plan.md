# Concurrent Writer Lock Order Plan

## Task

Fix the full-sync blocker where the durable runner reaches the write phase for
`PersoonNevenfunctie` and PostgreSQL aborts the page transaction with
`deadlock detected`.

The user-facing behavior is the existing public store/writer path:

- `PostgresSyncStore::write_page` and `write_sync_page` must allow normal full
  sync category pages to be written concurrently.
- Concurrent pages that touch overlapping `sync_entity` rows through direct
  entities and relation target placeholders must not deadlock.
- A failed SQL write must still be reported as an error; do not swallow
  database errors or retry blindly.

## Current Boundary

The write boundary lives in `crates/opentk-db/src/sync_writer.rs`.

Today `write_sync_page` interleaves these operations per entity inside one
transaction:

- upsert the source row in `sync_entity`
- delete the current category row, cascading relation/repeated-scalar rows
- upsert each relation target placeholder into `sync_entity`
- insert the concrete category row and relation rows

That mixes lock acquisition order with feed/entity order. During full sync,
different category tasks can write overlapping graph edges at the same time:
for example a `Persoon` page owns a `sync_entity` row directly while a
`PersoonNevenfunctie` page touches that same row as a relation target. If two
transactions acquire source and target `sync_entity` locks in opposite orders,
PostgreSQL can correctly detect a deadlock.

The writer should own the lock-ordering policy. The runner and parser should
continue to pass parsed pages into a single deep writer interface instead of
learning database lock details.

## Interface Design

Keep the public interface unchanged:

- `write_sync_page(pool: &PgPool, page: SyncPageWrite) -> Result<SyncPageWriteOutcome, SyncPageWriteError>`
- `PostgresSyncStore::write_page(page: PreparedSyncPage) -> SyncStoreFuture<SyncStoreWriteOutcome>`

Add private writer helpers only if needed:

- collect the set of `sync_entity` keys touched by a page, including source
  entities and relation targets
- acquire locks for those keys in a deterministic order before mutating rows
- write the existing entity/category/relation data after the ordered lock step

The likely implementation should use PostgreSQL transaction-scoped advisory
locks derived from `(source_category, source_id)`, or an equivalent explicit
row-lock scheme that works even when a relation target placeholder row does not
exist yet. The lock helper must be private to `opentk-db`; it is a database
write concern, not a payload type or runner concept.

## TDD Plan

Use vertical Red-Green cycles. Do not write all tests first.

- [x] Red 1: Add one integration test in `crates/opentk-db/tests/sync_writer.rs`
  that writes overlapping pages concurrently through the public
  `write_sync_page` function. The deterministic local reproduction uses
  reciprocal `Document` relation targets because it creates the same
  opposite `sync_entity` source/target lock order behind the shared writer
  boundary.
  - Behavior: many concurrent attempts complete successfully without
    `deadlock detected`.
  - Shape: use a migrated PostgreSQL schema, a small fixed `Persoon` XML
    payload, and a small fixed `PersoonNevenfunctie` XML payload whose
    `<persoon ref="...">` points at the same `Persoon` source id.
  - Make the test deterministic enough to fail on the current lock-order bug:
    run both page writes through separate pool connections in a loop or with a
    synchronized start barrier, and assert every joined result is `Ok`.
  - Expected initial result: current writer can fail with a SQL deadlock or the
    test demonstrates that the local reproduction needs a tighter concurrent
    shape before implementation proceeds.
- [x] Green 1: Update `crates/opentk-db/src/sync_writer.rs` so each page
  acquires all touched `sync_entity` locks in a canonical order before any
  source upsert, target upsert, delete cascade, or relation insert.
  - Sort by `(source_category, source_id)`.
  - Deduplicate keys.
  - Keep the transaction single-page atomic.
  - Do not add retries that hide write failures.
  - Keep the runner/store interfaces unchanged.
- [ ] Red 2, only if manual verification still deadlocks: add the smallest next
  public writer or runner test that captures the remaining lock shape.
- [ ] Green 2, only if Red 2 exists: fix that exact behavior, then return to
  manual verification.

## Manual Verification

- [x] Run the focused writer test by exact name until it passes reliably.
- [x] Re-run the captured full-sync command from the bug report if the config
  and database are still available:
  `timeout 180s env CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-193554.toml run`
- [x] Captured config/database was available; the rerun emitted no fresh error
  before the 180s timeout, but did not complete within that window, so the
  limitation is recorded in the task notes and progress log.
- [ ] If verification exposes a different SQL failure, create the next Red test
  before changing production code.

## Boundary Review

- [x] Keep lock ordering private to the database writer boundary.
- [x] Do not leak category-specific `PersoonNevenfunctie` branches into the
  runner or parser.
- [x] Prefer removing duplicated write-shape code if lock collection reveals
  source/target key logic repeated across helpers.
- [x] Do not ignore or downgrade any SQL errors. Any remaining swallowed error
  must become a separate bug task.
- [x] Check generated schema/migrations only if the Red test proves FK layout is
  part of the deadlock. Avoid schema churn for a transaction-ordering bug.

## Final Checks

- [x] `make check`
- [x] `make test`
- [x] `make lint`
- [x] Do not run `make test-long`; this is a normal bug task unless it becomes
  story-end validation.
- [x] Final improve-code-boundaries pass: confirm the writer still has a single
  deep public interface, deterministic lock policy is local and named, and
  there is no category-specific compatibility code.

NOW EXECUTE
