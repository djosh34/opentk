## Task: Story 09 Task 02 - GitHub Workflow for Multi-Arch Docker Build <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Automate building multi-arch scratch Docker images on pushes to `main` and tags, with image build and image publication split so publish failures are isolated from build failures.

Requirements:
- `.github/workflows/docker.yml`
- Trigger on `main` pushes, `v*` tags, `workflow_dispatch`
- Configure buildx for native cross-compilation, never QEMU/emulation
- Authenticate to `ghcr.io` only in the publish job
- Build `opentk-sync` and `opentk-api` scratch images for `linux/amd64,linux/arm64`
- Tags: `latest`, `sha-{short_sha}` for main; `:{tag}` for releases
- Use heavy caching immediately from the first workflow version. Do not defer optimization to a later task.
- Do not prescribe a specific cache backend in this task; instead verify from real GitHub workflow logs that cargo dependencies, target artifacts, Docker layers, and final image assembly reuse cache effectively.
- One buildx artifact build must produce both target-architecture binaries from one invoking architecture (`amd64` or `arm64`) without emulation.
- Later scratch-image assembly must be simple per-arch copy of the correct artifact into `scratch`, producing one multi-platform tag per binary.
- Split workflow jobs:
  - Build job creates and verifies image artifacts/manifests without pushing to GHCR.
  - Publish job consumes the already-built result and pushes to GHCR.
  - A publish/auth/GHCR failure must not be reported as an image build failure.

Verification requirement: use the `github-api-auth-wrapper` skill (`/home/joshazimullah.linux/github-api-curl`) to inspect real workflow runs. Confirm cache hits and timings with GitHub logs. A cached Docker build taking over 5 minutes, rebuilding the same Rust dependencies twice for test and image work, or rebuilding the same image again just to publish is a task failure.

In scope: docker workflow, buildx, registry auth, tagging, cache, GHCR publish split. Out of scope: Helm, K8s.

</description>


<acceptance_criteria>
- [ ] `.github/workflows/docker.yml` is valid syntax
- [ ] Builds `opentk-sync` and `opentk-api` scratch images before any GHCR publish step
- [ ] Publishes `ghcr.io/{owner}/opentk-sync` and `opentk-api` in a separate publish job
- [ ] Both linux/amd64 and linux/arm64
- [ ] Tags: latest, sha, version tags
- [ ] Cache behavior and build timing are verified from real GitHub logs using `github-api-auth-wrapper`
- [ ] Cached Docker workflow completes in under 5 minutes
- [ ] Test/build/image jobs reuse the same cache and do not rebuild Rust dependencies twice
- [ ] `arm64` builds are native cross-compiled and never emulated
- [ ] `workflow_dispatch` works for testing
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
