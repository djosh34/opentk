# Deleted Body Tombstone Plan

## Task

Fix the full-sync blocker where a live `PersoonNevenfunctie` delete marker contains scalar or relation body content and the strict payload parser rejects it with `DeletedEntityHasBody`.

The user-facing behavior is the existing public parser/runner path:

- `parse_entity_xml(category, xml)` must parse a deleted SyncFeed entity as a tombstone.
- The durable sync runner must be able to continue when the live feed sends a deleted entity with body content.
- Deleted entities must still carry source metadata: category, XML element, source id, deleted flag, and source updated timestamp.
- Deleted entities must not persist stale scalar or relation body content into category or relation tables.

## Current Boundary

The rejection lives in `crates/opentk-sync/src/payload.rs` after normal child parsing:

- children are parsed into `ParsedEntity.scalars` and `ParsedEntity.relations`
- then deleted entities with any parsed body are rejected as `PayloadParseError::DeletedEntityHasBody`

That error is too strict for the live SyncFeed. The parser boundary should normalize delete markers into tombstones. The database writer already treats `entity.deleted` as a delete path and removes category/relation/body rows, so the parser should not surface an error solely because the upstream delete marker contains stale body content.

## TDD Plan

Use vertical Red-Green cycles. Do not write all tests first.

- [x] Red 1: Change the existing public parser test in `crates/opentk-sync/tests/payload_parser.rs` so a deleted entity with body content parses successfully as a tombstone.
  - Behavior: `parse_entity_xml("Document", deleted XML with `<documentNummer>...`)` returns `ParsedEntity`.
  - Assertions: `deleted == true`, `source_id` is preserved, `scalars` is empty, `relations` is empty.
  - Expected initial result: the test fails with `DeletedEntityHasBody`.
- [x] Green 1: Update `crates/opentk-sync/src/payload.rs` so deleted roots consume/validate XML structure enough to reject malformed XML, but do not retain or reject scalar/relation body content.
  - Prefer the smallest clear change: once base attributes show `deleted=true`, read through the root body without calling `parse_child` / `parse_empty_child`.
  - Preserve existing parsing for non-deleted entities.
  - Preserve strict root, required attribute, UUID, timestamp, and XML well-formedness errors.
- [x] Red 2, only if needed after manual verification: not needed; manual verification showed parser normalization cleared the original failure and exposed a separate store deadlock.
- [x] Green 2, only if Red 2 is needed: not needed; filed `.ralph/tasks/bugs/full-sync-deadlocks-writing-persoon-nevenfunctie.md` for the separate store failure.

## Manual Verification

- [x] Run the focused parser test with the exact test name.
- [x] Re-run the previously failing full sync command or a narrow reproduction using the captured category/source shape if the full live command is practical within the task.
- [x] If verification exposes another body shape that still fails, create the next Red test first and continue.

## Boundary Review

- [x] Remove or keep `DeletedEntityHasBody` based on use after the fix. If it becomes dead, delete the enum variant instead of keeping legacy error surface.
- [x] Avoid adding a compatibility mode or category-specific exception. Deleted tombstone parsing should be category-agnostic.
- [x] Do not swallow XML errors. Malformed XML and invalid required delete metadata must still fail.
- [x] Do not add application code outside the parser/writer boundary unless a Red test proves it is needed.

## Final Checks

- [x] `make check`
- [x] `make test`
- [x] `make lint`
- [x] Do not run `make test-long`; this bug does not require the long lane unless the task is promoted to story-end validation.
- [x] Final improve-code-boundaries pass: confirm the result is simpler than before, with no stale error variant, no category-specific branch, and no duplicate tombstone shape.

NOW EXECUTE
