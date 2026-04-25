## Task: Story 5 Task 5 - Deep Verify Search Quality <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Prove search quality, speed, update correctness, and disk usage meet the project goals. Verification must use real synced data, document text, entity metadata, typo/fuzzy queries, exact queries, and known expected results. The task must judge whether search is genuinely good, not merely functional.

In scope: quality benchmark harness, expected result sets, fuzzy typo tests, latency measurements, disk usage measurements, update propagation tests, endpoint tests, and regression thresholds. Out of scope: changing the selected engine without updating the investigation recommendation.

</description>


<acceptance_criteria>
- [ ] Red/green TDD: quality benchmark fails when expected top results are missing or poorly ranked, then passes.
- [ ] Benchmark includes fuzzy typo queries, exact known-document queries, person/entity queries, and document text queries.
- [ ] Search latency is measured and thresholded for representative queries.
- [ ] Disk usage is measured and reported.
- [ ] Update propagation from PostgreSQL to search index is tested.
- [ ] HTTP search endpoint results are checked against the benchmark expectations.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
