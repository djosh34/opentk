# Story 08 Task 03 Plan: Multi-Arch Scratch Images

## Context Read

- Current task: `.ralph/tasks/story-08-docker-local-dev/task-03-multi-arch-scratch.md`.
- Existing Docker boundary:
  - `docker/Dockerfile.api` and `docker/Dockerfile.sync` each build one static musl binary into a `scratch` runtime image.
  - The two Dockerfiles duplicate dependency-cache setup, target selection, runtime CA cert copy, identity file creation, `USER 1000:1000`, and scratch runtime layout.
  - `scripts/docker-build.sh` is only a local single-arch convenience script and should stay that way unless execution proves replacement is simpler.
- Existing cargo boundary:
  - `.cargo/config.toml` only sets `CARGO_INCREMENTAL = "0"`.
  - No linker or cross-target config exists yet for `x86_64-unknown-linux-musl` or `aarch64-unknown-linux-musl`.
- Existing test boundary:
  - `crates/opentk-config/tests/config_loading.rs` already verifies repository-level Docker/Compose/docs/script contracts by reading public files directly.
  - Continue that pattern for this task. Do not add production Rust DTOs for Dockerfiles or build scripts.

## Public Interfaces To Add Or Change

- Add `docker/Dockerfile.scratch` as the single multi-arch scratch image interface:
  - accepts `BINARY=opentk-sync` or `BINARY=opentk-api`.
  - accepts `ARTIFACT_IMAGE`, defaulting to a local task-specific artifact tag.
  - has a simple final runtime path: map Docker `TARGETARCH` to the artifact path, copy `/artifacts/<target-triple>/<binary>` into the fixed image path `/bin/opentk`, copy CA certs and minimal identity files, then run from `scratch` as uid/gid `1000`.
  - final stage must not compile Rust. Its job is selecting the right prebuilt artifact and assembling the scratch filesystem.
  - Use `ENTRYPOINT ["/bin/opentk"]`. Docker JSON-form `ENTRYPOINT` does not expand `ARG` values, so the binary selection must happen at build time when the prebuilt artifact is copied into that stable runtime path.
- Add `docker/Dockerfile.scratch-artifacts` as the artifact builder interface:
  - runs on `BUILDPLATFORM`, not target platform, so `docker buildx --platform linux/amd64,linux/arm64` does not cause QEMU target execution.
  - one artifact build compiles both binaries for both Rust targets:
    - `x86_64-unknown-linux-musl`: `opentk-sync`, `opentk-api`
    - `aarch64-unknown-linux-musl`: `opentk-sync`, `opentk-api`
  - writes artifacts to stable paths under `/artifacts/<target-triple>/<binary>`.
  - copies CA certs and generates minimal `/runtime/etc/passwd` and `/runtime/etc/group` once for reuse by the scratch Dockerfile.
  - uses BuildKit cache mounts for cargo registry, cargo git, and cargo target output from the first build step.
- Update `.cargo/config.toml` with the cross-target build configuration needed by the chosen static cross-compilation toolchain:
  - keep `CARGO_INCREMENTAL = "0"`.
  - add only target-specific linker/rustflag config required for native cross-compilation.
  - do not add host-specific env hacks in application code.
- Add `scripts/docker-buildx-scratch.sh` as the user-facing multi-arch build interface:
  - creates or reuses a buildx builder.
  - first builds the artifact image in one invocation.
  - then builds final multi-platform tags for `opentk-sync` and `opentk-api` from `docker/Dockerfile.scratch`.
  - verifies the host-architecture image runs `--help`.
  - verifies image size is below 50MB for each host-architecture image.
  - verifies the produced image manifest contains both `linux/amd64` and `linux/arm64`.
  - fails loudly on missing Docker/buildx support, missing artifact paths, QEMU/emulation indicators, failed `--help`, or oversized images.
- Add concise docs, preferably `docs/docker-scratch.md`, and link from `README.md`:
  - document `scripts/docker-buildx-scratch.sh`.
  - document direct `docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=...`.
  - describe only static `scratch` runtime images.
  - state that target artifacts are native Rust cross-compilation outputs, not QEMU/emulated target builds.

## TDD / Verification Plan

Use vertical slices. Do not batch all tests before implementation.

1. RED tracer: run the direct scratch build command for `opentk-sync`.
   - Command: `docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=opentk-sync .`
   - Expected first failure: `docker/Dockerfile.scratch` or artifact source does not exist.
   - GREEN: add the smallest `docker/Dockerfile.scratch` skeleton and artifact Dockerfile wiring needed for the command to resolve stages and fail only on the next missing build concern.

2. RED: add one repository contract test through existing public file-reading style.
   - Place it in `crates/opentk-config/tests/config_loading.rs` unless that file becomes too muddy; if it does, create a new integration test in `crates/opentk-config/tests/docker_scratch.rs`.
   - Test observable repository contract:
     - `docker/Dockerfile.scratch` exists and contains `FROM scratch`.
     - it accepts `ARG BINARY`, maps `TARGETARCH`, copies from an artifact image/stage into `/bin/opentk`, sets `USER 1000:1000`, and has a fixed JSON-form `ENTRYPOINT ["/bin/opentk"]` without a shell wrapper.
     - final stage contains no `cargo build`, package install, shell, or runtime distro.
     - `docker/Dockerfile.scratch-artifacts` builds both expected binaries for both expected Rust targets.
     - artifact Dockerfile uses `BUILDPLATFORM`, BuildKit cache mounts, and stable `/artifacts/...` output paths.
     - `scripts/docker-buildx-scratch.sh` exists, has `set -euo pipefail`, invokes one artifact build, builds both final images, verifies `--help`, checks image size, and inspects both target platforms.
   - GREEN: implement the minimal Dockerfiles/script text to satisfy the file-contract behavior.

3. RED: run the artifact build alone.
   - Command: `docker buildx build -f docker/Dockerfile.scratch-artifacts -t opentk-scratch-artifacts:local --load .`
   - Expected failure should expose the next concrete cross toolchain issue, not a missing file.
   - GREEN: install/use the chosen cross-compilation toolchain and `.cargo/config.toml` target settings until the artifact image contains all four binaries.
   - If this reveals that the planned target/linker strategy cannot produce static `aarch64-unknown-linux-musl` without changing dependency features or binary behavior, switch this plan and task back to `TO BE VERIFIED` and stop.

4. RED: run the direct multi-platform scratch build for `opentk-sync`.
   - GREEN: complete artifact path selection and final `scratch` copy behavior until the command succeeds.
   - Verify host-arch `docker run --rm <sync tag> --help` works and image is under 50MB.

5. RED: run the same direct multi-platform scratch build for `opentk-api`.
   - GREEN: make the generic `BINARY` handling correct without duplicating Dockerfiles.
   - Verify host-arch `docker run --rm <api tag> --help` works and image is under 50MB.

6. RED: run `scripts/docker-buildx-scratch.sh`.
   - GREEN: finish builder creation, artifact build, final build, manifest inspection, host-arch run checks, and image-size checks.
   - Keep this script strict; no swallowed Docker, manifest, or verification failures.

7. Documentation slice:
   - Add or update docs and README link.
   - Keep wording scoped to static scratch runtime images and native cross-compilation. Do not discuss runtime distro/package compatibility.

8. Required end checks:
   - `make check`
   - `make test`
   - `make lint`
   - Do not run `make test-long` unless this task text changes to say the story is ending or long/e2e validation is required.

## Code Boundary Checks

Apply `$improve-code-boundaries` during execution:

- Replace duplicated per-binary scratch assembly with one generic `docker/Dockerfile.scratch` for the multi-arch path.
- Keep artifact compilation in `docker/Dockerfile.scratch-artifacts`; keep final runtime assembly in `docker/Dockerfile.scratch`.
- Do not put Docker build orchestration into Rust production crates.
- Do not create Rust structs/enums solely to parse Dockerfiles; repository contract tests can read public files directly.
- Avoid stringly shell sprawl by centralizing the binary/target mapping in the buildx script and Dockerfile case statements.
- Keep runtime naming stable: the selected binary may vary by `BINARY`, but the scratch image should always execute `/bin/opentk`.
- Fail on every unexpected Docker/buildx/cargo condition. If script logic needs optional behavior, make the condition explicit and reported.
- If the old `docker/Dockerfile.api` and `docker/Dockerfile.sync` become misleading after this task, either leave them only for the earlier local single-arch interface or remove/redirect them as part of a deliberate boundary cleanup. Do not maintain three divergent scratch recipes.

## Acceptance Mapping

- Direct buildx command succeeds for `opentk-sync`:
  - `docker/Dockerfile.scratch` plus artifact image supports `BINARY=opentk-sync` and both target platforms.
- Direct buildx command succeeds for `opentk-api`:
  - same generic Dockerfile supports `BINARY=opentk-api`.
- Scratch images run `--help` on host architecture:
  - script verifies both final tags with `docker run --rm ... --help`.
- Scratch images under 50MB:
  - script checks host-architecture image sizes and fails above threshold.
- `scripts/docker-buildx-scratch.sh` builds both images:
  - script is the public automation interface.
- One artifact build creates all four binaries:
  - artifact Dockerfile compiles two Rust targets times two binaries before final image assembly.
- Final multi-platform tags contain correct platform images:
  - final Dockerfile copies by `TARGETARCH`; script inspects manifests for `linux/amd64` and `linux/arm64`.
- Native cross-compilation, no QEMU/emulation:
  - artifact build runs on `BUILDPLATFORM` and compiles Rust target triples directly; script reports/fails on emulation indicators.
- Task text and docs describe only static scratch runtime images:
  - docs stay scoped to static scratch, CA certs, minimal identity files, and non-root execution.
- `make check`, `make test`, `make lint`:
  - run after implementation and before marking the task passing.

NOW EXECUTE
