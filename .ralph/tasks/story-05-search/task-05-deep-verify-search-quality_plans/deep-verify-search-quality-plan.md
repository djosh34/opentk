# Story 05 Task 05 Plan: Deep Verify Search Quality

## Current State

- Story 05 selected Meilisearch and already has production search boundaries in `opentk-search`, PostgreSQL-to-index orchestration in `opentk-db::search_sync`, and HTTP exposure in `opentk-api::search`.
- `opentk-search-eval` currently validates a tiny committed fixture corpus, fixture query categories, candidate quality scores, update behavior, build timing, update timing, and disk bytes.
- The current evaluator proves the scoring model works, but Task 05 needs a stronger verification harness that judges whether the production search stack is good enough on representative synced data, not just whether small fixtures are functional.
- This is the deep verification task for Story 05. Treat it as story-end validation only after the implementation itself is complete and the default checks are green.

## Public Interface Design

Keep the verification harness in `opentk-search-eval`; do not move benchmark DTOs into `opentk-api` or `opentk-db`.

Add a production-quality verification input and report model:

```rust
pub struct SearchQualityBenchmark {
    pub corpus: Vec<SearchBenchmarkDocument>,
    pub queries: Vec<SearchBenchmarkQuery>,
    pub update_case: SearchBenchmarkUpdate,
    pub thresholds: SearchQualityThresholds,
}

pub struct SearchBenchmarkDocument {
    pub id: String,
    pub source_category: String,
    pub source_id: uuid::Uuid,
    pub entity_kind: opentk_search::SearchEntityKind,
    pub title: String,
    pub summary: Option<String>,
    pub document_number: Option<String>,
    pub source_url: Option<String>,
    pub date: Option<String>,
    pub extracted_text: Option<String>,
    pub extracted_html: Option<String>,
    pub metadata_text: Vec<String>,
    pub relation_labels: Vec<String>,
}

pub struct SearchBenchmarkQuery {
    pub id: String,
    pub category: QueryCategory,
    pub query: String,
    pub expected_top: String,
    pub acceptable_results: Vec<String>,
    pub required_terms: Vec<String>,
}

pub struct SearchQualityThresholds {
    pub min_query_score: f64,
    pub min_total_score: f64,
    pub max_p95_latency_ms: u128,
    pub max_disk_bytes_per_record: u64,
}

pub struct SearchQualityReport {
    pub record_count: usize,
    pub query_results: Vec<QueryResultScore>,
    pub latency: SearchLatencyReport,
    pub disk: SearchDiskUsageReport,
    pub update_result: UpdateResult,
    pub endpoint_result: EndpointVerificationReport,
}
```

Add a deep verification runner around the existing scoring concepts:

```rust
pub fn run_quality_benchmark(
    benchmark: &SearchQualityBenchmark,
    index_dir: &std::path::Path,
) -> Result<SearchQualityReport, SearchEvaluationError>;

pub fn validate_quality_report(
    benchmark: &SearchQualityBenchmark,
    report: &SearchQualityReport,
) -> Result<(), SearchEvaluationError>;
```

Add optional adapters only where they cross a real boundary:

- `SearchBenchmarkDocument::to_source_record()` maps committed benchmark documents into production `opentk_search::SearchSourceRecord`.
- `SearchQualityReport` remains evaluator-owned; the API returns the existing stable `SearchResponseDto`, not benchmark reports.
- If DB-backed benchmark generation is needed, put it behind a separate `opentk-db` helper that emits evaluator input data. Do not make `opentk-search-eval` depend on private DB internals unless the workspace already permits that dependency cleanly.

## Benchmark Data

Replace the tiny fixture-only shape with a richer committed benchmark sample that is still fast enough for `make test`:

- At least three known documents with exact document numbers and unique title/body phrases.
- At least two document-text cases where the expected terms appear only in extracted text or extracted HTML.
- At least two person/entity cases using metadata and relation labels, including a party/fractie or committee-style relation.
- At least one fuzzy typo query that must rank the expected person or document first.
- At least one mixed query combining entity metadata and body/title terms.
- One update case that changes an indexed document, one delete case that removes a previous hit, and one added/updated term that must become the top hit after propagation.

Use UUID-backed source ids in fixtures so the same documents can be mapped through production search result types and HTTP endpoint tests.

## TDD Execution Plan

Use vertical red-green cycles. Do not write all tests first. Each red test must exercise public behavior through `opentk-search-eval`, `opentk-search`, `opentk-db`, or `opentk-api`.

- [ ] RED 1: Add an evaluator test proving a report fails when an expected top result is missing or ranked below the accepted threshold.
- [ ] GREEN 1: Add `SearchQualityThresholds`, per-query threshold validation, and total-score validation on top of the existing `QueryResultScore` behavior.
- [ ] RED 2: Add a fixture validation test proving the benchmark must include fuzzy typo, exact known-document, person/entity metadata, document text, HTML content, and mixed query categories.
- [ ] GREEN 2: Add the richer benchmark fixture format and validate required categories, expected ids, acceptable ids, required terms, and non-empty document content/metadata coverage.
- [ ] RED 3: Add a benchmark runner test proving representative queries measure latency and fail when p95 latency exceeds the configured threshold.
- [ ] GREEN 3: Record per-query latency, calculate p95 deterministically, and validate it against `max_p95_latency_ms`.
- [ ] RED 4: Add a disk-usage test proving index bytes and bytes-per-record are reported and fail when over `max_disk_bytes_per_record`.
- [ ] GREEN 4: Measure the generated index directory, derive bytes per record, include it in `SearchDiskUsageReport`, and validate the threshold explicitly.
- [ ] RED 5: Add an update propagation test proving an update changes the top result and a delete removes the old result from the ranked output.
- [ ] GREEN 5: Extend the runner update path to apply update/delete operations through the same index model used for search quality scoring.
- [ ] RED 6: Add a production-boundary mapping test proving benchmark documents become `SearchSourceRecord`/`SearchIndexOperation` values with document text, HTML, metadata text, relation labels, entity kind, source id, and deterministic keys.
- [ ] GREEN 6: Implement the mapper without copying API DTOs into the evaluator. Keep Meilisearch/private HTTP DTOs out of this layer.
- [ ] RED 7: Add an HTTP endpoint verification test using `opentk-api::router_with_search` and a benchmark-backed fake `SearchQueryClient`. Assert `/search` returns the expected top ids and snippets for at least one exact query, one fuzzy query, and one entity metadata query.
- [ ] GREEN 7: Add a small evaluator helper that can replay benchmark results through the public `SearchQueryClient` shape, then use it in API tests without making the API know about benchmark DTOs.
- [ ] RED 8: Add a regression test proving committed benchmark report/result fixtures cannot be stale: lowering scores, omitting latency, omitting disk usage, or breaking update results makes validation fail.
- [ ] GREEN 8: Commit regenerated benchmark reports and strict validation of all required report sections.
- [ ] RED 9: Add CLI behavior coverage for `search-eval` so it can run the deep benchmark and write a full report with quality, latency, disk, and update sections.
- [ ] GREEN 9: Extend `crates/opentk-search-eval/src/bin/search-eval.rs` with a deep-quality mode or subcommand while preserving explicit parse/write errors.

Run the targeted test for the current slice after every GREEN step before adding the next RED test.

## Boundary Review With `improve-code-boundaries`

Before final verification:

- Keep benchmark/scoring/report concerns in `opentk-search-eval`.
- Keep production index/query DTOs in `opentk-search`.
- Keep API endpoint response verification in `opentk-api` tests, using only the public `SearchQueryClient` interface.
- Do not reuse evaluator DTOs as API DTOs or database DTOs.
- Delete or merge any duplicate search document shapes that only rename fields and do not protect a real boundary.
- If the evaluator grows a pile of stringly report validation, create a small deep module around `SearchQualityReport` validation instead of scattering checks across tests.
- If benchmark-to-production mapping duplicates existing `map_record_to_operation` logic, route through that public mapper rather than reimplementing ranking/index document construction.

## Verification

Run, in order:

- [ ] `cargo fmt`
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`

Because this is the final deep verification task for Story 05, run `make test-long` only after the default checks pass and only if the repository's current task list confirms this task is finishing the entire story or the implementation explicitly changes long/e2e test selection.

After checks pass:

- [ ] Update acceptance boxes in `task-05-deep-verify-search-quality.md`.
- [ ] Set `<passes>true</passes>`.
- [ ] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Commit all changes, including `.ralph`, with `task finished task-05-deep-verify-search-quality: deep verify search quality` and include test evidence/challenges in the commit body.
- [ ] Push with `git push`.

NOW EXECUTE
