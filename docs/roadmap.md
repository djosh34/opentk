# Roadmap

## Phase 0: Design and API contract

- Decide typed table boundaries for the initial API.
- Decide category list for the first importer milestone.
- Create XML fixture samples from live SyncFeed responses.

Exit criteria:

- Architecture and storage model are accepted.
- API contract has enough detail to write integration tests.
- Parser fixtures cover normal entries, deleted entries, references, repeated references, enclosures, and resume feeds.

## Phase 1: Rust workspace and parser

- Create Rust workspace.
- Add `quick-xml` parser for Atom feed envelope.
- Add parser for embedded Tweede Kamer entity XML into typed Rust structs.
- Add fixture tests.

Exit criteria:

- Parser returns the same typed data for saved fixtures every run.
- Parser fails on malformed XML, missing IDs, missing cursor state, and unexpected required structure.

## Phase 2: SQLite schema and single-category ingestion

- Add binary-owned schema setup for category cursor, relationship, asset metadata, ingest errors, and typed tables.
- Add CLI command: `opentk ingest --category Document`.
- Implement page fetch, parse, transaction write, and cursor advance.
- Add integration tests using a local mock server and fixture database.

Exit criteria:

- A single category can be fetched, parsed, stored, projected, stopped, and resumed.
- Cursor only advances after the full page transaction commits.

## Phase 3: Multi-category importer

- Add multiple category workers.
- Add global adaptive limiter.
- Add per-category progress reporting.
- Add durable ingest error table.
- Add import status command.

Exit criteria:

- Multiple categories can backfill concurrently.
- Failures are visible in logs, database status, and non-zero CLI exit status.
- Long-delayed catch-up uses the same path as live polling.

## Phase 4: HTTP API

- Add `axum` API process.
- Implement health and metadata endpoints.
- Implement changes endpoint.
- Implement document list/detail/source/asset endpoints.
- Implement cursor pagination.
- Add API integration tests against a fixture database.

Exit criteria:

- API can serve while importer is stopped or running.
- API contract is documented with examples.

## Phase 5: HTML asset handling

- Implement asset metadata classification.
- Fetch and store HTML assets only.
- Keep binary document assets link-only.
- Add tests proving binary assets are represented as links.

Exit criteria:

- HTML assets are stored in DB only when content type is HTML.
- Binary assets are linked.

## Phase 6: Operational hardening

- Add status output for ingestion lag and category cursors.
- Add database maintenance command.
- Add backup/export guidance.
- Add end-to-end import smoke test.
- Add performance benchmarks for parsing, writes, and common API queries.

Exit criteria:

- Fresh setup is reproducible.
- Import can resume after interruption.
- CI runs formatting, linting, schema checks, and tests.
