## Task: Story 02 Task 05 - Implement Parallel Complete Sync Runner <status>done</status> <passes>true</passes>

<plan>
.ralph/tasks/story-02-complete-sync-postgres/task-05-implement-parallel-sync-runner_plans/parallel-sync-runner-plan.md
</plan>

<description>
Must use tdd skill to complete


**Goal:** Build the complete sync runner that coordinates category workers, PostgreSQL transactions, progress tracking, restart behavior, adaptive throttling, and full catch-up to `resume`. The runner must support first-time full sync, interrupted sync resume, delayed catch-up, and continuous polling using the same cursor logic.

Syncing must be parallel across categories while preserving cursor order within each category. Cursor updates must be committed with the relational writes for the page. Development syncing may refetch pages and replace updated rows. Deletions may be recorded as delete markers according to the accepted development semantics, while preserving enough information for downstream queries to understand current state.

In scope: CLI commands, worker orchestration, progress/status output, category selection, resume behavior, error persistence, adaptive throttling integration, and integration tests. Out of scope: HTTP API.

</description>


<acceptance_criteria>
- [x] Red/green TDD: integration test simulates a crash before cursor commit and proves the page is refetched and applied exactly once.
- [x] Integration test simulates a crash after commit and proves the next run starts from the advanced cursor.
- [x] Multiple categories sync concurrently with per-category order preserved.
- [x] Runner reaches `resume` and marks category caught up.
- [x] Runner can resume from stored cursor after process restart.
- [x] Progress/status command reports category cursor, state, lag, last fetch, and errors.
- [x] Error records are durable and include phase, category, skiptoken, entity id when available, and message.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — not impacted; not run per task instructions.
</acceptance_criteria>
