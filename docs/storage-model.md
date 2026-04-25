# Storage Model

## Baseline

The importer parses SyncFeed XML and writes directly into database tables in the same transaction that advances the category cursor.

The baseline write path is:

```text
fetch feed page
parse Atom envelope
parse embedded entity XML
write relational rows
write relationship rows
write document asset metadata
advance category cursor
commit
```

This makes the database the structured store for the system.

## Why Direct Relational Storage

Direct relational ingestion is the cleanest storage shape:

- fields that the API filters/sorts/joins on become columns,
- relations become rows in relation tables,
- cursor state is transactional,
- database inspection remains useful,
- `sqlx` migrations describe the data model explicitly,
- storage stays columnar, relational, and inspectable.

The parser still needs a short-lived in-memory representation so code is clean:

```text
ParsedEntry
  feed metadata
  entity metadata
  typed entity enum
  references
  enclosure metadata
```

That struct is an implementation detail used by the importer.

## Direct Tables

Tables are designed from API needs and SyncFeed semantics.

Core tables:

- category cursor and status,
- ingest errors,
- current entity metadata,
- entity relations,
- document asset metadata,
- typed entity tables for the selected categories.

Typed tables start with the categories needed by the API and expand when an endpoint needs more data.

Every typed row should include source metadata:

- source category,
- source entity id,
- latest skiptoken,
- deleted flag,
- source update timestamp,
- Atom update timestamp.

## Handling Schema Changes

Schema changes are handled through explicit relational changes:

1. Add a typed column when a scalar field has API value.
2. Add a narrow side table for repeated or sparse fields.
3. Add a relation row when the field is a reference.
4. Add an ingest error when the parser sees a structure that requires a schema decision.

For newly discovered fields that need temporary retention, use a relational spill table:

```sql
create table entity_extra_field (
  category text,
  entity_id text,
  skiptoken integer,
  field_path text,
  ordinal integer,
  value_text text,
  ref_id text,
  primary key (category, entity_id, skiptoken, field_path, ordinal)
);
```

This is still structured data. It can be inspected, queried, and migrated into typed columns later.

## Page Refetch

Fetched pages are applied directly. A crash before commit simply causes the next run to fetch the same page again from the durable category cursor.

The critical invariant:

```text
cursor moves after relational writes commit
```

Tracking tables:

```sql
create table sync_category (
  category text primary key,
  cursor_skiptoken integer,
  state text,
  caught_up_at text,
  last_fetch_started_at text,
  last_fetch_finished_at text,
  last_entry_updated_at text
);

create table ingest_error (
  id integer primary key,
  category text,
  skiptoken integer,
  entity_id text,
  happened_at text,
  phase text,
  message text
);
```

## Database Shape

The database decision is centered on two viable shapes:

- SQLite with explicit relational tables.
- PostgreSQL with relational tables plus JSONB for source fields that are still settling.

SQLite keeps the system compact and easy to run. PostgreSQL gives stronger semi-structured storage through JSONB and a better fit for a hosted API service.
