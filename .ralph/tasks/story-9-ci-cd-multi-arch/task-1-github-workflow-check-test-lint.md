## Task: Story 9 Task 1 - GitHub Workflow for Check, Test, and Lint <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create the first GitHub Actions workflow since the project has zero CI. Automate `make check`, `make test`, and `make test-long`.

Requirements:
- `.github/workflows/ci.yml`
- Fast lint job (`make check`) on PRs for quick feedback
- Test job with PostgreSQL 16 service container
- `make test-long` only on `main` pushes or `workflow_dispatch`
- Cache cargo aggressively via `Swatinem/rust-cache@v2`
- Matrix: stable Rust required, nightly allowed-to-fail
- Set `OPENTK_TEST_DATABASE_URL` to service container

In scope: CI workflow, caching, service container. Out of scope: release automation, Docker CI.

</description>


<acceptance_criteria>
- [ ] `.github/workflows/ci.yml` is valid syntax
- [ ] `make check` runs on PRs and fails on lint errors
- [ ] `make test` runs with PostgreSQL service
- [ ] `make test-long` runs on main but not PRs
- [ ] Cargo cached between runs
- [ ] Workflow completes in under 10 minutes
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
