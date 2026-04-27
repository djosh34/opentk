## Plan: Story 09 Task 02 - Explicit Dependency OCI Build Context

Task file:
`.ralph/tasks/story-09-ci-cd-multi-arch/task-02-github-workflow-docker-build.md`

Plan path:
`.ralph/tasks/story-09-ci-cd-multi-arch/task-02-github-workflow-docker-build_plans/plan-05-explicit-dependency-oci-context.md`

Mandatory skills:
- `$tdd`: execute in vertical RED -> GREEN slices. Add one observable contract assertion, prove it fails, implement the smallest workflow/Dockerfile change, rerun the focused test, then continue.
- `$improve-code-boundaries`: move dependency-cache ownership out of duplicated binary matrix builds. The dependency output must be an explicit build artifact/context, not an implicit stage that each matrix job can rebuild.
- `github-api-auth-wrapper`: use `/home/joshazimullah.linux/github-api-curl` to dispatch and inspect real GitHub workflow logs without reading or exposing tokens.

## Failed Prior Design

Plan 04 added `warm-dependency-cache`, but real GitHub workflow run `25002033554` on commit `a1cf096fdc8b2ad15ecfacc0c13aff50a0885bae` failed cache acceptance.

- `warm-dependency-cache`: `4m31s`
- `build (opentk-sync)`: `6m35s`
- `build (opentk-api)`: `7m05s`
- Both matrix artifact builds still rebuilt the Dockerfile `dependency-cache` stage:
  - `opentk-sync`: `383` `Downloaded` lines, `609` `Compiling` lines, dependency-cache cook about `222.2s`
  - `opentk-api`: `383` `Downloaded` lines, `611` `Compiling` lines, dependency-cache cook about `222.7s`

The boundary failure is that `docker/Dockerfile.scratch-artifacts` contains:

```dockerfile
COPY --from=dependency-cache /workspace/target /workspace/deps-target
```

Because `dependency-cache` is an internal Dockerfile stage, every binary matrix build is still responsible for materializing that stage. Exporting only `type=gha` cache from the warmup job was not a strong enough public interface for the matrix jobs.

## Corrected Interface Design

The public interface remains small:
- `.github/workflows/docker.yml`
- `docker/Dockerfile.scratch-artifacts`
- `docker/Dockerfile.scratch`
- repository contract tests in `crates/opentk-config/tests/config_loading.rs`

Do not introduce a workflow generator, wrapper CLI, bespoke YAML renderer, or new Rust API.

Boundary correction:
- `warm-dependency-cache` builds the dependency-only Dockerfile target once.
- `warm-dependency-cache` exports that target as an OCI artifact archive and uploads it with `actions/upload-artifact`.
- Binary matrix jobs download the dependency OCI artifact and pass it into BuildKit as a named build context.
- `docker/Dockerfile.scratch-artifacts` artifact compilation copies dependency outputs from that named build context instead of from an internal stage that matrix builds can rebuild.
- Binary matrix jobs still build binary-specific artifact images directly from `docker/Dockerfile.scratch-artifacts`.
- Final scratch assembly still uses `docker/Dockerfile.scratch`.
- `publish` remains separate, authenticates to GHCR only there, downloads OCI archives, and pushes them without rebuilding.

Preferred Dockerfile shape:
- Rename the internal warm stage from `dependency-cache` to `built-dependency-cache` or equivalent.
- Keep `built-dependency-cache` as the target used by `warm-dependency-cache`.
- Add an artifact-builder dependency boundary that uses `COPY --from=dependency-cache-context /workspace/target /workspace/deps-target`.
- The workflow supplies `dependency-cache-context` from the downloaded OCI artifact with BuildKit named context support.
- If a local build path also needs this context, update `scripts/docker-buildx-scratch.sh` to build the dependency OCI context first and pass it to artifact builds. Do this only if the focused tests show the script contract would otherwise become misleading or broken.

Workflow artifact transport:
- In `warm-dependency-cache`, build:
  - `docker buildx build --target built-dependency-cache`
  - keep `--cache-from/--cache-to type=gha,scope=opentk-scratch-deps`
  - export the target to `/tmp/opentk-scratch-deps.oci.tar` with an OCI output
  - upload `/tmp/opentk-scratch-deps.oci.tar` as `opentk-scratch-deps-oci`
- In each binary matrix job:
  - download `opentk-scratch-deps-oci`
  - unpack or reference it in the format BuildKit accepts for a named context
  - pass `--build-context dependency-cache-context=...`
  - remove `--cache-from type=gha,scope=opentk-scratch-deps` from the matrix artifact build if the explicit context makes it redundant
  - keep binary-specific artifact cache and final assembly cache scopes

Real-log success condition:
- The warm job may compile dependencies on a cold run.
- Matrix binary artifact logs must not show both jobs running full `cargo chef cook` dependency download/compile work.
- Matrix binary artifact logs should show the dependency context being loaded/imported and then binary-specific compilation only.
- No Docker workflow build job may exceed 10 minutes.

## TDD Execution Plan

Use vertical slices. Do not write all tests first.

1. Read this plan, the task file, `$tdd`, and `$improve-code-boundaries`.
2. Inspect `.github/workflows/docker.yml`, `docker/Dockerfile.scratch-artifacts`, `docker/Dockerfile.scratch`, `scripts/docker-buildx-scratch.sh`, and existing workflow contract tests.
3. RED: update one focused workflow cache contract test to require `warm-dependency-cache` uploads a dependency OCI artifact:
   - `--target built-dependency-cache`
   - `type=oci,dest=/tmp/opentk-scratch-deps.oci.tar`
   - `actions/upload-artifact`
   - `opentk-scratch-deps-oci`
4. Run:
   `CARGO_INCREMENTAL=0 cargo test -p opentk-config --test config_loading github_docker_workflow_caches_cargo_targets_layers_and_final_assembly -- --nocapture`
   Confirm it fails against the current plan-04 workflow.
5. GREEN: update `.github/workflows/docker.yml` warmup job to build and upload the dependency OCI artifact. Keep cache write scope in the warmup job.
6. Rerun the focused test and confirm it passes.
7. RED: extend the same focused contract test to require matrix jobs download the dependency OCI artifact and pass it as a named BuildKit context:
   - `actions/download-artifact`
   - `opentk-scratch-deps-oci`
   - `--build-context dependency-cache-context=`
8. Run the focused test and confirm it fails.
9. GREEN: update the build matrix job to download the artifact and pass the named context to the artifact build.
10. Rerun the focused test and confirm it passes.
11. RED: extend the artifact Dockerfile contract to require artifact builds copy from `dependency-cache-context` and to forbid `COPY --from=dependency-cache /workspace/target`.
12. Run the focused test and confirm it fails.
13. GREEN: update `docker/Dockerfile.scratch-artifacts`:
    - rename internal warm stage to `built-dependency-cache`
    - copy dependency target output from `dependency-cache-context`
    - keep `ARG BINARY` validation and final binary builds in the artifact-builder stage
14. Rerun the focused test and confirm it passes.
15. RED: run the broader focused contract suite:
    `CARGO_INCREMENTAL=0 cargo test -p opentk-config --test config_loading github_docker_workflow -- --nocapture`
    Fix only failures directly caused by the new dependency-context boundary.
16. If the local scratch build script contract becomes stale or broken, add one RED assertion for the script dependency-context flow, then update `scripts/docker-buildx-scratch.sh` minimally. Otherwise leave the script alone.
17. Refactor with `$improve-code-boundaries`:
    - dependency planning and cross-target dependency cooking stay in `docker/Dockerfile.scratch-artifacts`
    - cross-job dependency transport stays in workflow artifact plumbing
    - binary matrix jobs do not know how to cook dependencies
    - publish does not know how to build images
    - avoid adding shell wrappers unless tests prove a current script must be updated
18. Run `make check`.
19. Run `make lint`.
20. Run `make test`.
21. Do not run `make test-long` or e2e for this normal non-story-finishing task.
22. Commit and push only after local gates pass.
23. Use `/home/joshazimullah.linux/github-api-curl` to dispatch and inspect a real Docker workflow run on the pushed commit.
24. Verify from real GitHub logs:
    - `warm-dependency-cache` ran before matrix builds
    - warmup uploaded the dependency OCI artifact
    - matrix builds downloaded and used the dependency OCI context
    - matrix builds did not both run full fresh `cargo chef cook` dependency download/compile work
    - no Docker workflow build job exceeded 10 minutes
    - no `setup-qemu`, `qemu`, or `emulat` log evidence
    - build uses repository Dockerfiles directly
    - GHCR auth occurs only in publish
    - publish downloads OCI artifacts and contains no `docker buildx build`
    - final assembly cache is reused
25. If real logs still show both binary matrix jobs doing fresh dependency downloads/long dependency compilation, keep `<passes>false</passes>`, switch this plan and the task back to `TO BE VERIFIED`, append exact evidence to progress, and quit immediately.
26. Only if local gates and real GitHub verification pass:
    - set `<passes>true</passes>` in the task file
    - run `/bin/bash .ralph/task_switch.sh`
    - add all changed files, including `.ralph`
    - commit with `task finished task-02-github-workflow-docker-build: ...`, including local gate evidence and GitHub timing/log evidence
    - push
    - quit immediately

## Acceptance Checklist

- [ ] `.github/workflows/docker.yml` is valid YAML.
- [ ] Workflow directly uses repository Dockerfiles to build scratch images before publish.
- [ ] A single dependency warmup job prepares shared Cargo dependency/target output before binary fan-out.
- [ ] Dependency output is transported as an explicit OCI artifact/build context, not only an implicit BuildKit cache scope.
- [ ] Binary matrix builds depend on the dependency warmup job.
- [ ] Binary matrix builds do not both cook the full dependency graph.
- [ ] Build is split from publish.
- [ ] GHCR authentication happens only in publish.
- [ ] Publish consumes build artifacts and does not rebuild images.
- [ ] `opentk-sync` and `opentk-api` images are built for `linux/amd64` and `linux/arm64`.
- [ ] Each scratch image contains the correct binary for its target architecture.
- [ ] Tags cover `latest`, `sha-{short_sha}`, and release tag names.
- [ ] Buildx is configured for native Rust cross-compilation and no QEMU/emulation.
- [ ] Cache coverage exists for Cargo dependencies, target artifacts, Docker layers, and final assembly.
- [ ] Real GitHub logs verify dependency artifact/context reuse, target/artifact cache reuse, final assembly cache reuse, and publish/build separation.
- [ ] Every Docker workflow build job completes in less than 10 minutes.
- [ ] `make check` passes.
- [ ] `make lint` passes.
- [ ] `make test` passes.

NOW EXECUTE
