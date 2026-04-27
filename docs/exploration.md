# Exploration

Date: 2026-04-25

## What matters from the reference project

The reference project proves that the Tweede Kamer SyncFeed can be mirrored into a local database and queried efficiently. It also shows the main trap for this rebuild: storing full feed XML and then deriving a second database takes on too much data and makes ingestion ergonomics worse than they need to be.

The rebuild keeps the useful source insight with a cleaner storage shape:

- SyncFeed XML is the authoritative source.
- The importer should parse XML immediately into relational rows.
- The database should store API-oriented tables and relations directly.
- The importer should support initial backfill, long-delayed catch-up, and live polling through the same cursor loop.

## Live API observations

SyncFeed base:

```text
https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0/Feed?category=Document
```

Observed behavior:

- Responses are Atom XML with embedded Tweede Kamer XML payloads under `<content type="application/xml">`.
- Feed pages contain 250 entries.
- Feed-level `rel="next"` points to the next page when more entries exist.
- Feed-level `rel="resume"` appears when a high or caught-up `skiptoken` returns zero entries.
- Entry-level links can also carry a skiptoken.
- Deleted rows are represented by the embedded entity carrying `tk:verwijderd="true"`.
- Entity update time appears both in Atom `<updated>` and as `tk:bijgewerkt` on the embedded entity.
- XML namespaces must be handled explicitly.

Measured samples from this machine:

| Request | Result |
| --- | --- |
| `Document` first page | 200, 250 entries, about 189 KB |
| `Activiteit` first page | 200, 250 entries, about 191 KB |
| `Document&skiptoken=10000000` | 200, 250 entries, first observed entry updated 2021-03-04 |
| `Document&skiptoken=20000000` | 200, 250 entries, first observed entry updated 2024-08-07 |
| `Document&skiptoken=30000000` | 200, 0 entries, feed-level resume |

Parallel category fetch probe:

| Category | Status | Time | Size |
| --- | ---: | ---: | ---: |
| Document | 200 | 2.27s | 189 KB |
| Stemming | 200 | 4.07s | 212 KB |
| Activiteit | 200 | 14.91s | 191 KB |
| Zaak | 200 | 15.19s | 186 KB |
| Persoon | 200 | 20.59s | 325 KB |

This supports category-level parallelism with bounded fan-out. Some categories are much slower despite similar payload sizes, so the importer needs adaptive concurrency, request timing, and explicit retry/backoff.

## Document storage policy

For now:

- Store source URL, metadata, content type, length, and retrieval/check status for PDF, DOC, DOCX, RTF, ODT, and similar binary document assets.
- If an attached document is HTML, store the HTML payload in the database.
- Keep enough metadata to add document fetching or text extraction later while preserving the entity model.

The API exposes upstream links for binary documents during this phase.

## Current design constraints

- Rust implementation.
- `sqlx` for database access.
- SQLite is acceptable as the initial database.
- Direct relational ingestion.
- API-focused schemas.
- Page refetch based on durable category cursors.
