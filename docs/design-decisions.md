# Design Decisions

## D001: Canonical ingestion source

Status: accepted

Decision: Use Tweede Kamer SyncFeed XML as the canonical source.

Rationale:

- The XML feed contains the complete embedded entity payload.
- The importer needs delete markers, references, enclosures, and namespaces exactly as emitted by SyncFeed.

Consequences:

- The parser must handle Atom and Tweede Kamer XML namespaces explicitly.
- Parser fixtures must cover normal entries, deleted entries, references, repeated references, enclosures, and caught-up resume feeds.

## D002: Binary document storage

Status: accepted

Decision: Store links and metadata for PDF, DOC, DOCX, RTF, ODT, and similar binary document assets. Store HTML assets in the database when encountered.

Rationale:

- The immediate goal is a fast API over metadata and relationships.
- Binary document storage adds disk pressure and operational complexity.
- HTML content is small enough and queryable enough to justify storing early.

Consequences:

- API asset endpoints return upstream links for binary documents.
- A later phase can add binary download or text extraction while preserving the core entity model.

## D003: Database choice

Status: open

Decision: Use `sqlx` for database access and migrations. The primary database is being selected between SQLite relational storage and PostgreSQL relational storage with JSONB for source-adjacent fields.

Rationale:

- `sqlx` keeps migration workflow and query code consistent across importer and API.
- SQLite gives the simplest local deployment.
- PostgreSQL gives first-class JSONB for fields that are still settling and fits a hosted API service.

Risks:

- SQLite requires more up-front relational modeling.
- PostgreSQL adds server operation and a larger deployment surface.

## D004: Ingestion parallelism

Status: proposed

Decision: Parallelize across categories first. A category worker follows feed cursor links in order.

Rationale:

- SyncFeed pagination is cursor-like.
- Page order within a category matters for resume and deletion/update semantics.
- Live probes show category requests can run concurrently but have variable latency.

Consequences:

- Initial speed comes from category-level parallelism, efficient parsing, and page-sized transactions.
- Intra-category work can still pipeline fetch, parse, and write stages.

## D005: Direct relational ingestion

Status: proposed

Decision: Parse SyncFeed XML and write directly into database tables.

Rationale:

- The core data has stable identifiers, references, timestamps, and cursor semantics.
- Direct relational ingestion keeps the database inspectable and queryable.
- Unknown fields can be handled through typed migrations, narrow side tables, relation rows, or explicit ingest errors.

Consequences:

- The parser produces short-lived typed Rust structs.
- Projection writers insert/update typed tables and relation tables.
- Schema changes are handled with `sqlx` migrations.

## D006: Page refetch

Status: accepted

Decision: Apply fetched pages directly. A crash before commit causes the next run to fetch the same page again from the stored category cursor.

Rationale:

- Direct relational writes are simpler and avoid duplicate data.
- Refetching a page is cheap.
- Cursor advancement gives the importer a durable resume point.

Consequences:

- Tracking logic around category cursors must be exact.
- Cursor updates happen in the same transaction as relational writes.

## D007: Sync model

Status: proposed

Decision: Use one durable cursor per category. A worker starts at the stored `skiptoken`, follows feed-level `next` links until `resume`, then marks the category caught up. Live updates and delayed catch-up use the same mechanism.

Rationale:

- The feed provides cursor links.
- A single cursor per category is simple to reason about and supports long downtime followed by catch-up.

Consequences:

- The importer can run continuously or periodically.
- Cursor advancement is transactional with relational writes.
