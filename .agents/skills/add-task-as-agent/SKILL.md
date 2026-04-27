---
name: add-task-as-agent
description: Create a task when the AGENT (Claude) needs to create one. Agents should always use THIS skill, not add-task-as-user.
---

## Purpose

This skill creates **focused, tasks** from completed research. Use your extensive research/subagent explore findings to define clear, concrete tasks.

## Prerequisites

- Research/exploration phase has identified what needs to be done
- You understand the scope and can break it into independent pieces

## Where to create

- Tasks go in the same story dir as the research task: `.ralph/tasks/story-storyname/`
- Use descriptive slugs that reflect the goal: `task-convert-config-parsing.md`

## Task file format

For any code related task TDD is mandatory.
For non-code tasks such as Dockerfiles, workflows, documentation, naming, or other file/text manipulation, do not require `make test` or Rust text-assert tests; require manual verification that the thing works instead, such as a successful Docker build or checking authenticated GitHub workflow logs with `github-api-curl`.
For those tasks TDD is NOT allowed. And must not have make test/test-long as acceptance criterea and must have this manual verify step as criterium.

```markdown
## Task: [Clear Goal Description] <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** [multiple sentences stating the objective]
[also include the higher order goal of this task]
[Complete Discussed goal, things in-scope, out of scope, all decisions made]
[Must include ALL things discussed within the chat/context. You must verify that the task alone, stands on its own, without having any additional context beside this repo only (e.g. you can refer to files)]


</description>


<acceptance_criteria>
- [ ]  [each test to be red/greenTDD-ed at minimum]
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
```
