---
name: add-bug
description: Create a bug task file in .ralph/tasks/bugs/. Triggers on "add bug", "create bug", "new bug", "/add-bug".
---

## Where to create

- `mkdir -p .ralph/tasks/bugs/`
- Write the bug file: `.ralph/tasks/bugs/bug-slug.md`

## Bug file format

For any code related task TDD is mandatory. Preferred is to make a RED test that catches the same bug (as was found) first, and then continue with the TDD skill to solve it.
For non-code tasks such as Dockerfiles, workflows, documentation, naming, or other file/text manipulation, do not require `make test` or Rust text-assert tests; require manual verification that the thing works instead, such as a successful Docker build or checking authenticated GitHub workflow logs with `github-api-curl`.
For those tasks TDD is NOT allowed.

```markdown
## Bug: Bug Title <status>not_started</status> <passes>false</passes> <priority>optional: medium|high|ultra high</priority>

<description>
[What is broken and how it was detected.]
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
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
```
