## Task: Story 08 Task 01 - Dockerfiles for Local Development <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create production-ready Dockerfiles for local development that build static Rust binaries and run them in `scratch` runtime images. The project currently has zero Docker support.

Requirements:
- Multi-stage builds with BuildKit cache mounts for cargo registry and target
- Builder stage may use an appropriate Rust build image, but the runtime stage must be `scratch`
- Runtime stage contains only the binary, CA certificates, minimal `/etc/passwd`/`/etc/group` entries needed for a non-root user, and writable paths that the app actually requires
- One Dockerfile per binary:
  - `docker/Dockerfile.sync` → `opentk-sync` binary
  - `docker/Dockerfile.api` → `opentk-api` binary (inference)
- Shared builder strategy: copy workspace Cargo.toml + all crate Cargo.toml first, dummy build to cache deps, then copy source and build real binary
- Runtime runs as non-root uid 1000 (`opentk`) and includes CA certificates copied from the builder or a certificate source stage
- Include `.dockerignore` excluding target, .git, .ralph
- Include `scripts/docker-build.sh` for building all images

These are static scratch binaries; runtime distro/package compatibility discussion is out of scope.

In scope: scratch Dockerfiles, .dockerignore, build script. Out of scope: docker-compose, multi-arch.

</description>


<acceptance_criteria>
- [x] `docker/Dockerfile.sync` builds and runs `opentk-sync --help`
- [x] `docker/Dockerfile.api` builds and runs `opentk-api --help`
- [x] BuildKit cache mounts used for cargo registry and target
- [x] Runtime images use `scratch` and run as non-root uid 1000
- [x] Dockerfiles and docs describe only static scratch runtime images
- [x] `scripts/docker-build.sh` builds both images
- [x] `.dockerignore` properly excludes unnecessary files
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite)
- [x] `make lint` — passes cleanly
</acceptance_criteria>

<plan>
.ralph/tasks/story-08-docker-local-dev/task-01-dockerfiles_plans/dockerfiles-local-dev-plan.md
</plan>

DONE
