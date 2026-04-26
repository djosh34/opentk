## Task: Story 9 Task 2 - GitHub Workflow for Multi-Arch Docker Build <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Automate building and pushing the multi-arch scratch Docker images on pushes to `main` and tags.

Requirements:
- `.github/workflows/docker.yml`
- Trigger on `main` pushes, `v*` tags, `workflow_dispatch`
- `docker/setup-buildx-action@v3` with cross-compilation
- `docker/login-action@v3` to `ghcr.io` via `GITHUB_TOKEN`
- Build `opentk-sync` and `opentk-api` scratch images for `linux/amd64,linux/arm64`
- Tags: `latest`, `sha-{short_sha}` for main; `:{tag}` for releases
- `cache-from`/`cache-to` with `type=gha` (GitHub Actions cache backend)
- Builder stage shared across both images to avoid rebuilding deps twice

In scope: docker workflow, buildx, registry auth, tagging, cache. Out of scope: Helm, K8s.

</description>


<acceptance_criteria>
- [ ] `.github/workflows/docker.yml` is valid syntax
- [ ] Builds and pushes `ghcr.io/{owner}/opentk-sync` and `opentk-api`
- [ ] Both linux/amd64 and linux/arm64
- [ ] Tags: latest, sha, version tags
- [ ] Build cache uses `type=gha`
- [ ] `workflow_dispatch` works for testing
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
