# Story 08 Task 01 Plan: Static Scratch Dockerfiles

## Current Read

- The workspace has no Docker support yet: no `docker/` directory, no `.dockerignore`, and no Docker build script.
- `opentk-sync` is a binary declared by package `opentk-db` at `crates/opentk-db/src/bin/opentk-sync.rs`.
- `opentk-api` is the package/binary in `crates/opentk-api/src/bin/opentk-api.rs`.
- Both CLIs expose `--help` through `clap`, so the runtime smoke contract can be verified without requiring Postgres, Meilisearch, or config files.
- `make check` and `make lint` are the same lint lane: `cargo fmt --check` plus workspace clippy with warnings denied. `make test` runs the default Postgres-backed test script. Do not run `make test-long` for this normal task.

## Public Interface

- New files:
  - `.dockerignore`
  - `docker/Dockerfile.sync`
  - `docker/Dockerfile.api`
  - `scripts/docker-build.sh`
- Public commands:
  - `DOCKER_BUILDKIT=1 docker build -f docker/Dockerfile.sync -t opentk-sync:local .`
  - `docker run --rm opentk-sync:local --help`
  - `DOCKER_BUILDKIT=1 docker build -f docker/Dockerfile.api -t opentk-api:local .`
  - `docker run --rm opentk-api:local --help`
  - `scripts/docker-build.sh`

## TDD Execution Plan

Use the `tdd` skill as the execution method. Because this task adds container build/runtime behavior rather than Rust logic, the red/green tests are the acceptance commands above, not implementation-coupled tests that grep Dockerfile text.

1. RED tracer: run the focused build command for `docker/Dockerfile.sync`; it should fail because the Dockerfile does not exist.
2. GREEN: create the minimal shared multi-stage Dockerfile pattern for `opentk-sync`:
   - builder stage from a Rust image.
   - install `ca-certificates` and musl tooling.
   - `rustup target add x86_64-unknown-linux-musl`.
   - copy workspace `Cargo.toml`, `Cargo.lock`, and every crate `Cargo.toml` before source.
   - create dummy `src/lib.rs` files and dummy bin files needed by packages with binaries.
   - run a dependency-cache build with BuildKit cache mounts for cargo registry/git and target.
   - copy real source and build `cargo build --release --target x86_64-unknown-linux-musl -p opentk-db --bin opentk-sync`.
   - runtime stage is `scratch`, copies only CA certificates, minimal passwd/group, required writable dirs if any are identified, and `/opentk-sync`; `USER 1000:1000`; `ENTRYPOINT ["/opentk-sync"]`.
3. Verify `docker run --rm opentk-sync:local --help` passes. If it fails because the binary needs dynamic loader files, fix the static build instead of adding a runtime distro.
4. RED for API: run the focused build command for `docker/Dockerfile.api`; it should fail before the API Dockerfile exists.
5. GREEN: add `docker/Dockerfile.api` using the same builder/runtime strategy, building `cargo build --release --target x86_64-unknown-linux-musl -p opentk-api --bin opentk-api` and using `ENTRYPOINT ["/opentk-api"]`.
6. Verify `docker run --rm opentk-api:local --help` passes.
7. Add `.dockerignore` excluding `target`, `.git`, `.ralph`, and local/editor/build noise that should not enter Docker build context.
8. Add `scripts/docker-build.sh` as the public convenience interface:
   - strict shell options.
   - sets `DOCKER_BUILDKIT=1` by default.
   - builds both images from repository root.
   - allows tag prefix/namespace override through a small environment variable if useful, but does not add compose or multi-arch behavior.
9. Run `scripts/docker-build.sh`, then rerun both image `--help` checks.
10. Run `make check`, `make test`, and `make lint`.

Per-cycle rule: add one failing acceptance check, make only the minimal Docker/script change needed to pass it, then continue. If execution reveals that static `scratch` runtime requires changing Rust dependency features, binary interfaces, or config behavior beyond the Docker surface, switch this plan and the task file back to `TO BE VERIFIED` and stop.

## Improve Code Boundaries Plan

Use the `improve-code-boundaries` skill during execution and final review:

- Keep Docker concerns in `docker/` and the build orchestration in `scripts/docker-build.sh`; do not spread image commands across Makefile targets unless a later task asks for it.
- Prefer one obvious Docker build interface over duplicate helper scripts.
- Keep the runtime boundary deep and narrow: `scratch` contains only the executable, CA bundle, minimal identity files for `opentk` uid/gid 1000, and any truly required writable paths. Do not add shells, package managers, distroless compatibility layers, or backwards-compatible aliases.
- Avoid duplicating divergent builder behavior between the two Dockerfiles. If duplication becomes error-prone during execution, use Dockerfile comments and identical stage structure, but do not introduce a custom generator unless it removes real complexity.
- Remove obsolete placeholder files or Docker experiments if any are found while executing.

## Acceptance Mapping

- `docker/Dockerfile.sync` builds and runs `opentk-sync --help`: verify with direct `docker build` and `docker run --rm opentk-sync:local --help`.
- `docker/Dockerfile.api` builds and runs `opentk-api --help`: verify with direct `docker build` and `docker run --rm opentk-api:local --help`.
- BuildKit cache mounts used: verify during review that dependency and final cargo builds use `--mount=type=cache` for cargo registry/git and target.
- Runtime images use `scratch` and non-root uid 1000: verify Dockerfiles use `FROM scratch`, copy passwd/group, and set `USER 1000:1000`.
- Static scratch runtime docs only: keep any comments/script output scoped to static scratch images; do not discuss runtime distro compatibility.
- `scripts/docker-build.sh` builds both images: verify by running it.
- `.dockerignore` excludes unnecessary files: verify includes at least `target`, `.git`, and `.ralph`.
- Final gates: run `make check`, `make test`, and `make lint`; do not run `make test-long`.

NOW EXECUTE
