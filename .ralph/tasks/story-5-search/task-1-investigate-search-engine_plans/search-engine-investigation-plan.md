# Story 5 Task 1 Plan: Investigate Search Engine

## Current State

- The workspace has `opentk-core`, `opentk-db`, `opentk-sync`, and `opentk-api`; there is no search crate or production search boundary yet.
- PostgreSQL is the source of truth. Search must index current relational rows plus `document_content.extracted_text` and `document_content.extracted_html`.
- Story 4 already stores document assets, extracted text, stored HTML, provenance, and read-model access. Search quality must use those shapes instead of inventing a second document model.
- Story 5 later tasks need a chosen engine, storage estimate, update-sync expectations, and API result-shape guidance. This task should produce measured evidence and a recommendation, not production indexing.

## Public Interfaces

Add one experimental crate that is explicitly not the production search implementation:

```text
crates/opentk-search-eval/
```

Its public boundary should be small:

```rust
pub struct SearchEvaluationInput {
    pub corpus: Vec<SearchDocument>,
    pub queries: Vec<SearchQueryCase>,
}

pub struct SearchEvaluationReport {
    pub engine: SearchEngine,
    pub index_build_ms: u128,
    pub update_ms: u128,
    pub disk_bytes: u64,
    pub query_results: Vec<QueryResultScore>,
    pub notes: Vec<String>,
}
```

Use one command-line runner:

```text
cargo run -p opentk-search-eval --bin search-eval -- --engine meilisearch --out target/search-eval/meilisearch.json
cargo run -p opentk-search-eval --bin search-eval -- --engine tantivy --out target/search-eval/tantivy.json
```

Keep engine adapters inside the evaluation crate:

```text
src/engine/meilisearch.rs
src/engine/tantivy.rs
```

Do not add search code to `opentk-api` or `opentk-db` in this task. Later tasks can promote the chosen production boundary with cleaner names.

## Representative Data

Create a deterministic benchmark corpus under:

```text
crates/opentk-search-eval/fixtures/search_corpus.json
crates/opentk-search-eval/fixtures/search_queries.json
```

The corpus must include realistic rows derived from current repo shapes:

- `document` metadata: `source_category`, `source_id`, title/subject-like fields, document number, date, URL.
- document content: extracted text and stored HTML samples, including phrases from parliamentary documents.
- entity metadata: person names, party/fractie labels, activities, dossiers, and relation-like labels.
- update samples: one changed title/body and one deleted/replaced record.

The query set must include:

- fuzzy typo queries, for example misspelled person names and document titles;
- exact known-document queries, including document numbers and exact phrases;
- document text queries against extracted text;
- HTML/content queries where the relevant text only appears in stored HTML;
- entity metadata queries for person/fractie/activity/dossier labels;
- mixed queries combining metadata and body terms.

Each query case must state expected top results and acceptable lower-ranked relevant results. Result quality should be scored with simple, readable metrics: expected top hit, top-3 recall, typo tolerance, and useful snippet/highlight availability.

## TDD Execution Plan

Follow vertical red-green cycles. Do not write all tests first.

- [x] RED 1: Add a test in `opentk-search-eval` that loads `search_corpus.json` and `search_queries.json` and fails when either file is missing, empty, malformed, or lacks every required query category.
- [x] GREEN 1: Add the minimal fixture schema, parser, fixture files, and validation needed to pass. Keep validation errors explicit; do not ignore malformed inputs.
- [x] RED 2: Add a test that requires committed evaluation result files for at least two engines under `crates/opentk-search-eval/fixtures/results/`, with matching engine names, matching query ids, positive build/update timings, positive disk bytes, and quality scores for every query.
- [x] GREEN 2: Add the result schema and placeholder generation path only after proving the test fails. Replace placeholders with real measured output before final checks.
- [x] RED 3: Add a behavior test for the shared scoring code: exact top hit scores higher than top-3-only relevance, typo queries must record typo-tolerance evidence, and missing expected results fail the evaluation.
- [x] GREEN 3: Implement engine-neutral scoring in the evaluation crate.
- [x] RED 4: Add a focused test for update measurement input: a changed/deleted document fixture must produce an update case that every engine result must report.
- [x] GREEN 4: Implement the update-case fixture model and report validation.
- [x] RED 5: Add a documentation validation test that fails unless `docs/search-engine-investigation.md` references both measured result files, names the recommended engine, and records disk/build/update/quality tradeoffs.
- [x] GREEN 5: Write the measured recommendation document after experiments are complete.

## Experiment Plan

Evaluate at least these two viable engines against the same corpus and queries:

- Meilisearch: preferred candidate, strong built-in typo tolerance, ranking rules, snippets/highlights, HTTP API, operational service.
- Tantivy: Rust-native embedded index, strong lexical/BM25 search, lower operational complexity, likely more custom work for fuzzy behavior and snippets.

If a candidate cannot be run locally after a reasonable attempt, record that as an explicit failure in the recommendation and add a third viable candidate, such as PostgreSQL full-text plus `pg_trgm`, so acceptance still has two measured engines.

For each engine:

- Build a fresh index in `target/search-eval/<engine>/`.
- Measure wall-clock index build time with `std::time::Instant`.
- Measure disk usage by recursively summing the index data directory.
- Run the same query set and record ordered result ids, scores/ranks, snippets/highlights if available, and quality metrics.
- Apply the same update fixture: changed text/title plus delete or replacement. Measure update time and verify the post-update query result changes.
- Record operational complexity: extra service, binary/container requirement, API/client maturity, failure modes, sync semantics, and how hard Task 3 will be.

## Boundary Review

Use the `improve-code-boundaries` skill before final verification:

- Keep benchmark fixture parsing, scoring, and experimental engine adapters in `opentk-search-eval`.
- Keep production DB/read/API crates unchanged unless a tiny shared type is genuinely needed. This task should not leak benchmark-only DTOs into production code.
- Do not duplicate official schema types. The evaluator may use a flattened `SearchDocument` fixture shape because it is an experiment artifact, not production schema.
- Delete any unused candidate adapter or placeholder result files before final checks.
- If experiment results show the planned `SearchDocument`/query/report types cannot express important ranking or update evidence, switch the task plan back to `TO BE VERIFIED` and stop.

## Documentation

Add:

```text
docs/search-engine-investigation.md
```

The document must include:

- corpus and query-set summary;
- result table for each engine with disk usage, build time, update time, total quality score, typo score, exact score, document-text score, entity-metadata score, and mixed-query score;
- operational tradeoffs;
- chosen engine recommendation;
- implications for Story 5 Tasks 2-4.

## Verification

Run, in order:

- [x] `cargo fmt`
- [x] `make check`
- [x] `make lint`
- [x] `make test`

Do not run `make test-long` for this task unless the task file is changed to require long/e2e validation. This is not the story-ending validation task.

After all checks pass:

- [x] Update acceptance boxes in `task-1-investigate-search-engine.md`.
- [x] Set `<passes>true</passes>`.
- [ ] Run `/bin/bash .ralph/task_switch.sh`.
- [ ] Add all files, including `.ralph` updates, commit with `task finished task-1-investigate-search-engine: ...`, push, and quit immediately.

NOW EXECUTE
