# Search Engine Investigation

Story 5 Task 1 evaluated search behavior with a repeatable harness in `crates/opentk-search-eval`. The harness indexes the same fixture corpus for both candidate engines, runs the same query set, applies the same update case, and validates committed result files:

- `crates/opentk-search-eval/fixtures/results/meilisearch.json`
- `crates/opentk-search-eval/fixtures/results/tantivy.json`

## Corpus And Queries

The benchmark corpus has five representative records shaped like the current sync/read model: three document records with title, source id, document number, URL, extracted text, and stored HTML; one person record with member/fractie/activity metadata; and one dossier record with committee and activity metadata.

The query set covers:

- fuzzy typo: misspelled person name plus a subject term;
- exact: a parliamentary document number;
- document text: terms only present in extracted text;
- HTML content: terms only present in stored HTML;
- entity metadata: committee, dossier, and activity labels;
- mixed: party/fractie metadata combined with body/title terms.

The update fixture changes `doc-2024-29810`, deletes `doc-2024-18725`, then verifies that a new update-only term returns the changed record and the deleted record no longer appears for its old query.

## Results

| Candidate | Disk | Build | Update | Total quality | Typo | Exact | Document text | Entity metadata | Mixed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Meilisearch | 3,579 bytes | 1 ms | 1 ms | 6.00 / 6.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| Tantivy | 3,579 bytes | 1 ms | 1 ms | 5.80 / 6.00 | 0.80 | 1.00 | 1.00 | 1.00 | 1.00 |

Both candidates handled exact document-number lookup, extracted text, stored HTML, metadata, mixed queries, and update behavior. The difference was fuzzy ranking quality. Meilisearch returned `person-fatima-kaya` first for `Fatma Kaija digitalisering` and recorded typo evidence: `fatma` matched `fatima`. Tantivy recovered a relevant result through the exact term `digitalisering`, but the misspelled person was second, so its typo score was lower.

The tiny disk and timing numbers are expected for the small committed fixture. They are still useful because the harness measures the same dimensions later production work must preserve: index build time, update time, disk footprint, ranked results, snippets/highlights, and quality scores. Task 2 should scale this corpus with a larger local sample before sizing production storage.

## Operational Tradeoffs

Meilisearch has the better built-in search product shape for this project: typo tolerance, ranking rules, snippets/highlights, faceting/filtering, and a simple HTTP API. The tradeoff is operational complexity. Production will need a Meilisearch service, index lifecycle management, health checks, task/error handling for async indexing, disk sizing, and sync retry behavior.

Tantivy has a strong Rust-native embedded story and avoids a separate service. The tradeoff is product behavior. Fuzzy person/title search, snippet generation, highlighting, filter semantics, and API ergonomics would need more custom code. That custom work would move search quality risk into this repository instead of using a search engine that already exposes those behaviors.

## Recommendation

Recommendation: choose Meilisearch for Story 5.

Search quality is more important than pure simplicity for this story, and the measured fuzzy query is the one place where the candidates diverged. Meilisearch produced the expected top hit for every benchmark query, including the typo case. Tantivy is still viable if operational constraints become dominant, but choosing it now would require building and maintaining typo tolerance and result presentation code that Meilisearch already provides.

## Story 5 Tasks 2-4

Task 2 should design a production search boundary around Meilisearch concepts without exposing benchmark DTOs: document identity, indexable title/body/HTML/metadata fields, update/delete events, and search result snippets.

Task 3 should implement sync-to-index updates as explicit write operations with error reporting. Failed indexing must not be swallowed; it should surface as a task or operational error.

Task 4 should expose API search through the production boundary, not through the evaluator. Keep result shape stable: id, source category, title, URL, matched fields/snippets, and enough provenance for users to navigate back to synced PostgreSQL data.
