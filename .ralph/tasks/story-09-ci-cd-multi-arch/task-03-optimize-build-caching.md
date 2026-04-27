## Task: Story 09 Task 03 - Prove Build Caching and Layer Reuse <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Prove that the cache strategy implemented in Tasks 01 and 2 actually works. Optimization must already be present in the initial CI and Docker workflow tasks; this task is for measurement, verification, and tightening failures.

Requirements:
1. Measure and document cold build time vs warm build time.
2. Verify from real GitHub logs that check/test jobs and Docker image builds reuse caches instead of recompiling the workspace independently.
3. Verify the image build and GHCR publish split does not rebuild images in the publish phase.
4. Verify one invoking architecture can build both target artifacts without QEMU/emulation.
5. Document the chosen strategy in `docs/build-caching.md` without prescribing future maintainers to one cache vendor/action unless the implementation truly depends on it.

Use the `github-api-auth-wrapper` skill (`/home/joshazimullah.linux/github-api-curl`) to inspect workflow logs and timings.

Targets: cached check/test/lint workflow under 5 minutes, cached Docker build workflow under 5 minutes, and no duplicate full Rust dependency build between test and image work. Slowness is failure.

In scope: cache verification, CI/Docker cache tightening, build time measurement, docs. Out of scope: distributed caching services.

</description>


<acceptance_criteria>
- [ ] Single buildx artifact build produces both target architectures without emulation
- [ ] Cold and warm build times measured and documented
- [ ] Cached check/test/lint workflow is under 5 minutes
- [ ] Cached Docker build workflow is under 5 minutes
- [ ] Publish job does not rebuild images
- [ ] Test and image workflows do not duplicate full Rust dependency builds
- [ ] Real GitHub logs and timings were inspected using `github-api-auth-wrapper`
- [ ] `docs/build-caching.md` explains the strategy
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
