## Task: Story 8 Task 3 - Multi-Arch Scratch Dockerfiles <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create an alternative Dockerfile that cross-compiles for both `linux/amd64` and `linux/arm64` and produces minimal scratch images.

Requirements:
- Single `docker/Dockerfile.scratch` with cross-compilation toolchains
- Builder installs `gcc-x86-64-linux-gnu`, `gcc-aarch64-linux-gnu`, cross libc dev packages
- Configure Cargo for cross-compile via `.cargo/config.toml`
- Build both target architectures; use BuildKit cache mounts
- Final stage is `scratch` with only: binary, CA certs, /tmp directory
- Single Dockerfile produces different images via `--build-arg BINARY=opentk-sync` or `opentk-api`
- `scripts/docker-buildx-scratch.sh` automates buildx creation and multi-platform build

In scope: scratch Dockerfile, cargo cross config, buildx script. Out of scope: GitHub Actions automation.

</description>


<acceptance_criteria>
- [ ] `docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=opentk-sync` succeeds
- [ ] Same for `opentk-api`
- [ ] Scratch images run `--help` on host architecture
- [ ] Scratch images under 50MB each
- [ ] `scripts/docker-buildx-scratch.sh` builds both images
- [ ] Cross-compilation uses native toolchains, not QEMU
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
