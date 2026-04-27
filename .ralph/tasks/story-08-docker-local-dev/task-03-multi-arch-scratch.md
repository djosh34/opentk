## Task: Story 08 Task 03 - Multi-Arch Scratch Dockerfiles <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a multi-arch build flow that compiles static `linux/amd64` and `linux/arm64` Rust binaries without emulation and produces minimal scratch images.

Requirements:
- One buildx invocation must build an artifact image containing both target binaries for both binaries (`opentk-sync` and `opentk-api`) and both target architectures.
- A later, much simpler per-binary/per-arch scratch Dockerfile stage must copy the correct prebuilt artifact into `scratch`.
- The final output is one multi-platform tag per binary, where each platform image contains the correct binary for that platform.
- The architecture that invokes buildx may be `amd64` or `arm64`, and must be able to build both target artifacts.
- Verify that `arm64` is never built through QEMU/emulation. Emulated target builds are a hard task failure.
- Use BuildKit cache mounts for cargo registry and target output from the start.
- Final stage is `scratch` with only: binary, CA certs, minimal identity files needed for non-root execution, and writable paths the app actually requires.
- `scripts/docker-buildx-scratch.sh` automates buildx creation, artifact build, final image assembly, and local verification.

These are scratch images containing static binaries; runtime distro/package compatibility discussion is out of scope.

In scope: scratch Dockerfile, cargo cross config, buildx script. Out of scope: GitHub Actions automation.

</description>


<acceptance_criteria>
- [ ] `docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=opentk-sync` succeeds
- [ ] Same for `opentk-api`
- [ ] Scratch images run `--help` on host architecture
- [ ] Scratch images under 50MB each
- [ ] `scripts/docker-buildx-scratch.sh` builds both images
- [ ] One buildx artifact build creates both `amd64` and `arm64` binaries before final image assembly
- [ ] Final multi-platform tags contain one platform image per architecture with the correct binary copied from artifacts
- [ ] Builds use native cross-compilation, not QEMU or any emulation
- [ ] Task text and produced docs describe only static scratch runtime images
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
