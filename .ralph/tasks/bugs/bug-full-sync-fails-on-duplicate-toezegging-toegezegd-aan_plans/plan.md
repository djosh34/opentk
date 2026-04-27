# Plan: duplicate Toezegging toegezegdAan

## Context

Story 10 Task 1 failed during a clean full sync with:

`DuplicateSingleField { category: "Toezegging", field: "toegezegdAan" }`

The parser rejects the second live `toegezegdAan` element because `next_ordinal`
enforces `Field.max_occurs` from `opentk_core::official_schema`, and the current
documented live drift for `Toezegging.toegezegdAan` models the relation as
`Occurs::Exactly(1)`.

The pinned XSD does not list `toegezegdAan` at all; it lists
`toegezegdAanFractie` and `toegezegdAanPersoon`. A previous bug already modeled
the live `toegezegdAan` relation as documented feed drift, but the 20260427
full-sync run proves that live payloads may include more than one
`toegezegdAan` relation for a single `Toezegging`.

## TDD workflow

- [x] RED: Add one public parser behavior test to
      `crates/opentk-sync/tests/payload_parser.rs` proving a live `Toezegging`
      payload with two `<toegezegdAan ref="..."/>` relation elements parses
      successfully and retains both relation targets with ordinals `1` and `2`.
- [x] Confirm the focused test fails with the current
      `DuplicateSingleField { category: "Toezegging", field: "toegezegdAan" }`
      error.
- [x] GREEN: Change only the schema/model boundary needed to make that test
      pass.
- [x] Run the focused parser test after the fix.
- [x] Manually verify the original bug shape through the public
      `parse_entity_xml("Toezegging", xml)` interface using the duplicate
      `toegezegdAan` payload. If the manual verification still fails, add the
      next RED test before changing production code again.

## Interface and boundary design

- Public interface stays unchanged: `parse_entity_xml(category, xml)` and
  `parse_entry_payload(entry)` return `ParsedEntity`.
- `ParsedEntity.relations` already supports repeated relation fields with
  ordinals, so no new DTO or API type is needed.
- Do not special-case `toegezegdAan` inside `payload.rs`; the parser should keep
  enforcing the schema model generically.
- Update the repository-owned schema fact for live
  `Toezegging.toegezegdAan` from `Occurs::Exactly(1)` to
  `Occurs::Unbounded`.
- The cleanest likely change is in
  `crates/opentk-sync/src/official_schema.rs`:
  `TASK_DOCUMENTED_LIVE_FIELD_DRIFTS` should document that live
  `toegezegdAan` is a repeatable relation. If the generated core schema already
  contains `toegezegdAan`, update its `max_occurs` there too; if it is derived
  from the documented drift, keep the fix at the drift boundary.
- Keep `toegezegdAanFractie` and `toegezegdAanPersoon` unless a separate Red
  test or official source comparison proves they are dead metadata.

## improve-code-boundaries check

- Boundary smell to avoid: wrong-placeism. The parser must not accumulate
  category-specific live-feed exceptions.
- Boundary cleanup to preserve: documented live drifts belong beside
  official-source comparison and checked-in schema metadata, not in runtime parse
  branches.
- Avoid adding a compatibility mode, skip list, swallowed duplicate error, or
  ad hoc string check. The existing `DuplicateSingleField` error remains useful
  for truly single-occurrence fields.
- If implementation reveals that `toegezegdAan` and
  `kamerbriefNakoming` drift handling is duplicated between generated model and
  source comparison, flatten that duplication at the schema boundary rather than
  adding another conversion layer.

## Verification

- [x] Focused payload parser test fails RED, then passes GREEN.
- [x] Run `cargo test -p opentk-sync --test official_schema_sources` if schema
      metadata or source verification changes.
- [x] Re-run the captured full-sync command from the bug report if practical:
      `timeout 180s env CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260427-030235.toml run`
- [x] If the rerun progresses past duplicate `toegezegdAan` but exposes a new
      unrelated sync failure, create a separate bug task instead of broadening
      this parser fix.
- [x] `make check` passes cleanly.
- [x] `make test` passes cleanly.
- [x] `make lint` passes cleanly.
- [x] Do not run `make test-long`; this is a normal bug task unless execution
      proves it has become a story-end validation gate.
- [x] Final improve-code-boundaries review confirms live drift remains modeled
      at the schema boundary and parser code is not muddier.

NOW EXECUTE
