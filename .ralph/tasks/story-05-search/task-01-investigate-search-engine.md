## Task: Story 05 Task 01 - Investigate Search Engine With Current Repo State <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Investigate which search engine is best and easiest to implement for the current repository state and synced PostgreSQL data. The task must run real searches against representative data and judge result quality. Better search quality is more important than pure simplicity. Meilisearch is a preferred candidate, but disk usage must be measured and compared.

The task must evaluate fuzzy search quality, typo tolerance, ranking, indexing speed, disk usage, operational complexity, update sync behavior, and API integration. Candidate engines may include Meilisearch and other maintained engines that fit Rust/PostgreSQL integration and instant fuzzy search requirements.

In scope: research, local experiments, sample indexed data, benchmark queries, quality scoring, disk measurements, update behavior, and a written recommendation committed to the repo. Out of scope: final production implementation.

</description>


<acceptance_criteria>
- [x] Red/green TDD: add a repeatable search-evaluation harness that fails on missing benchmark inputs/results, then make it pass.
- [x] At least two viable search engines are evaluated against the same representative query set.
- [x] Evaluation includes fuzzy/typo queries, exact queries, document text queries, entity metadata queries, and mixed queries.
- [x] Evaluation records disk usage, index build time, update time, and result quality.
- [x] Recommendation explains the chosen engine and tradeoffs using measured data.
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): not applicable; this task did not change long/e2e selection and `make test-long` was intentionally not run.
</acceptance_criteria>

<plan>
.ralph/tasks/story-05-search/task-01-investigate-search-engine_plans/search-engine-investigation-plan.md
</plan>

NOW EXECUTE
