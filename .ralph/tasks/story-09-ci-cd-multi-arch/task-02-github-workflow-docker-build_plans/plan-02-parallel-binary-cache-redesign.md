## Plan: Story 09 Task 02 - Parallel Binary Docker Workflow Redesign

Task file:
`.ralph/tasks/story-09-ci-cd-multi-arch/task-02-github-workflow-docker-build.md`

Mandatory skills:
- `$tdd`: execute as vertical RED -> GREEN slices. Each test must assert observable workflow/Docker behavior before the implementation change that satisfies it.
- `$improve-code-boundaries`: keep GitHub orchestration in `.github/workflows/docker.yml`, image content/architecture selection in Dockerfiles, and test assertions focused on public contracts. Remove serial shell glue when the workflow matrix can own that boundary.
- `github-api-auth-wrapper`: use `/home/joshazimullah.linux/github-api-curl` for real workflow dispatch/log verification without reading or exposing the token.

## Current State

- `.github/workflows/docker.yml` exists and passes local contract tests, but real GitHub logs showed the cached artifact path still taking more than 5 minutes.
- `docker/Dockerfile.scratch-artifacts` builds all four release artifacts in one Docker build:
  - `opentk-sync` for `x86_64-unknown-linux-musl`
  - `opentk-api` for `x86_64-unknown-linux-musl`
  - `opentk-sync` for `aarch64-unknown-linux-musl`
  - `opentk-api` for `aarch64-unknown-linux-musl`
- The workflow then serially assembles both final OCI archives in one shell loop.
- The timing boundary is too coarse: one slow artifact step gates both images, and the build job cannot complete until both serial image assemblies finish.

## Public Interface Design

The public interface remains the committed deployable workflow and Dockerfiles:
- `.github/workflows/docker.yml`
- `docker/Dockerfile.scratch-artifacts`
- `docker/Dockerfile.scratch`

Do not add a workflow generator, YAML-rendering helper, or bespoke Rust API. The workflow itself is the artifact users and GitHub Actions execute.

Redesign the build unit around one binary image per matrix job:
- `build` is a matrix job over `binary: [opentk-sync, opentk-api]`.
- Each matrix job:
  - runs on `ubuntu-24.04`
  - sets up Buildx without QEMU/emulation
  - builds a binary-scoped artifact image from `docker/Dockerfile.scratch-artifacts`
  - produces both `linux/amd64` and `linux/arm64` artifacts for that one binary from the native amd64 runner through Rust cross-compilation
  - immediately assembles that binary's final multi-platform OCI archive from `docker/Dockerfile.scratch`
  - uploads a binary-specific artifact such as `opentk-sync-oci-archive` or `opentk-api-oci-archive`
- `publish` depends on all matrix `build` jobs, downloads both binary-specific OCI archives, authenticates to GHCR only there, and pushes tags without rebuilding.

Dockerfile boundary changes:
- `docker/Dockerfile.scratch-artifacts` accepts `ARG BINARY`.
- It validates `BINARY` as `opentk-sync` or `opentk-api`.
- It builds only the requested binary for both musl targets, then copies only that binary's two target artifacts plus runtime files into `/artifacts/...`.
- Add a dependency-planning layer using `cargo-chef` or an equivalent source-before-deps separation, so dependency compilation is not invalidated by ordinary source changes.
- Use separate BuildKit cache scopes that still share dependency cache:
  - shared Cargo registry/git cache scope for both binaries
  - shared dependency-planning/target dependency scope where possible
  - binary-specific final target/artifact/image scopes to avoid two matrix jobs fighting over one mutable target cache
  - binary-specific final image assembly cache scopes

This design intentionally trades some duplicated orchestration text for better CI critical path: each image can finish independently, while the publish job remains a pure publication step.

## TDD Execution Plan

Use vertical slices. Do not write all tests first.

1. RED: update `github_docker_workflow_builds_scratch_images_from_repository_dockerfiles` or add one focused test asserting the build job is a matrix over `opentk-sync` and `opentk-api`, uploads binary-specific OCI artifacts, and does not contain a serial `for binary in` assembly loop.
2. GREEN: change `.github/workflows/docker.yml` to use `strategy.matrix.binary`, replace the shell loop with matrix-specific artifact and assembly commands, and make artifact names binary-specific.
3. Run the focused config-loading test. Keep going only when it passes.
4. RED: add/extend a Dockerfile contract test asserting `docker/Dockerfile.scratch-artifacts` has `ARG BINARY`, validates the two supported binaries, uses `${BINARY}` for cargo build/copy, and no longer copies both final binaries unconditionally.
5. GREEN: update `docker/Dockerfile.scratch-artifacts` to be binary-scoped while still producing both `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` artifacts from one native amd64 build.
6. Run the focused config-loading test again.
7. RED: add/extend the cache contract test for dependency planning:
   - requires `cargo-chef` or an equivalent explicit dependency recipe/planning layer
   - requires Cargo registry/git cache mounts
   - requires target cache mounts
   - requires workflow `cache-from`/`cache-to` for shared dependency/artifact layers and binary-specific final assembly layers
   - forbids a single shared `opentk-scratch-target` mutable target cache for both matrix binaries if the workflow fans out in parallel
8. GREEN: add the dependency-planning Dockerfile stages and update workflow cache scopes accordingly.
9. Run the focused config-loading test again.
10. RED: update the publish contract test so `publish` downloads both binary-specific OCI artifacts and still contains no `docker buildx build`.
11. GREEN: update the publish job to download both OCI archives, login to GHCR only in `publish`, and `skopeo copy --all` each downloaded archive to `ghcr.io/{owner}/{binary}:{tag}`.
12. Run the focused config-loading test again.
13. Refactor with `$improve-code-boundaries`:
    - keep the workflow as the public contract
    - keep Dockerfiles responsible for image contents and architecture-to-artifact selection
    - if test assertions become repetitive, add tiny test-local helpers only when they remove duplicated YAML traversal
    - remove obsolete serial-loop expectations and any stale docs that describe the old all-binaries artifact image
14. Run `make check`.
15. Run `make lint`.
16. Run `make test`.
17. Do not run `make test-long` or e2e for this normal task.
18. Commit and push the implementation only after the local gates pass.
19. Use `/home/joshazimullah.linux/github-api-curl` to dispatch and inspect a real Docker workflow run on the pushed commit.
20. Inspect job timings and logs. Acceptance requires:
    - no Docker workflow build job exceeds 10 minutes
    - a cached Docker workflow run completes in under 5 minutes
    - Cargo dependency/cache logs show reuse for dependency artifacts, target artifacts, Docker layers, and final assembly
    - no QEMU/emulation setup appears
    - publish downloads the build artifacts and does not rebuild images
21. If real logs still miss the timing/cache requirements, keep `<passes>false</passes>`, switch this plan and the task back to `TO BE VERIFIED`, append progress with the exact failed timing evidence, and quit.
22. Only if local gates and real GitHub timing/cache verification pass:
    - set `<passes>true</passes>` in the task file
    - run `/bin/bash .ralph/task_switch.sh`
    - add and commit all changed files, including `.ralph`, with `task finished task-02-github-workflow-docker-build: ...`
    - push
    - quit immediately

## Acceptance Checklist

- [ ] `.github/workflows/docker.yml` is valid YAML.
- [ ] Build uses a matrix over `opentk-sync` and `opentk-api`.
- [ ] Build job uses repository Dockerfiles directly.
- [ ] Each matrix artifact build creates both target-architecture artifacts for its binary from one native invoking architecture without emulation.
- [ ] No serial workflow shell loop assembles both binaries behind one job step.
- [ ] Final scratch images copy the correct per-arch artifact.
- [ ] Build and publish are split, with GHCR auth only in publish.
- [ ] Publish downloads OCI artifacts and never rebuilds images.
- [ ] Tags cover `latest`, `sha-{short_sha}`, and release version tags.
- [ ] Dependency planning prevents ordinary source changes from invalidating dependency compilation.
- [ ] Cache coverage exists for Cargo dependencies, target artifacts, Docker layers, and final assembly.
- [ ] Real GitHub logs verify cache hits and timing.
- [ ] `make check` passes.
- [ ] `make lint` passes.
- [ ] `make test` passes.

NOW EXECUTE
