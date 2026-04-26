## Task: Story 8 Task 1 - Dockerfiles for Local Development <status>not_started</status> <passes>false</passes>

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
- [ ] `docker/Dockerfile.sync` builds and runs `opentk-sync --help`
- [ ] `docker/Dockerfile.api` builds and runs `opentk-api --help`
- [ ] BuildKit cache mounts used for cargo registry and target
- [ ] Runtime images use `scratch` and run as non-root uid 1000
- [ ] Dockerfiles and docs describe only static scratch runtime images
- [ ] `scripts/docker-build.sh` builds both images
- [ ] `.dockerignore` properly excludes unnecessary files
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
