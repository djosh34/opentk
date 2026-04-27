## Bug: Remove bs text-assert tests <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
The test suite contains overengineered bs-tests that do dumb stuff like asserting particular text strings exist in Dockerfiles, GitHub Actions workflows, shell scripts, README/docs files, Cargo manifests, and production source files. These tests do not test actual runtime behavior. They mostly lock in implementation text, prose, filenames, command snippets, and exact source-code spelling, which makes the suite brittle and gives false confidence.

Remove these tests instead of preserving them. This project is greenfield with no backwards compatibility requirement, so do not keep static text-contract tests around as legacy protection. Tests should exercise actual code/logic/behavior.

Known tests that must be removed:

- `crates/opentk-config/tests/config_loading.rs::docker_compose_declares_local_stack_contract`
  - Reads `docker-compose.yml`.
  - Asserts exact service images, port strings, volume strings, Dockerfile paths, healthcheck command arrays, dependency conditions, and command arrays.
  - This is static compose-file text/config contract testing, not behavior.

- `crates/opentk-config/tests/config_loading.rs::dockerfiles_declare_runtime_healthchecks`
  - Reads `docker/Dockerfile.api`, `docker/Dockerfile.sync`, and `docker/Dockerfile.scratch`.
  - Asserts literal Dockerfile strings such as `HEALTHCHECK CMD curl -f http://localhost:3000/health || exit 1`, `HEALTHCHECK CMD /bin/opentk-sync --config /etc/opentk/config.toml --health-check || exit 1`, and `HEALTHCHECK NONE`.
  - This is exactly the kind of Dockerfile text assert that should be removed.

- `crates/opentk-config/tests/config_loading.rs::docker_compose_docs_explain_polling_and_cdc_boundaries`
  - Reads `docs/docker-compose.md` and `README.md`.
  - Asserts docs mention exact strings like `docker compose up --build`, `opentk-sync poll`, `upstream SyncFeed-to-PostgreSQL`, `PostgreSQL-to-Meilisearch CDC`, `curl http://localhost:3000/health`, `degraded`, and `config/opentk.compose.toml`.
  - Documentation prose text asserts should not be part of the behavior test suite.

- `crates/opentk-config/tests/config_loading.rs::operations_docs_explain_healthchecks_and_shutdown`
  - Reads `docs/operations.md`.
  - Asserts docs contain exact terms such as `/health`, `degraded`, `search_sync`, `opentk-sync --health-check`, `Docker healthchecks`, `SIGTERM`, and `graceful shutdown`.
  - Remove this docs string-contains test.

- `crates/opentk-config/tests/config_loading.rs::scratch_docker_build_contract_uses_prebuilt_multi_arch_artifacts`
  - Reads `docker/Dockerfile.scratch`, `docker/Dockerfile.scratch-artifacts`, and `scripts/docker-buildx-scratch.sh`.
  - Calls helpers that assert large lists of required/forbidden literal snippets.
  - Also indirectly asserts docs and README text through `assert_scratch_docs_contract`.
  - Remove this static artifact/Docker/script/docs contract test.

- `crates/opentk-config/tests/config_loading.rs::github_docker_workflow_declares_trigger_contract`
  - Parses `.github/workflows/docker.yml`.
  - Asserts exact trigger details like branch `main`, tag pattern `v*`, and `workflow_dispatch`.
  - This is static GitHub workflow shape testing.

- `crates/opentk-config/tests/config_loading.rs::github_docker_workflow_splits_build_from_publish_auth`
  - Parses `.github/workflows/docker.yml`.
  - Serializes workflow job fragments back into YAML and searches strings like `ghcr.io` and `docker/login-action`.
  - Remove this string-contract workflow test.

- `crates/opentk-config/tests/config_loading.rs::github_docker_workflow_builds_scratch_images_from_repository_dockerfiles`
  - Parses `.github/workflows/docker.yml`.
  - Checks literal workflow/action/Docker command fragments such as `docker/setup-buildx-action`, `docker/Dockerfile.scratch-artifacts`, `linux/amd64,linux/arm64`, `--build-arg "BINARY=${{ matrix.binary }}"`, `actions/upload-artifact`, and absence of `for binary in`.
  - Remove this static workflow text assert test.

- `crates/opentk-config/tests/config_loading.rs::github_docker_workflow_uses_native_cross_compilation_without_qemu`
  - Reads `.github/workflows/docker.yml`, serializes it, lowercases it, and asserts it does not contain substrings like `setup-qemu`, `docker/setup-qemu-action`, `qemu`, and `emulat`.
  - Remove this brittle substring test.

- `crates/opentk-config/tests/config_loading.rs::github_docker_workflow_caches_cargo_targets_layers_and_final_assembly`
  - Reads `docker/Dockerfile.scratch-artifacts` and `.github/workflows/docker.yml`.
  - Asserts exact cache scopes, exact BuildKit mount strings, exact `cargo chef` commands, exact `COPY --from=...` fragments, exact artifact names, and absence of specific cache IDs.
  - This is heavily overfit to implementation text and should be removed.

- `crates/opentk-config/tests/config_loading.rs::github_docker_workflow_publishes_oci_artifacts_with_main_sha_and_release_tags`
  - Parses `.github/workflows/docker.yml`.
  - Asserts exact strings like `actions/download-artifact`, `apt-get install -y --no-install-recommends skopeo`, `docker://ghcr.io/${owner}/${binary}:${tag}`, `refs/heads/main`, `refs/tags/v`, and absence of `docker buildx build`.
  - Remove this static workflow publish-contract test.

- `crates/opentk-config/tests/config_loading.rs::assert_scratch_dockerfile_contract`
  - Helper used by the scratch Docker test.
  - Asserts Dockerfile literal snippets like `ARG BINARY`, `FROM scratch`, `case "${TARGETARCH}" in`, `ENTRYPOINT ["/bin/opentk"]`, and forbids text like `cargo build`, `apt-get`, `apk add`, `/bin/sh`.
  - Remove this helper with its caller.

- `crates/opentk-config/tests/config_loading.rs::assert_scratch_artifact_dockerfile_contract`
  - Helper used by the scratch Docker test.
  - Asserts exact Dockerfile implementation snippets like `FROM --platform=${BUILDPLATFORM} rust:1-bookworm AS builder`, `rustup target add ...`, exact `cargo build --release --target ...` commands, exact artifact copy paths, and absence of hard-coded output paths.
  - Remove this helper with its caller.

- `crates/opentk-config/tests/config_loading.rs::assert_scratch_build_script_contract`
  - Helper used by the scratch Docker test.
  - Asserts exact shell script snippets in `scripts/docker-buildx-scratch.sh`, including function names, command flags, platform strings, `QEMU`, and literal numeric size text.
  - Remove this helper with its caller.

- `crates/opentk-config/tests/config_loading.rs::assert_scratch_docs_contract`
  - Helper used by the scratch Docker test.
  - Reads `docs/docker-scratch.md` and `README.md`.
  - Asserts exact command examples, prose fragments, and README links.
  - Remove this helper with its caller.

- `crates/opentk-config/tests/config_loading.rs::postgres_init_script_runs_migrations_without_ignoring_errors`
  - Reads `scripts/init-db.sh`.
  - Asserts exact shell script strings like `set -euo pipefail`, `sqlx migrate run --source /workspace/migrations`, `psql`, `--set ON_ERROR_STOP=1`, and `/workspace/migrations/*.up.sql`.
  - Remove this static shell-script text test. If migration startup behavior matters, test behavior directly elsewhere.

- `crates/opentk-config/tests/production_source_contract.rs::sync_binary_is_named_opentk_sync_not_complete_sync`
  - Checks exact source-file existence/non-existence and reads `crates/opentk-db/Cargo.toml` for literal binary-name strings.
  - Remove this source/manfiest text contract test.

- `crates/opentk-config/tests/production_source_contract.rs::production_application_config_does_not_read_individual_setting_env_vars`
  - Reads `Cargo.toml`, `crates/opentk-config/src/lib.rs`, `crates/opentk-api/src/bin/opentk-api.rs`, `crates/opentk-api/src/lib.rs`, and `crates/opentk-db/src/bin/opentk-sync.rs`.
  - Forbids substrings like `OPENTK_`, `DATABASE_URL`, `dotenvy`, `std::env::var`, `env = `, and `features = ["derive", "env"]`.
  - This can fail on comments, docs, unrelated code, or valid future implementation and does not test behavior.

- `crates/opentk-config/tests/production_source_contract.rs::production_search_defaults_live_only_in_unified_config`
  - Reads production source files.
  - Forbids strings like `DEFAULT_SEARCH_URL`, `DEFAULT_SEARCH_INDEX`, `default_search_client`, `pub fn router(pool`, `127.0.0.1:7700`, `OPENTK_MEILISEARCH`, and `OPENTK_SEARCH`.
  - Remove this source-code substring scanner.

- `crates/opentk-config/tests/production_source_contract.rs::runtime_binaries_initialize_plain_fmt_logging_and_log_redacted_config`
  - Reads binary source files.
  - Requires exact source snippets `tracing_subscriber::fmt::init();` and `config.redacted()`.
  - Forbids `EnvFilter`.
  - Remove this implementation-style text test. Runtime logging/config redaction should be verified behaviorally if needed.

- `crates/opentk-search/tests/documentation.rs::search_indexing_pipeline_document_covers_operational_design`
  - Reads `docs/search-indexing-pipeline.md`.
  - Asserts docs contain exact section/prose/data snippets like `Backfill Indexing`, `Incremental Indexing`, `Failure Recovery`, `API Result Shape`, `3,579 bytes`, `716 bytes`, `search_index_cursor`, and `search_index_failure`.
  - Remove this documentation text coverage test.

- `crates/opentk-search-eval/tests/documentation.rs::investigation_doc_records_measured_recommendation`
  - Reads `docs/search-engine-investigation.md`.
  - Asserts exact fixture paths and prose terms like `Recommendation`, `Meilisearch`, `disk`, `build`, `update`, `quality`, and `Story 5 Tasks 2-4`.
  - Remove this documentation text coverage test.

Borderline tests to inspect carefully and remove if they are only text snapshots rather than meaningful behavior:

- `crates/opentk-db/tests/postgres_schema.rs::checked_in_migrations_match_schema_spec`
  - Uses `include_str!` to read checked-in SQL migrations and compares full text exactly against generated output.
  - This may be intentional generated-output drift detection, but it is still text equality. Decide whether it should be removed or replaced with behavior that applies migrations and validates resulting schema behavior.

- `crates/opentk-core/tests/workspace_smoke.rs::documented_opentk_sync_cargo_command_links_and_prints_help`
  - Runs `cargo run -p opentk-db --bin opentk-sync -- --help`, then asserts stdout contains exact help prose.
  - This executes code, but the important assertion is still exact help text. Consider replacing with a less brittle CLI behavior assertion or removing it.

- `crates/opentk-sync/tests/document_content_extractor.rs` golden expected-content fixtures.
  - Uses `include_str!` for `fixture.html.expected.html`, `fixture.html.expected.txt`, `fixture.docx.expected.txt`, and `fixture.pdf.expected.txt`.
  - These are more behavioral than Dockerfile/source scanners, but inspect whether they are useful extractor behavior tests or just brittle text snapshots.

- `crates/opentk-api/tests/cases/deep_verify_http_api.rs` document-content golden expected fixture assertions.
  - Imports expected document extraction fixture text and compares API output exactly.
  - Inspect whether this is necessary API behavior coverage or redundant brittle golden text.

- `crates/opentk-sync/tests/payload_parser.rs` XML fixture `include_str!` tests.
  - Uses XML fixtures for parser behavior.
  - Probably acceptable behavior tests, but verify they are not merely asserting arbitrary text fragments.

- `crates/opentk-db/tests/sync_writer.rs` XML fixture `include_str!` tests.
  - Uses syncfeed XML fixture text in DB writer tests.
  - Probably acceptable behavior tests, but verify they are not merely asserting arbitrary text fragments.

Also do a repo-wide sweep for any other bs-tests that do dumb string scanning or text-fragment assertions instead of testing behavior. Search for patterns such as `fs::read_to_string`, `include_str!`, `.contains("...")`, `starts_with`, `ends_with`, `serde_yaml::to_string(...).contains(...)`, and tests reading files under `docker/`, `.github/`, `docs/`, `README.md`, `Cargo.toml`, `scripts/`, or production `src/`.

Do not remove tests that genuinely exercise actual code/logic just because they use fixtures. The target is tests whose main value is checking that particular text strings exist or do not exist in files.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [ ] Removed the listed Dockerfile, GitHub Actions, shell script, docs, README, Cargo manifest, and production source substring tests
- [ ] Swept the repo for other bs-tests that assert particular text strings exist or do not exist in files
- [ ] Removed any additional tests found that do dumb source/docs/config text-fragment assertions instead of behavior
- [ ] Preserved tests that genuinely exercise actual code/logic/behavior, including legitimate fixture-driven parser/extractor tests where they add behavioral coverage
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
