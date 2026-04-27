## Plan: Story 09 Task 02 - Timing Verification

Task file:
`.ralph/tasks/story-09-ci-cd-multi-arch/task-02-github-workflow-docker-build.md`

Mandatory skills:
- `$tdd`: execute as vertical RED -> GREEN slices. Do not add broad speculative tests; each new or changed test must first fail against a real missing behavior.
- `$improve-code-boundaries`: keep the workflow as the executable public contract, keep Dockerfiles responsible for artifact/image contents, and keep timing/check logic focused on the workflow build limit.
- `github-api-auth-wrapper`: use `/home/joshazimullah.linux/github-api-curl` for GitHub Actions dispatch/log inspection without reading or exposing a token.

## Current State

- The task is back at `TO BE VERIFIED` pending verification against the GitHub workflow timing requirement.
- The task states that no Docker workflow build may take longer than 10 minutes.
- Previous local gates passed on the implementation:
  - `make check`
  - `make lint`
  - `make test`
- Previous real GitHub run evidence from progress:
  - build `opentk-sync`: 6m42s
  - build `opentk-api`: 7m09s
  - publish: 26s
  - the implementation satisfied the Docker workflow build timing requirement.
- Previous implementation commit `a1c9ff5` was pushed, but the task was not marked passing.

## Public Interface Design

The public interface remains the committed CI/build surface:
- `.github/workflows/docker.yml`
- `docker/Dockerfile.scratch-artifacts`
- `docker/Dockerfile.scratch`
- repository tests that validate the observable workflow/Dockerfile contract

Do not introduce a generator, wrapper CLI, or alternate build API. GitHub Actions and Dockerfiles are the interface.

Keep the existing boundary split:
- GitHub workflow owns orchestration, parallelism, artifact upload/download, GHCR authentication placement, and publish separation.
- `docker/Dockerfile.scratch-artifacts` owns binary selection and cross-compiled artifact production for both target architectures from the native runner.
- `docker/Dockerfile.scratch` owns final scratch image assembly by copying the correct architecture-specific artifact.
- Tests should describe externally meaningful CI behavior, not private helper shape.

The design correction for this plan is not a new workflow architecture. The timing gate is:
- no Docker workflow build job may exceed 10 minutes.

## TDD Execution Plan

Use vertical slices. Do not write all tests first.

1. Read the current task, this plan, the TDD skill, and all `$improve-code-boundaries` smell notes.
2. Inspect `.github/workflows/docker.yml`, `docker/Dockerfile.scratch-artifacts`, `docker/Dockerfile.scratch`, and the existing workflow/Dockerfile contract tests.
3. RED only if needed: if any test or task-local assertion fails to describe the 10-minute Docker workflow timing gate, add or update one focused test that fails because the public contract should describe that gate.
4. GREEN only if step 3 found stale executable contract code: update that code while preserving cache-behavior assertions and the 10-minute gate.
5. If executable code and tests already encode the 10-minute timing gate correctly, do not manufacture a RED cycle. Record that no executable contract change was needed, because adding a test that is already green would violate the `$tdd` skill.
6. Run the focused workflow/Dockerfile contract test if a focused test file exists for this task.
7. Refactor with `$improve-code-boundaries`:
   - remove stale wording in docs or Ralph plan references only when they would mislead execution
   - do not add wrapper abstractions around YAML or Dockerfile text
   - keep cache evidence requirements separate from timing acceptance
8. Run `make check`.
9. Run `make lint`.
10. Run `make test`.
11. Do not run `make test-long` or e2e for this normal task.
12. If local gates fail, fix them through vertical RED -> GREEN slices where behavior is missing, then rerun the gates.
13. Commit and push any new changes only after local gates pass. If there are no source changes beyond Ralph planning state, still include all changed Ralph files in the final task commit as required by the workflow.
14. Use `/home/joshazimullah.linux/github-api-curl` to inspect the existing pushed workflow evidence if it is still on the commit being finished; otherwise dispatch a new `workflow_dispatch` run for the pushed commit.
15. Verify from real GitHub logs:
    - workflow ran without QEMU/emulation setup
    - build uses the repository Dockerfiles directly
    - build job creates OCI artifacts/manifests without GHCR auth
    - publish job downloads built OCI artifacts, authenticates to GHCR only there, and does not rebuild images
    - cache logs show reuse/coverage for Cargo dependencies, target artifacts, Docker layers, and final image assembly
    - every Docker workflow build job duration is less than 10 minutes
16. If any Docker workflow build job exceeds 10 minutes, keep `<passes>false</passes>`, switch this plan and the task back to `TO BE VERIFIED`, append exact timing evidence to progress, and quit immediately.
17. If cache evidence is absent or publish rebuilds images, keep `<passes>false</passes>`, switch this plan and the task back to `TO BE VERIFIED`, append exact evidence to progress, and quit immediately.
18. Only if local gates and real GitHub verification pass:
    - set `<passes>true</passes>` in the task file
    - run `/bin/bash .ralph/task_switch.sh`
    - add all changed files, including `.ralph`
    - commit with `task finished task-02-github-workflow-docker-build: ...`, including local gate evidence and GitHub timing/log evidence
    - push
    - quit immediately

## Verification Evidence From Execution

- Local focused workflow contract test passed:
  `CARGO_INCREMENTAL=0 cargo test -p opentk-config --test config_loading github_docker_workflow -- --nocapture`
- Local gates passed:
  - `make check`
  - `make lint`
  - `make test`
- Real GitHub Docker workflow run `25000953101` on pushed commit `279b91bc36ec4d7e9f8b4b3776e6696852a7ae22` completed successfully.
- Timing passed the 10-minute Docker build job limit:
  - `build (opentk-sync)`: `2026-04-27T14:28:37Z` to `2026-04-27T14:35:22Z` (`6m45s`)
  - `build (opentk-api)`: `2026-04-27T14:28:37Z` to `2026-04-27T14:35:41Z` (`7m04s`)
  - `publish`: `2026-04-27T14:35:45Z` to `2026-04-27T14:36:10Z` (`25s`)
- Publish/build separation passed in the logs:
  - build uploaded `opentk-sync-oci-archive` and `opentk-api-oci-archive`
  - publish downloaded those artifacts
  - GHCR login occurred only in publish
  - publish used `skopeo copy --all` from `oci-archive:${binary}.oci.tar`
  - publish logs did not contain `docker buildx build`
- Native/no-emulation evidence passed:
  - logs showed Buildx builder `OS/Arch: linux/amd64`
  - no `setup-qemu`, `qemu`, or `emulat` log matches were present
- Cache verification did not pass strongly enough:
  - final scratch assembly logs showed a cached metadata step (`#4 CACHED`) and 2-3 second assembly steps
  - artifact build logs still showed dependency downloads and long dependency compilation in `dependency-cache`, including `Downloaded string_cache...` and `Compiling string_cache...`
  - both matrix artifact jobs still ran long dependency-cache work, so Cargo dependency/target cache reuse is not yet proven effective

Design must be revisited before the task can pass. The next plan should make Cargo dependency and target cache reuse visibly effective in GitHub logs, or replace the cache approach with one that does.

## Acceptance Checklist

- [ ] `.github/workflows/docker.yml` is valid YAML.
- [ ] Workflow directly uses repository Dockerfiles to build scratch images before publish.
- [ ] Build is split from publish.
- [ ] GHCR authentication happens only in publish.
- [ ] Publish consumes build artifacts and does not rebuild images.
- [ ] `opentk-sync` and `opentk-api` images are built for `linux/amd64` and `linux/arm64`.
- [ ] Each scratch image contains the correct binary for its target architecture.
- [ ] Tags cover `latest`, `sha-{short_sha}`, and release tag names.
- [ ] Buildx is configured for native Rust cross-compilation and no QEMU/emulation.
- [ ] Cache coverage exists for Cargo dependencies, target artifacts, Docker layers, and final assembly.
- [ ] Real GitHub logs verify cache behavior and publish/build separation.
- [ ] Every Docker workflow build job completes in less than 10 minutes.
- [ ] The workflow timing gate is the 10-minute Docker workflow build limit.
- [ ] `make check` passes.
- [ ] `make lint` passes.
- [ ] `make test` passes.

TO BE VERIFIED
