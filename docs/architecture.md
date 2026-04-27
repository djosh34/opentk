# Architecture

## Goal

Build a fast Rust HTTP API over Tweede Kamer SyncFeed data. The system should run locally, ingest reliably, survive long pauses, and catch up to the current feed automatically.

## Recommended stack

- Rust workspace.
- `tokio` for async runtime.
- `reqwest` for HTTP with connection pooling, timeouts, retry classification, and tracing.
- `quick-xml` for strict streaming XML parsing.
- `axum` for the HTTP API.
- `sqlx` for database access; binaries own schema creation or validation at startup.
- `tracing` and `tracing-subscriber` for structured logs.
- `clap` for importer/admin commands.
- `insta` or fixture-based tests for XML parser contracts.

## Data flow

```text
SyncFeed XML
  -> fetch page for category cursor
  -> parse Atom envelope and embedded entity XML
  -> build short-lived typed parse structs
  -> in one transaction:
       update typed projections
       update entity relationships
       advance category cursor
  -> API reads typed projections and source metadata
```

## Ingestion model

The feed is cursor-driven by `skiptoken`. Within one category, the importer follows feed-level `next` links until the feed returns `resume`. Across categories, workers can run concurrently.

Recommended importer shape:

- One worker per active category, bounded by global concurrency.
- Each worker fetches one page, parses it, writes one transaction, then follows the next cursor.
- A global adaptive limiter reacts to latency, HTTP status, and error rate.
- Non-2xx responses, malformed XML, unexpected namespaces, missing entity IDs, missing cursor state, and failed projection writes are hard errors.
- The cursor advances after projections, relationships, and asset metadata commit.
- The same worker supports initial backfill, delayed catch-up, and live polling.

## Storage Model

Use direct relational storage. XML is parsed into short-lived Rust structs, then written into typed tables and relation tables.

## API shape

Principles:

- Cursor pagination only.
- Stable resource identifiers are source UUIDs.
- List responses include source metadata.
- Deletions are visible through change endpoints.
- Document asset endpoints return upstream links for binary assets and stored bodies for HTML assets.

Initial endpoints:

```text
GET /health
GET /meta/categories
GET /changes?category=Document&after_skiptoken=...
GET /entities/{category}/{id}
GET /documents
GET /documents/{id}
GET /documents/{id}/asset
GET /activities
GET /activities/{id}
GET /persons
GET /persons/{id}
```

Example document response shape:

```json
{
  "id": "uuid",
  "nummer": "2024D00000",
  "soort": "Brief regering",
  "titel": "...",
  "onderwerp": "...",
  "datum": "2024-01-01",
  "source": {
    "category": "Document",
    "skiptoken": 20000000,
    "updated_at": "2024-08-07T12:06:44.6847096Z"
  },
  "asset": {
    "url": "https://gegevensmagazijn.tweedekamer.nl/...",
    "content_type": "application/pdf",
    "content_length": 123456,
    "stored": false
  }
}
```

## Consistency model

- API reads the last fully committed state.
- Import commits one page at a time per category.
- Change endpoints expose `skiptoken` so clients can resume.
- During backfill, the API can return partial data with category cursor progress.

This keeps the API usable while a long import or catch-up is running.
