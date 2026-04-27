## Plan: Remove bs text-assert tests

Task: `.ralph/tasks/bugs/bug-remove-bs-text-assert-tests.md`

Plan path: `.ralph/tasks/bugs/bug-remove-bs-text-assert-tests_plans/remove-bs-text-assert-tests-plan.md`

### Current State

- The active bug task has no existing `TO BE VERIFIED` or `NOW EXECUTE` marker, so this plan is the required planning artifact before implementation.
- Required skills were read:
  - `$tdd`: use vertical red/green steps, test behavior through public interfaces, avoid implementation-coupled assertions.
  - `$improve-code-boundaries`: remove boundary mud aggressively; prefer deleting misplaced text/source contract tests rather than preserving or wrapping them.
- The named bad tests are present in:
  - `crates/opentk-config/tests/config_loading.rs`
  - `crates/opentk-config/tests/production_source_contract.rs`
  - `crates/opentk-search/tests/documentation.rs`
  - `crates/opentk-search-eval/tests/documentation.rs`
- The repo sweep found additional candidates to inspect, but most are behavior tests that merely use text fixtures or inspect protocol payloads. Those should stay when they exercise runtime behavior through code.

### Interface / Design

- No production interface, type, enum, or schema change is planned.
- The intended boundary cleanup is in the test suite:
  - Keep tests for `Config` and `ConfigLoader` behavior in `opentk-config`.
  - Remove tests that make `opentk-config` own Docker Compose, Dockerfile, GitHub Actions, shell script, docs, README, manifest, or source-code contracts.
  - Remove whole documentation-only test files when their only purpose is prose coverage.
  - Preserve fixture-driven parser, extractor, API, DB writer, and search behavior tests where the fixture is input/output evidence for real code behavior.

### TDD Execution

Because the bug is "bad tests exist", adding a permanent test that scans test source for forbidden strings would be another brittle source-text test. That would directly violate this task's TDD exception and the bug's goal. Use a temporary red/green verification loop instead:

1. RED tracer: run one listed offending test by exact name before deletion, for example:
   - `cargo test -p opentk-config dockerfiles_declare_runtime_healthchecks`
   - Expected before fix: it exists and passes, proving the bug is present.
2. GREEN slice: delete that test and its now-unused helpers/imports if any, then rerun the exact command.
   - Expected after fix: Cargo reports no matching test in that package, while the remaining package tests still compile.
3. Repeat in vertical slices for each cluster rather than deleting everything blind:
   - Docker Compose / Dockerfile / docs / scratch / workflow / init script text-contract tests in `config_loading.rs`.
   - Source and manifest substring scanner tests in `production_source_contract.rs`.
   - Documentation prose coverage tests in `opentk-search/tests/documentation.rs` and `opentk-search-eval/tests/documentation.rs`.
4. After each cluster, run a focused package test command to catch compile errors from unused helpers/imports:
   - `cargo test -p opentk-config --test config_loading`
   - `cargo test -p opentk-config --test production_source_contract` only until the file is removed; if the file becomes empty, remove it and rely on package tests.
   - `cargo test -p opentk-search`
   - `cargo test -p opentk-search-eval`

### Removal Scope

Remove from `crates/opentk-config/tests/config_loading.rs`:

- `docker_compose_declares_local_stack_contract`
- `dockerfiles_declare_runtime_healthchecks`
- `docker_compose_docs_explain_polling_and_cdc_boundaries`
- `operations_docs_explain_healthchecks_and_shutdown`
- `scratch_docker_build_contract_uses_prebuilt_multi_arch_artifacts`
- `github_docker_workflow_declares_trigger_contract`
- `github_docker_workflow_splits_build_from_publish_auth`
- `github_docker_workflow_builds_scratch_images_from_repository_dockerfiles`
- `github_docker_workflow_uses_native_cross_compilation_without_qemu`
- `github_docker_workflow_caches_cargo_targets_layers_and_final_assembly`
- `github_docker_workflow_publishes_oci_artifacts_with_main_sha_and_release_tags`
- `postgres_init_script_runs_migrations_without_ignoring_errors`
- Helpers only used by those deleted tests:
  - `assert_scratch_dockerfile_contract`
  - `assert_scratch_artifact_dockerfile_contract`
  - `assert_scratch_build_script_contract`
  - `assert_scratch_docs_contract`
  - `docker_workflow`
  - `service`
  - `scalar`
  - `path_mapping`
  - `string_sequence`
  - `assert_sequence_contains`
  - `depends_condition`
  - `job_contains`

Preserve in `config_loading.rs`:

- `required_database_url_loads_with_compiled_defaults`
- `explicit_path_loads_file_and_missing_explicit_path_is_clear_error`
- `invalid_config_reports_field_context`
- `explicit_values_override_defaults`
- `redacted_config_masks_secrets_and_keeps_operational_settings`
- `compose_config_uses_service_hostnames_and_explicit_sync_scope`, unless execution reveals it is merely static TOML contract coverage with no meaningful `ConfigLoader` behavior remaining. If changed, keep the behavioral `ConfigLoader::with_path(...).load()` assertions and remove only the direct TOML category text inspection.

Remove `crates/opentk-config/tests/production_source_contract.rs` entirely unless a behavioral test remains after deletion. The listed tests are all source/manifest substring scanners.

Remove `crates/opentk-search/tests/documentation.rs` and `crates/opentk-search-eval/tests/documentation.rs` entirely. They are documentation prose coverage tests.

### Borderline Sweep Decisions

- Preserve `crates/opentk-db/tests/postgres_schema.rs::checked_in_migrations_match_schema_spec` for this task unless execution shows it is redundant with a migration-application behavior test. It compares generated schema output to checked-in migrations, which is drift detection for generated artifacts rather than docs/source prose scanning.
- Preserve `crates/opentk-core/tests/workspace_smoke.rs::documented_opentk_sync_cargo_command_links_and_prints_help` but weaken the brittle help prose assertion if needed. It executes `cargo run` and validates the public CLI exists. Prefer asserting only status success and maybe `--help` exits successfully, not exact help copy.
- Preserve `crates/opentk-sync/tests/document_content_extractor.rs` golden fixture tests. They exercise extractor behavior against real fixture inputs.
- Preserve `crates/opentk-api/tests/cases/deep_verify_http_api.rs` document-content fixture assertions. They verify API behavior over extracted content.
- Preserve `crates/opentk-sync/tests/payload_parser.rs` and `crates/opentk-db/tests/sync_writer.rs` XML fixture tests. They exercise parser/writer behavior.
- During execution, run the repo-wide sweep from the bug description and inspect any remaining direct reads of `docker/`, `.github/`, `docs/`, `README.md`, `Cargo.toml`, `scripts/`, or production `src/` from tests.

### Boundary Cleanup

Use `$improve-code-boundaries` during implementation by deleting the misplaced ownership boundary rather than abstracting it:

- `opentk-config` should test config parsing/loading/redaction, not Docker, CI, docs, shell scripts, manifests, or production source spelling.
- Documentation tests should not exist as a separate crate-level contract.
- Avoid replacing removed tests with equivalent static scanners in another file.
- Remove now-unused helpers/imports/files completely.

### Verification

Run, in order:

1. Focused red/green commands for deleted test clusters as described above.
2. `rg -n "fs::read_to_string|include_str!|\\.contains\\(|starts_with\\(|ends_with\\(|serde_yaml::to_string" crates/*/tests -g '*.rs'` and manually classify remaining hits.
3. `make check`
4. `make test`
5. `make lint`

Do not run `make test-long`; this is not a story-finishing task and the bug does not require the long/e2e lane.

### Completion

- Tick the bug acceptance criteria only after the sweep and required checks pass.
- Set `<passes>true</passes>` in `.ralph/tasks/bugs/bug-remove-bs-text-assert-tests.md`.
- Run `/bin/bash .ralph/task_switch.sh`.
- Add all files, including `.ralph` files.
- Commit with `task finished bug-remove-bs-text-assert-tests: remove brittle text contract tests` and include verification evidence in the commit body.
- Push.

NOW EXECUTE
