## Task: Story 1 Task 3 - Deep Verify Rust Setup End To End <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Prove the project setup is actually usable from a clean checkout state. This verification task must exercise all setup documentation, workspace commands, and local test/lint flows as a user would run them before starting importer development.

In scope: run the documented setup path, verify all required commands, verify workspace tests and linting, verify no task-critical command silently skips. Record any setup mismatch as a failing test or bug task rather than ignoring it.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: add an automated setup verification script or test that fails before required setup assumptions are encoded and passes after.
- [ ] Fresh local setup instructions are executable as written.
- [ ] All workspace crates build.
- [ ] No generated setup artifact is missing from documentation.
- [ ] Command output clearly reports failures.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
