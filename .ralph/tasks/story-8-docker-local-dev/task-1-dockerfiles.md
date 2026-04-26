## Task: Story 8 Task 1 - Dockerfiles for Local Development <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create production-ready Dockerfiles for local development. The project currently has zero Docker support.

Requirements:
- Multi-stage builds with BuildKit cache mounts for cargo registry and target
- Builder stage: `rust:bookworm`
- Runtime stage: `debian:bookworm-slim` (not alpine, to avoid musl issues)
- One Dockerfile per binary:
  - `docker/Dockerfile.sync` → `opentk-sync` binary
  - `docker/Dockerfile.api` → `opentk-api` binary (inference)
- Shared builder strategy: copy workspace Cargo.toml + all crate Cargo.toml first, dummy build to cache deps, then copy source and build real binary
- Runtime creates non-root user (uid 1000, `opentk`) and installs `ca-certificates`
- Include `.dockerignore` excluding target, .git, .ralph
- Include `scripts/docker-build.sh` for building all images

In scope: Dockerfiles, .dockerignore, build script. Out of scope: docker-compose, multi-arch.

</description>


<acceptance_criteria>
- [ ] `docker/Dockerfile.sync` builds and runs `opentk-sync --help`
- [ ] `docker/Dockerfile.api` builds and runs `opentk-api --help`
- [ ] BuildKit cache mounts used for cargo registry and target
- [ ] Runtime images use debian:bookworm-slim with non-root user
- [ ] `scripts/docker-build.sh` builds both images
- [ ] `.dockerignore` properly excludes unnecessary files
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
