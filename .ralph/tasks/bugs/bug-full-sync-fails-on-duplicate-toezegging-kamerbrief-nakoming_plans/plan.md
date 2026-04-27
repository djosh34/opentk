# Plan: duplicate Toezegging kamerbriefNakoming

## Context

Story 010 Task 01 failed during a clean full sync with:

`DuplicateSingleField { category: "Toezegging", field: "kamerbriefNakoming" }`

The parser currently enforces `Field.max_occurs` from `opentk_core::official_schema`.
`Toezegging.kamerbriefNakoming` is modeled as `Occurs::Exactly(1)`, so the second live-feed
element fails in `next_ordinal`. The pinned XSD also says maxOccurs=1, which means the live feed
has drifted from the official source in the same general boundary as the already documented
`Toezegging.toegezegdAan` live drift.

## TDD workflow

- [x] RED: Add one behavior test to `crates/opentk-sync/tests/payload_parser.rs` proving a live
      `Toezegging` payload with two `kamerbriefNakoming` elements parses successfully and retains
      both scalar values with ordinals `1` and `2`.
- [x] Confirm the test fails with the current `DuplicateSingleField` error.
- [x] GREEN: Change only the schema/model boundary needed to make that test pass.
- [x] Run the focused test after the fix.
- [x] Manually verify the original bug shape by parsing the same duplicate-field payload through the
      public `parse_entity_xml` interface. If the manual verification still fails, add the next RED
      test before changing production code again.

## Interface and boundary design

- Public interface stays unchanged: `parse_entity_xml(category, xml)` returns a `ParsedEntity` whose
  scalar fields can contain repeated values with ordinals when the schema says the field is repeatable.
- Do not special-case `kamerbriefNakoming` inside `payload.rs`; that would put live schema drift inside
  the parser and muddy the boundary.
- Update the repository-owned schema fact for `Toezegging.kamerbriefNakoming` in
  `crates/opentk-core/src/official_schema/generated.rs` from `Occurs::Exactly(1)` to
  `Occurs::Unbounded`.
- Extend `TASK_DOCUMENTED_LIVE_FIELD_DRIFTS` in `crates/opentk-sync/src/official_schema.rs` with a
  documented override for `Toezegging.kamerbriefNakoming`, including the evidence that the public
  SyncFeed emits the field more than once while the pinned XSD says maxOccurs=1.
- Refactor `compare_fields` in `crates/opentk-sync/src/official_schema.rs` so documented live field
  drifts can override a same-name XSD field as well as add a missing live field. The current append-only
  drift handling is fine for `toegezegdAan`, but it is the wrong boundary for an existing XSD field whose
  occurrence metadata drifted.
- Add or update one schema-source test in `crates/opentk-sync/tests/official_schema_sources.rs` proving
  `verify_checked_in_model()` still passes while `TASK_DOCUMENTED_LIVE_FIELD_DRIFTS` includes the
  `kamerbriefNakoming` override. This test belongs to schema drift behavior, not parser behavior.

## improve-code-boundaries check

- Boundary smell to avoid: wrong-placeism. The parser should not know individual live-feed exceptions;
  it should enforce the schema model.
- Boundary cleanup to make: documented live drifts should be modeled as source/model comparison inputs
  that can add or override fields before comparison. That keeps official XSD extraction, documented live
  drift, and parser enforcement in their own layers.
- Keep the implementation simple: no new global parser exceptions, no compatibility mode, no new
  runtime configuration.

## Verification

- [x] Focused payload parser test passes.
- [x] Focused schema-source test passes.
- [x] `make check` passes cleanly.
- [x] `make test` passes cleanly.
- [x] `make lint` passes cleanly.
- [x] Do not run `make test-long` unless execution reveals this is story-end validation or the task is
      explicitly upgraded to require it.
- [x] Final improve-code-boundaries review confirms the exception remains in schema metadata and does
      not make parser code more muddy.

NOW EXECUTE
