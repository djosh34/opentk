## Bug: GitHub Token Lacks Workflow Scope For Docker Verification <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
Story 09 Task 02 requires pushing `.github/workflows/docker.yml`, triggering `workflow_dispatch`, and verifying Docker workflow cache behavior/timings from real GitHub logs using `/home/joshazimullah.linux/github-api-curl`.

This is blocked because `git push origin master` was rejected by GitHub:

`refusing to allow a Personal Access Token to create or update workflow .github/workflows/docker.yml without workflow scope`

The fallback checks also failed: SSH authentication to `git@github.com` was denied and `gh auth status` is not configured. Without a credential that can update workflow files, GitHub reports zero Actions workflows for `djosh34/opentk`, so the Docker workflow cannot be dispatched or log-verified.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `git push origin master` can push commits that create or update `.github/workflows/docker.yml`
- [ ] `/home/joshazimullah.linux/github-api-curl -sS -H 'Accept: application/vnd.github+json' https://api.github.com/repos/djosh34/opentk/actions/workflows` lists the Docker workflow after push
- [ ] Story 09 Task 02 can trigger or inspect a real Docker workflow run and verify cache/timing logs
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
