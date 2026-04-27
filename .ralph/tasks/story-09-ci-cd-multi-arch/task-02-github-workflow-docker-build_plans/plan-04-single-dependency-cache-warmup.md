## Plan: Story 09 Task 02 - Single Dependency Cache Warmup

Task file:
`.ralph/tasks/story-09-ci-cd-multi-arch/task-02-github-workflow-docker-build.md`

Plan path:
`.ralph/tasks/story-09-ci-cd-multi-arch/task-02-github-workflow-docker-build_plans/plan-04-single-dependency-cache-warmup.md`

Mandatory skills:
- `$tdd`: execute as vertical RED -> GREEN slices. Add one behavior assertion, prove it fails, implement the smallest workflow/Dockerfile change, then rerun the focused test before adding the next assertion.
- `$improve-code-boundaries`: keep the workflow as the public CI contract, keep Dockerfiles responsible for build/image contents, and remove cache responsibility from duplicated binary matrix jobs where possible.
- `github-api-auth-wrapper`: use `/home/joshazimullah.linux/github-api-curl` for GitHub workflow dispatch and log inspection without reading or exposing a token.

## Current Evidence

- Previous implementation already passed local gates:
  - `make check`
  - `make lint`
  - `make test`
- Real GitHub Docker workflow run `25000953101` on commit `279b91bc36ec4d7e9f8b4b3776e6696852a7ae22` completed successfully.
- Timing was acceptable:
  - `build (opentk-sync)`: `6m45s`
  - `build (opentk-api)`: `7m04s`
  - `publish`: `25s`
- Publish/build separation was acceptable:
  - build uploaded `opentk-sync-oci-archive` and `opentk-api-oci-archive`
  - publish downloaded those artifacts
  - GHCR login happened only in publish
  - publish used `skopeo copy --all` from `oci-archive:${binary}.oci.tar`
  - publish did not run `docker buildx build`
- Native/no-emulation evidence was acceptable:
  - Buildx builder was `linux/amd64`
  - logs had no `setup-qemu`, `qemu`, or `emulat` matches
- Cache verification failed:
  - final scratch assembly had cached metadata and fast 2-3 second assembly steps
  - artifact build logs still showed dependency downloads and long dependency compilation in `dependency-cache`
  - both binary matrix jobs did that dependency-cache work, so Cargo dependency/target cache reuse was not proven effective

## Corrected Interface Design

The public interface remains:
- `.github/workflows/docker.yml`
- `docker/Dockerfile.scratch-artifacts`
- `docker/Dockerfile.scratch`
- repository tests asserting the observable workflow/Dockerfile contract

Do not introduce a workflow generator, bespoke Rust build API, wrapper CLI, or YAML rendering layer.

Boundary correction:
- Add a single workflow job before binary fan-out, tentatively named `warm-dependency-cache`.
- `warm-dependency-cache` runs the repository artifact Dockerfile up to the dependency-cache boundary once from the native `ubuntu-24.04` amd64 runner.
- The binary `build` matrix must depend on `warm-dependency-cache`.
- The binary `build` matrix must consume the warmed dependency cache and must not be responsible for proving dependency compilation reuse by racing both binaries through the same dependency-cache stage.
- `build` remains the job that produces and uploads binary-specific OCI archives.
- `publish` remains separate, authenticates to GHCR only there, downloads OCI archives, and pushes them without rebuilding images.

Dockerfile correction:
- Keep `docker/Dockerfile.scratch-artifacts` as the owner of dependency planning and cross-target artifact production.
- Ensure the dependency-cache stage is a clean, buildable target independent of `ARG BINARY`.
- Keep `ARG BINARY` validation and final binary compilation in the artifact-builder stage.
- If BuildKit cache mounts still do not produce visible reuse, prefer a buildable dependency cache image/layer target that can be exported to and restored from `type=gha` cache, rather than adding ad-hoc shell caches outside Docker.

Workflow cache correction:
- Use a shared dependency cache scope only in `warm-dependency-cache` for writing.
- Binary matrix jobs may read the shared dependency cache scope, but should not write it unless a RED test or real logs show that a read-only dependency scope is impossible.
- Use binary-specific artifact target scopes for final binary compilation.
- Keep binary-specific final assembly cache scopes.
- The real log success condition is visible reuse:
  - `warm-dependency-cache` may compile dependencies on a cold run
  - binary matrix jobs must not both compile the full dependency graph in `dependency-cache`
  - on a warmed run, dependency-cache should show cache hits/reuse rather than fresh dependency downloads and long dependency compilation

## TDD Execution Plan

Use vertical slices. Do not write all tests first.

1. Read the current task, this plan, the `$tdd` skill, and `$improve-code-boundaries` skill.
2. Inspect `.github/workflows/docker.yml`, `docker/Dockerfile.scratch-artifacts`, `docker/Dockerfile.scratch`, and the workflow/Dockerfile contract tests in `crates/opentk-config/tests/config_loading.rs`.
3. RED: update `github_docker_workflow_caches_cargo_targets_layers_and_final_assembly` or add one focused test asserting the workflow has a `warm-dependency-cache` job.
   - It must use `docker/Dockerfile.scratch-artifacts`.
   - It must build only the dependency-cache target or equivalent dependency-planning boundary.
   - It must write the shared `opentk-scratch-deps` cache scope.
   - It must not contain `${{ matrix.binary }}`.
4. Run the focused test and confirm it fails for the missing warmup boundary.
5. GREEN: update `.github/workflows/docker.yml` with `warm-dependency-cache`.
   - Use `runs-on: ubuntu-24.04`.
   - Checkout and setup Buildx.
   - Run `docker buildx build` against `docker/Dockerfile.scratch-artifacts`.
   - Target the dependency-cache stage or equivalent.
   - Export the dependency cache to `type=gha,scope=opentk-scratch-deps,mode=max`.
6. Run the focused test and confirm it passes.
7. RED: extend the workflow contract test to require `build.needs` includes `warm-dependency-cache` while `publish.needs` remains `build`.
8. Run the focused test and confirm it fails if `build` is not sequenced after warmup.
9. GREEN: wire the existing binary matrix `build` job to depend on `warm-dependency-cache`.
10. Run the focused test and confirm it passes.
11. RED: extend the cache contract test so binary matrix builds read the shared dependency cache but do not write the shared dependency cache from each matrix job.
12. Run the focused test and confirm it fails against the current matrix writing `--cache-to "type=gha,scope=opentk-scratch-deps,mode=max"`.
13. GREEN: remove shared dependency cache writes from the binary matrix job, keeping binary-specific artifact cache writes and final assembly cache writes.
14. Run the focused test and confirm it passes.
15. If any step shows the dependency-cache Dockerfile target cannot be built cleanly without `BINARY`, adjust only the Dockerfile stage boundary needed to make that target explicit and testable.
16. Run the full focused contract test:
    `CARGO_INCREMENTAL=0 cargo test -p opentk-config --test config_loading github_docker_workflow -- --nocapture`
17. Refactor with `$improve-code-boundaries`:
    - keep warmup orchestration in workflow YAML
    - keep dependency planning inside `docker/Dockerfile.scratch-artifacts`
    - avoid shell wrappers unless a RED test proves the workflow became too muddy
    - remove stale Ralph plan wording only if it would mislead execution
18. Run `make check`.
19. Run `make lint`.
20. Run `make test`.
21. Do not run `make test-long` or any e2e lane for this normal non-story-finishing task.
22. Commit and push implementation changes only after local gates pass.
23. Use `/home/joshazimullah.linux/github-api-curl` to dispatch and inspect a real Docker workflow run on the pushed commit.
24. Verify from real GitHub logs:
    - `warm-dependency-cache` ran before matrix image builds
    - no Docker workflow build job exceeded 10 minutes
    - no `setup-qemu`, `qemu`, or `emulat` log evidence
    - build uses repository Dockerfiles directly
    - GHCR auth occurs only in publish
    - publish downloads OCI artifacts and contains no `docker buildx build`
    - final assembly cache is reused
    - binary matrix jobs do not both rebuild the full Rust dependency graph in `dependency-cache`
    - on a warmed run, dependency-cache reuse is visible through cached BuildKit steps or absence of fresh dependency download/compile logs
25. If real logs still show both binary matrix jobs doing fresh dependency downloads/long dependency compilation, keep `<passes>false</passes>`, switch this plan and task back to `TO BE VERIFIED`, append exact evidence to progress, and quit immediately.
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
- [ ] A single dependency warmup job prepares shared Cargo dependency/target cache before binary fan-out.
- [ ] Binary matrix builds depend on the dependency warmup job.
- [ ] Binary matrix builds do not both write the same shared dependency cache scope.
- [ ] Build is split from publish.
- [ ] GHCR authentication happens only in publish.
- [ ] Publish consumes build artifacts and does not rebuild images.
- [ ] `opentk-sync` and `opentk-api` images are built for `linux/amd64` and `linux/arm64`.
- [ ] Each scratch image contains the correct binary for its target architecture.
- [ ] Tags cover `latest`, `sha-{short_sha}`, and release tag names.
- [ ] Buildx is configured for native Rust cross-compilation and no QEMU/emulation.
- [ ] Cache coverage exists for Cargo dependencies, target artifacts, Docker layers, and final assembly.
- [ ] Real GitHub logs verify dependency-cache reuse, target/artifact cache reuse, final assembly cache reuse, and publish/build separation.
- [ ] Every Docker workflow build job completes in less than 10 minutes.
- [ ] `make check` passes.
- [ ] `make lint` passes.
- [ ] `make test` passes.

NOW EXECUTE
