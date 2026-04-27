## Plan: Story 09 Task 02 - GitHub Docker Workflow

Task file:
`.ralph/tasks/story-09-ci-cd-multi-arch/task-02-github-workflow-docker-build.md`

Mandatory skills:
- `$tdd`: execute as vertical RED -> GREEN slices through behavior tests against the workflow and Docker contract files.
- `$improve-code-boundaries`: keep the workflow boundary simple; avoid duplicate image/tag/cache logic spread between tests, shell scripts, and workflow YAML.
- `github-api-auth-wrapper`: verify real GitHub workflow runs with `/home/joshazimullah.linux/github-api-curl`; do not read or expose the token.

## Current State

- Existing scratch image boundary:
  - `docker/Dockerfile.scratch-artifacts` builds all four static Rust artifacts from one build platform using native Rust cross-compilation.
  - `docker/Dockerfile.scratch` copies the correct target artifact into a `scratch` image based on `TARGETARCH` and `BINARY`.
  - `scripts/docker-buildx-scratch.sh` locally verifies the same artifact-then-final-image flow.
- Existing tests already cover Dockerfile/script contracts in `crates/opentk-config/tests/config_loading.rs`.
- `.github/workflows/docker.yml` does not exist yet.

## Public Interface Design

The public interface for this task is the committed workflow file:
`.github/workflows/docker.yml`.

Do not create a bespoke workflow generator or Rust API unless a RED test proves the raw YAML contract has become too muddy. The workflow itself is the deployable artifact, and tests should parse/assert its observable behavior.

Proposed workflow shape:

- Triggers:
  - `push` to `main`
  - `push` tags matching `v*`
  - `workflow_dispatch`
- Permissions:
  - default read permissions for the build job
  - package write permission only where required for publish
- `build` job:
  - checkout repository
  - setup Docker Buildx
  - explicitly avoid QEMU setup and QEMU action usage
  - build one artifact image from `docker/Dockerfile.scratch-artifacts` on one invoking architecture
  - use BuildKit cache for Cargo registry, Cargo git, Rust target artifacts, Docker layers, and final assembly
  - assemble `opentk-sync` and `opentk-api` from `docker/Dockerfile.scratch` for `linux/amd64,linux/arm64`
  - export build result as OCI/Docker artifacts or a local registry/cache output consumable by publish without rebuilding Rust dependencies
  - verify metadata/manifests include both platforms and that final assembly is artifact-copy only
- `publish` job:
  - depends on `build`
  - authenticates to `ghcr.io` only in this job
  - consumes the already-built result/cache from `build`
  - pushes `ghcr.io/{owner}/opentk-sync` and `ghcr.io/{owner}/opentk-api`
  - applies tags:
    - main: `latest`, `sha-{short_sha}`
    - release tag: `{tag}`
  - publish/auth/GHCR failures must be isolated from image build failures.

## TDD Execution Plan

Use vertical slices; one behavioral test first, then minimal workflow changes, then repeat.

1. RED: add one test in `crates/opentk-config/tests/config_loading.rs` asserting `.github/workflows/docker.yml` exists and has the required trigger surface: `main`, `v*`, and `workflow_dispatch`.
2. GREEN: add the smallest valid workflow skeleton satisfying that trigger contract.
3. RED: extend the test to assert the workflow has split `build` and `publish` jobs, and that `docker/login-action` or equivalent GHCR auth appears only in `publish`.
4. GREEN: add the two jobs and publish-only GHCR auth.
5. RED: extend the test to assert the build job directly uses `docker/Dockerfile.scratch-artifacts` and `docker/Dockerfile.scratch`, builds both `opentk-sync` and `opentk-api`, and targets `linux/amd64,linux/arm64`.
6. GREEN: add artifact build and final image assembly steps using Docker Buildx and the repository Dockerfiles directly.
7. RED: extend the test to assert no QEMU/emulation action or `setup-qemu` usage exists, and that artifact build runs from `BUILDPLATFORM`/native Rust cross-compilation instead of emulated target build.
8. GREEN: remove any emulation setup and keep the workflow aligned with `docker/Dockerfile.scratch-artifacts`.
9. RED: extend the test to assert cache coverage for Cargo registry, Cargo git, Rust target artifacts, Docker layer cache, and final image assembly cache.
10. GREEN: add BuildKit cache mounts/cache-from/cache-to or equivalent cache configuration immediately in the first workflow version.
11. RED: extend the test to assert tag behavior for `latest`, `sha-{short_sha}`, and release tags, with GHCR image names for both binaries.
12. GREEN: add metadata/tag steps for both images.
13. Refactor: if workflow tests become string soup, introduce small YAML helper functions inside the test module only when they reduce duplication without hiding the public workflow behavior.
14. Run `make check`, `make lint`, and `make test`. Do not run `make test-long` unless the task is escalated to story-end validation.
15. Trigger/inspect a real `workflow_dispatch` run. Use `/home/joshazimullah.linux/github-api-curl` like normal `curl` to inspect workflow runs and logs. Confirm:
    - cache hits/reuse for Cargo dependencies, target artifacts, Docker layers, and final image assembly
    - cached Docker workflow completes under 5 minutes
    - no Docker workflow build exceeds 10 minutes
    - test/build/image work does not rebuild the same Rust dependencies twice
    - publish consumes the built result rather than rebuilding images
16. If the workflow exceeds timing limits or cache reuse is weak, alter the workflow before proceeding. Do not mark the task passing in that state.
17. Final boundary review using `$improve-code-boundaries`:
    - workflow should own GitHub CI behavior
    - Dockerfiles should own image contents and architecture-to-binary selection
    - tests should assert public contracts without duplicating workflow implementation
    - no ad-hoc error swallowing, skipped tests, or linter bypasses

## Acceptance Checklist

- [ ] `.github/workflows/docker.yml` is valid YAML and committed.
- [ ] Build job uses repository Dockerfiles directly.
- [ ] Artifact build creates both target-architecture binaries from one invoking architecture without emulation.
- [ ] Final scratch images copy the correct per-arch artifact.
- [ ] Build and publish are split, with GHCR auth only in publish.
- [ ] Tags cover `latest`, `sha-{short_sha}`, and release version tags.
- [ ] Cache coverage exists immediately for Cargo dependencies, target artifacts, Docker layers, and final assembly.
- [ ] Real GitHub logs verify cache hits and timing.
- [ ] `make check` passes.
- [ ] `make lint` passes.
- [ ] `make test` passes.

TO BE VERIFIED
