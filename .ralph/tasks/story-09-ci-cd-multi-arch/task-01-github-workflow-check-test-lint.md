## Task: Story 09 Task 01 - GitHub Workflow for Check, Test, and Lint <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create the first GitHub Actions workflow since the project has zero CI. Automate `make check`, `make test`, and `make test-long`.

Requirements:
- `.github/workflows/ci.yml`
- Fast lint job (`make check`) on PRs for quick feedback
- Test job with PostgreSQL 16 service container
- `make test-long` only on `main` pushes or `workflow_dispatch`
- Use aggressive caching from the first implementation, but do not bake in a specific cache action or key strategy in this task text. The implementer must choose, measure, and prove the chosen approach works.
- Matrix: stable Rust required, nightly allowed-to-fail
- Provide test database configuration through the test harness/config-file mechanism, not application single-setting env vars

Verification requirement: use the `github-api-auth-wrapper` skill (`/home/joshazimullah.linux/github-api-curl`) to inspect real workflow runs. Confirm from logs/timing that cache restore/save is happening and the workflow is quick. A normal cached check/test/lint run taking over 5 minutes is a task failure.

In scope: CI workflow, caching, service container, measured cache verification. Out of scope: release automation, Docker CI.

</description>


<acceptance_criteria>
- [ ] `.github/workflows/ci.yml` is valid syntax
- [ ] `make check` runs on PRs and fails on lint errors
- [ ] `make test` runs with PostgreSQL service
- [ ] `make test-long` runs on main but not PRs
- [ ] Cargo cached between runs
- [ ] `github-api-auth-wrapper` is used to verify real GitHub workflow cache behavior and timing
- [ ] Cached check/test/lint workflow completes in under 5 minutes
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
