## Task: Story 9 Task 3 - Optimize Build Caching and Layer Reuse <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Ensure workspace dependencies compile exactly once and are reused across both images.

Optimization strategy:
1. Single multi-target Dockerfile where the builder stage is shared and final images use different `--target`
2. OR: use `docker/build-push-action` with explicit cache keys including Cargo.lock hash
3. In CI, builder stage cached via `type=gha` with scoped cache keys
4. Measure and document: cold build time vs warm build time
5. Document in `docs/build-caching.md`

Target: warm build under 3 minutes for both images combined.

In scope: Dockerfile optimization, CI cache config, build time measurement, docs. Out of scope: distributed caching beyond GHA.

</description>


<acceptance_criteria>
- [ ] Single buildx build produces both images sharing builder layer
- [ ] Cold and warm build times measured and documented
- [ ] Warm build under 3 minutes combined
- [ ] `docs/build-caching.md` explains the strategy
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
