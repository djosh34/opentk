## Task: Story 2 Task 3 - Implement SyncFeed Client and Rate Limit Respect <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the HTTP SyncFeed client against the official SyncFeed API with exact cursor handling, retries, timeout classification, response validation, and bounded concurrency. The client must fetch pages by category, follow feed-level `next` links, detect `resume`, expose response timing, and report every error explicitly.

The importer applies pages directly and refetches after crashes based on the durable category cursor. The client must make refetch cheap and safe by treating requests as idempotent. Parallelism must happen across categories and within safe local processing boundaries while respecting observed API limits through adaptive global throttling.

In scope: `reqwest` client, timeouts, retry policy, adaptive limiter, response status handling, XML content-type checks, cursor URL parsing, `next`/`resume` validation, metrics/log fields, and local mock-server tests. Out of scope: writing all entity tables.

</description>


<acceptance_criteria>
- [x] Red/green TDD: mock-server test fails on wrong `next`/`resume` handling, then passes.
- [x] Client follows category cursor pages until `resume`.
- [x] Client records and exposes request latency and status for limiter decisions.
- [x] Non-2xx, malformed cursor URL, missing cursor state, timeout, and invalid XML response are explicit errors.
- [x] Global concurrency is bounded and configurable.
- [x] Tests prove category workers can fetch in parallel while per-category cursor order is preserved.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): not applicable; this task does not finish the story or change long/e2e selection.
</acceptance_criteria>

.ralph/tasks/story-2-complete-sync-postgres/task-3-implement-syncfeed-client-and-rate-limits_plans/syncfeed-client-plan.md

NOW EXECUTE
