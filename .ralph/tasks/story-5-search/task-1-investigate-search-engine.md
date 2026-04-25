## Task: Story 5 Task 1 - Investigate Search Engine With Current Repo State <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Investigate which search engine is best and easiest to implement for the current repository state and synced PostgreSQL data. The task must run real searches against representative data and judge result quality. Better search quality is more important than pure simplicity. Meilisearch is a preferred candidate, but disk usage must be measured and compared.

The task must evaluate fuzzy search quality, typo tolerance, ranking, indexing speed, disk usage, operational complexity, update sync behavior, and API integration. Candidate engines may include Meilisearch and other maintained engines that fit Rust/PostgreSQL integration and instant fuzzy search requirements.

In scope: research, local experiments, sample indexed data, benchmark queries, quality scoring, disk measurements, update behavior, and a written recommendation committed to the repo. Out of scope: final production implementation.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: add a repeatable search-evaluation harness that fails on missing benchmark inputs/results, then make it pass.
- [ ] At least two viable search engines are evaluated against the same representative query set.
- [ ] Evaluation includes fuzzy/typo queries, exact queries, document text queries, entity metadata queries, and mixed queries.
- [ ] Evaluation records disk usage, index build time, update time, and result quality.
- [ ] Recommendation explains the chosen engine and tradeoffs using measured data.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
