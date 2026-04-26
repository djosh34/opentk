# Complete Sync Verification

`opentk-db::sync_verification` is the PostgreSQL readiness boundary for a
complete SyncFeed import. It verifies the database through generated schema
metadata and direct SQL-visible state; it does not keep a second list of
official entities, fields, or relations.

## Deterministic Verification

Use `opentk-sync verify` against a migrated database:

```bash
opentk-sync --config ./opentk.toml verify
```

The categories come from the unified config. With no configured override, the
command expands to every official category from `opentk-core::official_schema`:

```bash
opentk-sync --config ./opentk.toml verify \
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

Document-content verification is also fixture-backed by default. The deep HTTP
API tests fetch controlled PDF, DOCX, HTML, and official-text alternative
fixtures through the same asset fetcher used in production, persist the
`document_asset` and `document_content` rows, and compare PostgreSQL row values
to `GET /documents/{source_id}/content`. Supported fixture output is exact:
one word mismatch changes the extraction report to `failed`/`invalid`.

`make test-long` includes an ignored controlled document-content smoke. When
`OPENTK_LIVE_DOCUMENT_ASSET_URL` is set, it fetches that live asset and records
the selected source URL, content types, official-source rank, source/output
hashes, extraction tool/version, and extracted byte counts. Without the
environment variable it uses a deterministic local text fallback so the long
lane remains selectable in isolated environments.

## Storage Metrics

`StorageVerification` records:

- `table_bytes`: total relation bytes for generated tables;
- `index_bytes`: total index bytes for generated tables;
- `link_metadata_bytes`: approximate bytes used by durable document asset link
  and retrieval metadata;
- `extracted_text_bytes`: stored extracted text bytes in `document_content`;
- `stored_html_bytes`: stored extracted HTML bytes in `document_content`;
- `constraint_count`: generated table constraints present in PostgreSQL;
- `binary_asset_metadata_rows`: rows with linked download metadata such as
  enclosure URL, content type, or content length;
- `document_content_rows`: stored content extraction/provenance rows.

The metrics are evidence for read-model footprint and asset metadata coverage,
not a quota or performance budget.

## Boundary Rules

Verification logic belongs in `opentk-db::sync_verification`. The CLI is only
bootstrap and report printing. Tests should verify behavior through parser,
writer, generated schema, and public verifier output rather than duplicating the
official model or asserting private helper details.
