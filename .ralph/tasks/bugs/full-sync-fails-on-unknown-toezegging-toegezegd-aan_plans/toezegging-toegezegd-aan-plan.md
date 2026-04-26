# Toezegging ToegezegdAan Parser Plan

## Task

Fix the full-sync blocker where the live `Toezegging` SyncFeed payload contains
`toegezegdAan`, but the repository-owned schema metadata only knows the pinned
XSD fields `toegezegdAanFractie` and `toegezegdAanPersoon`.

The user-facing behavior is the existing public parser/runner path:

- `parse_entity_xml("Toezegging", xml)` must parse the live field instead of
  surfacing `PayloadParseError::UnknownField`.
- The durable sync runner must be able to continue past the captured
  `Toezegging.toegezegdAan is unknown` parse error.
- The parser must still reject genuinely unknown fields for other payloads.

## Current Boundary

The rejection lives in `crates/opentk-sync/src/payload.rs`:

- `parse_child` and `parse_empty_child` call `EntityType::field_named`.
- `Toezegging` metadata in
  `crates/opentk-core/src/official_schema/generated.rs` does not include
  `toegezegdAan`.
- The checked-in model currently matches the vendored pinned XSD in
  `crates/opentk-sync/official_sources/tweedekamer/xsd/tkData-v1-0-toezegging.xsd`,
  so blindly editing the generated model will also require an explicit source
  gap or vendored source update, not a silent mismatch.

## Interface Design

Keep the public interface unchanged:

- Continue using `parse_entity_xml` / `parse_entry_payload` as the behavior
  boundary.
- Continue emitting `ParsedRelation` rows for modeled relation fields.
- Do not add a compatibility flag, category-specific parser mode, or swallowed
  unknown-field path.

Preferred implementation shape:

- First capture the live field as a public parser regression test.
- Then decide from the Red failure whether the right model is an explicit
  `Toezegging` relation field in the schema metadata or a narrow schema-source
  override that documents the live feed drift from the pinned XSD.
- If the implementation adds a metadata override, keep it in the schema
  boundary, not in `payload.rs`; the parser should still just ask the entity
  model for the field.

## TDD Plan

Use vertical Red-Green cycles. Do not write all tests first.

- [x] Red 1: Add one public parser test in
  `crates/opentk-sync/tests/payload_parser.rs` for a minimal live
  `Toezegging` payload containing `<toegezegdAan ref="..."/>`.
  - Behavior: `parse_entity_xml("Toezegging", xml)` returns `ParsedEntity`.
  - Assertions: the entity category is `Toezegging`, the relation named
    `toegezegdAan` exists, the target id is preserved, and the scalar body still
    parses.
  - Expected initial result: the focused test fails with
    `PayloadParseError::UnknownField { category: "Toezegging", field:
    "toegezegdAan" }`.
- [x] Green 1: Make that one test pass by fixing the schema boundary.
  - Prefer adding the live relation to the repository-owned schema metadata in
    a way that keeps `payload.rs` generic.
  - If `all_vendored_official_entities_have_matching_model_entries` fails,
    update the schema-source verification to document this live-feed drift
    explicitly instead of ignoring the mismatch.
  - Do not remove `toegezegdAanFractie` or `toegezegdAanPersoon` unless a Red
    test or official source proves they are legacy.
- [x] Red 2, only if manual verification still fails with a related
  `Toezegging` parse error: add the next narrow public parser test for that
  exact live shape.
- [x] Green 2, only if Red 2 is needed: fix only the behavior captured by the
  new Red test.

## Manual Verification

- [x] Run the focused parser test by exact name after each Red/Green cycle.
- [x] Run `cargo test -p opentk-sync --test official_schema_sources` if schema
  metadata or source verification changes.
- [x] Re-run the captured full-sync command from the bug report against the same
  config/database if practical:
  `timeout 180s env CARGO_INCREMENTAL=0 cargo run -p opentk-db --bin opentk-sync -- --config .ralph/reports/full-sync-config-20260426-205545.toml run`
- [x] If the rerun progresses past the `toegezegdAan` parse error but exposes a
  new unrelated sync failure, file a separate bug task and do not broaden this
  parser fix.

## Boundary Review

- [x] Final improve-code-boundaries pass: the schema knowledge must live in the
  schema/model boundary, not as a hard-coded parser exception.
- [x] Avoid string-soup rendering changes; this bug is about modeled field
  lookup, not user-facing formatting.
- [x] Do not swallow unknown fields globally. Unknown fields should still report
  a typed `UnknownField` error unless deliberately modeled.
- [x] Keep the TDD test behavior-focused and public-interface based; do not test
  private helper functions.

## Final Checks

- [x] `make check`
- [x] `make test`
- [x] `make lint`
- [x] Do not run `make test-long`; this is a normal bug task unless manual
  verification proves it must become a story-end validation gate.

NOW EXECUTE
