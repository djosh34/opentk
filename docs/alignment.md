# Alignment

## Main Concerns

- Total storage cost, including linked document metadata and stored HTML assets.
- Syncing during development through exact category cursor tracking and cheap page refetch.
- Database shape for evolving XML fields.

## Current Decisions

- Use Rust.
- Use `sqlx`.
- Use SyncFeed XML as the source.
- Apply fetched pages directly.
- Advance category cursors in the same transaction as data writes.
- Store binary document assets as upstream links and metadata.
- Store HTML assets in the database.

## Open Decisions

- SQLite relational storage or PostgreSQL relational storage with JSONB sidecars.
- First typed table set.
- Parser strictness for newly observed XML fields.
- Category import order.
- API endpoint details.
