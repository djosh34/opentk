# SyncFeed Client

The `opentk-sync::syncfeed` module is the HTTP boundary for Tweede Kamer
`SyncFeed` reads. It keeps request, cursor, retry, XML validation, and rate
limit behavior out of importer and database code.

## Cursor Rules

The client builds first-page requests as:

```text
/SyncFeed/2.0/Feed?category=<category>&content=internal
```

Resumed cursors must be absolute URLs for the same configured origin, path
`/SyncFeed/2.0/Feed`, matching `category`, a present `skiptoken`, and matching
`content` mode when `content` is present.

For pages with entries, the durable next cursor is the last entry-level
`<link rel="next">`. A feed-level `next` is only a fallback when a page has no
entry cursor. Empty pages are accepted only when they include a valid feed-level
`<link rel="resume">`; that represents the category caught-up state.

## Errors

The client does not default or ignore bad state. It returns explicit errors for:

- invalid base URL or request client construction;
- transport failures and request timeouts;
- non-success statuses that are not retryable;
- retry exhaustion for retryable statuses;
- missing or non-XML content type;
- malformed Atom/XML responses;
- missing entry `next`, missing cursor state, malformed cursor URLs, and category
  mismatches;
- empty pages without `resume`;
- category worker join failures.

## Retry And Limiting

Requests are idempotent `GET`s. The retry policy retries only timeouts and
retryable statuses: `429`, `500`, `502`, `503`, and `504`. Each successful page
exposes `SyncFeedRequestOutcome` with status, latency, attempt count, and whether
the observed status was rate-limited.

Global request concurrency is bounded by one shared semaphore per
`SyncFeedClient`. Cloned clients share the same limiter, so parallel category
workers still respect the configured maximum. A small adaptive pacing delay is
also shared: `429` observations increase delay up to the configured maximum
retry delay, and successful responses reduce it.

## Complete Sync Runner

`opentk-sync::runner` is the ingestion orchestration boundary. It runs one
sequential loop per category and executes those category loops concurrently.
That preserves cursor order inside a category while allowing independent
categories to overlap. The runner streams one page at a time: fetch, parse,
write page plus cursor through the storage trait, then fetch the next cursor.

Crash behavior follows the committed cursor:

- before the storage write commits, no cursor advances, so restart refetches the
  same page;
- after the storage write commits, restart loads the advanced `next_url`;
- after an empty page with `resume`, the category is marked `caught_up`.

The PostgreSQL implementation lives in `opentk-db::sync_state`. It records
durable errors with phase, category, skiptoken, entity id when known, and
message. The `complete-sync status` command reports category state, cursor,
lag, last fetch time, and last error from that store.

## Testing

Tests use a local HTTP fixture server. This task intentionally avoids live
`SyncFeed` calls; story-end validation is the right place for long-running or
external API checks.

Embedded entity payload parsing is documented in
[`docs/syncfeed-parser.md`](syncfeed-parser.md). The Atom client owns page
fetching and cursor validation; `opentk-sync::payload` owns strict parsing of
the XML content inside each entry.
