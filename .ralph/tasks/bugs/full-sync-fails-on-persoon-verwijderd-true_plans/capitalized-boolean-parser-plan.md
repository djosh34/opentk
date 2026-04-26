# Plan: Capitalized Boolean Parser

Task: `.ralph/tasks/bugs/full-sync-fails-on-persoon-verwijderd-true.md`

## Public behavior

- A live SyncFeed payload for `Persoon` with `verwijderd="True"` must parse successfully through `parse_entity_xml`.
- The parsed entity must preserve the tombstone state as `deleted == true`, not treat the value as text or silently default it.
- Existing invalid boolean values must still fail with an explicit `PayloadParseError::InvalidValue`; no error may be swallowed or converted to a default.

## Boundary design

- Keep `crates/opentk-sync/src/payload.rs` as the parsing boundary; this is where XML lexical values are converted into typed `ParsedValue` and `ParsedEntity` fields.
- Keep the public test surface at `parse_entity_xml(category, xml)`, not private helper tests. This follows the `tdd` skill: verify caller-visible parser behavior, not helper implementation.
- Use the existing single `parse_bool` function for both `verwijderd` base attributes and `xs:boolean` / `booleanType` scalar elements. This avoids adding duplicate boolean conversion paths.
- Apply `improve-code-boundaries` by resisting new wrappers, DTOs, or category-specific fixes. The boundary smell to avoid is wrong-place/stringly parsing: the lexical compatibility rule belongs in the shared boolean parser, not in `Persoon`-specific code or the sync runner.
- Keep the parser strict outside the observed compatibility rule. The plan is to accept capitalized boolean spellings seen in the live feed while continuing to reject unrelated values such as `sometimes`.

## TDD execution

Use vertical Red-Green only.

1. Red: add one integration-style parser test in `crates/opentk-sync/tests/payload_parser.rs`.
   - Name it for the behavior, for example `live_persoon_capitalized_deleted_boolean_parses`.
   - Build a minimal `Persoon` XML payload with a valid UUID, `verwijderd="True"`, and valid `bijgewerkt`.
   - Call `parse_entity_xml("Persoon", xml)`.
   - Assert `parsed.category == "Persoon"` and `parsed.deleted` is true.
   - Run the focused test and confirm it fails on the current invalid `xs:boolean` value.
2. Green: update `parse_bool` in `crates/opentk-sync/src/payload.rs`.
   - Accept `"True"` as `true`.
   - Consider accepting `"False"` as `false` at the same boundary for symmetry if the test/code review makes that interface clearer; do not add category-specific logic.
   - Update the invalid boolean message only if the accepted set changes and the old wording becomes misleading.
   - Run the focused parser test until it passes.
3. Manual verification:
   - Rerun a narrow parse check using a minimal `Persoon` payload with `verwijderd="True"` through the test suite or an equivalent focused cargo test.
   - Search the parser for any other boolean conversion path. If another live path still rejects the same lexical value, add one new Red test for that public behavior before fixing it.
4. Refactor/boundary review:
   - Confirm the fix did not add a new abstraction, category special case, or duplicate parser.
   - Confirm invalid booleans still produce explicit errors and are not ignored/defaulted.
   - If a broader type/interface change becomes necessary, switch this plan back to `TO BE VERIFIED` and stop.
5. Final verification:
   - Run `make check`.
   - Run `make test`.
   - Run `make lint`.
   - Do not run `make test-long`; this bug is not a story-finishing task and the task does not explicitly require the long/e2e lane.
6. Completion:
   - Tick the task acceptance criteria only after the relevant evidence exists.
   - Set `<passes>true</passes>` in the task only after all required checks pass.
   - Run `/bin/bash .ralph/task_switch.sh`.
   - `git add` all files, including `.ralph` changes.
   - Commit with `task finished full-sync-fails-on-persoon-verwijderd-true: ...` and include test/check evidence plus any implementation notes.
   - Push the commit.

NOW EXECUTE
