## Task: Story 9 Task 4 - Publish Scratch Images to GHCR <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Publish the already-built scratch images to `ghcr.io` without mixing publish failures into image-build failures.

Requirements:
1. Consume the multi-platform image output from the Docker build workflow instead of rebuilding binaries or rebuilding images.
2. Publish one multi-platform tag per binary:
   - `ghcr.io/{owner}/opentk-sync`
   - `ghcr.io/{owner}/opentk-api`
3. Tags:
   - `latest` on `main`
   - `sha-{short_sha}` on `main`
   - `{tag}` for version tags
4. Authentication and registry interaction must happen only in the publish job.
5. If GHCR auth, permission, registry availability, or manifest push fails, the failure must clearly belong to publishing, not image building.
6. Verify the published manifest points to both `linux/amd64` and `linux/arm64` images and that each platform image contains the correct static binary.
7. Verify publish does not repeat the Rust build, artifact build, or final image assembly.

Use the `github-api-auth-wrapper` skill (`/home/joshazimullah.linux/github-api-curl`) to inspect real workflow logs and timings.

In scope: GHCR auth, manifest/tag publication, publish-only verification, workflow log verification. Out of scope: changing how binaries are compiled.

</description>


<acceptance_criteria>
- [ ] Publish job consumes previously built image output
- [ ] Publish job never rebuilds Rust dependencies, binaries, or scratch images
- [ ] `ghcr.io/{owner}/opentk-sync` is published as one multi-platform image tag
- [ ] `ghcr.io/{owner}/opentk-api` is published as one multi-platform image tag
- [ ] Published manifests include `linux/amd64` and `linux/arm64`
- [ ] Failure to publish is reported separately from failure to build images
- [ ] Real GitHub logs are verified with `github-api-auth-wrapper`
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
