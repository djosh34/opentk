# Search API Plan

## Goal

Expose fuzzy search through the HTTP API without leaking Meilisearch response JSON across the API boundary. Clients should be able to call one endpoint, receive stable result DTOs, and follow `source_url`/API URLs to fetch the full document or entity.

## Boundary Design

- Add a query-side search boundary to `opentk-search`, next to the existing indexing boundary.
- Keep `MeilisearchClient` as the concrete HTTP adapter for both indexing and querying.
- Add a small public trait for search reads, for example:

```rust
#[allow(async_fn_in_trait)]
pub trait SearchQueryClient {
    async fn search(&self, request: SearchRequest) -> Result<SearchResponse, SearchIndexError>;
}
```

- Add stable domain result types in `opentk-search`:

```rust
pub struct SearchRequest {
    pub query: String,
    pub limit: u32,
    pub offset: u32,
    pub filter: Option<SearchFilter>,
}

pub struct SearchResponse {
    pub query: String,
    pub limit: u32,
    pub offset: u32,
    pub estimated_total_hits: Option<u32>,
    pub results: Vec<SearchResult>,
}

pub struct SearchResult {
    pub key: String,
    pub source_category: String,
    pub source_id: uuid::Uuid,
    pub entity_kind: SearchEntityKind,
    pub title: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub date: Option<String>,
    pub document_number: Option<String>,
    pub snippets: Vec<SearchSnippet>,
    pub ranking_score: Option<f64>,
    pub api_url: String,
}

pub struct SearchSnippet {
    pub field: String,
    pub text: String,
    pub highlighted: Option<String>,
}
```

- Keep Meilisearch-specific raw DTOs private to `opentk-search`. Map `_formatted`, `_rankingScore`, and `estimatedTotalHits` into stable types there.
- Do not expose search engine concepts directly from `opentk-api`; the API should depend on `SearchQueryClient` and translate domain search results into HTTP DTOs only.
- Update `ApiState` to carry a `SearchQueryClient` implementation behind `Arc<dyn ... + Send + Sync>`.
- Preserve `router(pool)` for tests and existing call sites by constructing a default Meilisearch client from environment/config only if that is already available cleanly; otherwise introduce a `router_with_search(pool, search_client)` helper for tests and make server startup wire the real client.
- Add API configuration for search service URL, API key, and index name if not already present. This is greenfield, so do not add legacy compatibility shims.

## Endpoint Contract

- Add `GET /search`.
- Query parameters:
  - `q`: required non-empty search text.
  - `limit`: optional, default `20`, accepted range `1..=100`.
  - `offset`: optional, default `0`.
  - `category`: optional exact source category filter.
  - `entity_kind`: optional exact entity-kind filter.
- Response:

```json
{
  "query": "kamerbriev",
  "limit": 20,
  "offset": 0,
  "estimated_total_hits": 3,
  "items": [
    {
      "key": "Document:11111111-1111-4111-8111-111111111111",
      "source_category": "Document",
      "source_id": "11111111-1111-4111-8111-111111111111",
      "entity_kind": "Document",
      "title": "Fixture document",
      "summary": "Read endpoint",
      "source_url": "https://example.test/document.pdf",
      "api_url": "/documents/11111111-1111-4111-8111-111111111111",
      "date": "2026-04-26T00:00:00Z",
      "document_number": "2026D00001",
      "snippets": [
        {
          "field": "extracted_text",
          "text": "plain snippet",
          "highlighted": "plain <em>snippet</em>"
        }
      ],
      "ranking_score": 0.98
    }
  ]
}
```

- Error behavior:
  - Invalid/empty `q`, invalid `limit`, or invalid `offset` returns `400 invalid_request`.
  - Search backend request/response errors return `503 search_unavailable` with no swallowed error.

## TDD Execution Plan

Execute in vertical red-green slices. Do not write all tests first.

1. RED: Add one router-level test in `crates/opentk-api/tests/cases/search_api.rs` using a fake `SearchQueryClient`. Test `GET /search?q=fixture&limit=2` returns a document result with `source_id`, `entity_kind`, `title`, `api_url`, snippets, and ranking score.
2. GREEN: Add the API route, request parsing, response DTOs, state injection, fake-test constructor, and minimal mapping needed for the document result.
3. RED: Add a second router-level test for an entity metadata result, likely a `Persoon`, and assert `api_url` points at `/persons/{source_id}` while generic/untyped categories point at `/entities/{category}/{source_id}`.
4. GREEN: Implement URL derivation and entity-kind/category response mapping without duplicating read-model DTOs.
5. RED: Add a search-client test in `crates/opentk-search/tests/meilisearch_search.rs` with the existing lightweight HTTP fixture pattern. It should assert typo/fuzzy queries are sent to Meilisearch with `q`, `limit`, `offset`, `attributesToHighlight`, `attributesToCrop`, `showRankingScore`, and return mapped snippets from `_formatted`.
6. GREEN: Implement `SearchQueryClient for MeilisearchClient`, private Meilisearch search DTOs, snippet extraction, ranking score mapping, and explicit invalid-response errors.
7. RED: Add an API test for query validation: missing/blank `q`, `limit=0`, and `limit=101` return `400`.
8. GREEN: Add `SearchQuery` validation and keep all failures explicit via `ApiError`.
9. RED: Add an API test where the fake search client returns a backend error and assert `503 search_unavailable`.
10. GREEN: Add `ApiError::SearchUnavailable` or equivalent; do not collapse search failures into generic internal errors.
11. RED: Extend `crates/opentk-api/tests/openapi.rs` to assert `/search`, `q`, `limit`, `offset`, `category`, `entity_kind`, `SearchResponse`, `SearchResult`, and `SearchSnippet` appear in OpenAPI.
12. GREEN: Add OpenAPI path and component schemas.
13. REFACTOR with `improve-code-boundaries`: split the growing API module if it becomes muddy. Preferred boundary is `crates/opentk-api/src/search.rs` for HTTP query/DTO/handler code and private Meilisearch DTOs staying in `opentk-search`.
14. Run `make check`, `make lint`, and `make test`. Do not run `make test-long` for this non-story-finishing task unless the task scope changes to require it.

## Acceptance Checklist Mapping

- Search endpoint returns document results: covered by first router-level test.
- Search endpoint returns entity metadata results: covered by second router-level test.
- Fuzzy typo queries return relevant results: covered at the Meilisearch client boundary by asserting typo query request/response mapping; ranking quality itself was selected in prior story tasks.
- Response includes follow-up data: `source_id`, `source_category`, `source_url`, and `api_url` assertions.
- OpenAPI spec: explicit OpenAPI test before implementation.
- Error handling and pagination: validation and backend-error tests.

## Notes

- Keep tests behavior-oriented and public-interface based.
- Use fake search clients only at the API state boundary; do not mock internal helper functions.
- Preserve explicit error propagation. Any unavoidable swallowed error must become a bug task, but the planned implementation should not need that.
- No legacy compatibility is required.

NOW EXECUTE
