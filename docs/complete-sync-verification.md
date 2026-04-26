# Complete Sync Verification

`opentk-db::sync_verification` is the PostgreSQL readiness boundary for a
complete SyncFeed import. It verifies the database through generated schema
metadata and direct SQL-visible state; it does not keep a second list of
official entities, fields, or relations.

## Deterministic Verification

Use `complete-sync verify` against a migrated database:

```bash
complete-sync verify --database-url "$OPENTK_DATABASE_URL"
```

With no `--category` flags, the command expands to every official category from
`opentk-core::official_schema`. With explicit `--category` flags, verification
is limited to those categories:

```bash
complete-sync verify \
  --database-url "$OPENTK_DATABASE_URL" \
  --category Document \
  --required-relation-samples 1
```

The verifier reports:

- category table coverage, current row counts, registry row counts, cursor, and
  durable sync state;
- relation table coverage for generated relation tables, including source and
  target lookup queryability;
- direct SQL probes for HTTP-readiness, including document lookup,
  `Document` to `Kamerstukdossier` joins, registry changes, and category sync
  status;
- table snapshots that can prove reapplying the same page did not duplicate
  current rows, relation rows, registry rows, or cursor rows;
- storage evidence from PostgreSQL catalog size functions.

The deterministic CI tests seed PostgreSQL through the real parser and writer,
then call the same public verifier used by the CLI.

## Live Smoke

The live smoke is an ignored test and runs only in the long lane:

```bash
make test-long
```

It starts the normal local PostgreSQL test harness, fetches a bounded first page
from the real SyncFeed, writes parsed entities into PostgreSQL, and calls
`verify_sync_database`. The default category set is `Document`; widen it with a
comma-separated environment variable:

```bash
OPENTK_LIVE_SYNC_CATEGORIES=Document,Zaak make test-long
```

The default base URL is `https://gegevensmagazijn.tweedekamer.nl`. Override it
with `OPENTK_LIVE_SYNC_BASE_URL` for a compatible SyncFeed endpoint. The client
uses controlled concurrency in its configuration even though the default smoke
keeps the representative run deliberately bounded.

The live smoke is intentionally not part of `make test`; normal tests remain
deterministic and fixture-backed.

## Storage Metrics

`StorageVerification` records:

- `table_bytes`: total relation bytes for generated tables;
- `index_bytes`: total index bytes for generated tables;
- `html_asset_bytes`: currently `0`, because Story 2 stores linked binary
  metadata rather than fetched HTML bodies;
- `binary_asset_metadata_rows`: rows with linked download metadata such as
  enclosure URL, content type, or content length.

The metrics are evidence for read-model footprint and asset metadata coverage,
not a quota or performance budget.

## Boundary Rules

Verification logic belongs in `opentk-db::sync_verification`. The CLI is only
bootstrap and report printing. Tests should verify behavior through parser,
writer, generated schema, and public verifier output rather than duplicating the
official model or asserting private helper details.
