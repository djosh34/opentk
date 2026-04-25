## Task: Story 1 Task 2 - Add Developer Commands and CI-Ready Checks <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add the repeatable commands that every later task depends on. The repo must expose `make check`, `make test`, `make lint`, and `make test-long` with strict behavior and no hidden skips. The commands must run formatting, linting, tests, and database migration validation where applicable.

This task must preserve the project rule that linter failures and skipped tests are unacceptable. If a required tool is missing, the command must fail clearly with installation guidance rather than silently skipping.

In scope: Makefile or equivalent command runner, local environment documentation, CI-compatible command definitions, and a small smoke test proving the commands execute. Out of scope: full CI provider configuration unless the repo already has an established CI convention.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: add a test or scripted assertion that validates the command runner exposes all required targets.
- [ ] `make check` runs formatting checks, lint checks, tests, and migration validation hooks that exist at this stage.
- [ ] `make test` runs the default test suite.
- [ ] `make lint` fails on warnings.
- [ ] `make test-long` exists and is documented even if it initially has no ultra-long tests.
- [ ] Missing required tools produce clear failures.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
