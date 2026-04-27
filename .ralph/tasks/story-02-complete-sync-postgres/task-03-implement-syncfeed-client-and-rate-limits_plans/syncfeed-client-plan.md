## Plan: SyncFeed HTTP Client and Rate Limits

## Sources

- Task file: `.ralph/tasks/story-02-complete-sync-postgres/task-03-implement-syncfeed-client-and-rate-limits.md`
- Official SyncFeed API page: `https://opendata.tweedekamer.nl/documentatie/syncfeed-api`
- `tdd` skill: red/green tracer bullets through public behavior, one behavior at a time.
- `improve-code-boundaries` skill: keep HTTP/cursor/limiter details behind one sync boundary and remove duplicate/stringly shapes.

## Official SyncFeed Rules To Preserve

The official documentation says SyncFeed 2.0 is at `https://gegevensmagazijn.tweedekamer.nl/SyncFeed/2.0`, the feed endpoint is `/Feed`, category filtering uses `category=<entiteitsoort>`, cursoring uses `skiptoken=<aantal>`, embedded entity XML uses `content=internal`, responses are Atom XML, a feed page may contain feed-level `next` and `resume` links, and the durable sync cursor should be based on the `<link rel="next">` value of the last processed `<entry>`.

Task-specific interpretation:

- The client fetches pages idempotently and never mutates durable progress itself.
- A fetched page returns enough typed cursor data for the later importer/runner to commit the page and the durable category cursor atomically.
- A category is caught up when the response has no entries and contains a valid feed-level `resume` link for that same category.
- Feed-level `next` is used only when there are more page results but no entry-level cursor can be advanced from the last entry. When entries exist, the last entry-level `next` is the exact cursor returned to downstream code.

## Boundary Design

Add a new `opentk-sync::syncfeed` module and export it from `crates/opentk-sync/src/lib.rs`.

Public types:

- `SyncFeedClient`
  - Owns a `reqwest::Client`, base URL, retry policy, timeout configuration, and shared limiter.
  - Main methods:
    - `new(config: SyncFeedClientConfig) -> Result<Self, SyncFeedClientError>`
    - `fetch_page(cursor: SyncFeedCursor) -> Result<SyncFeedPage, SyncFeedClientError>`
    - `fetch_category_until_resume(category: impl Into<String>, start: CategoryCursor) -> Result<Vec<SyncFeedPage>, SyncFeedClientError>` for testable end-to-end behavior and for task 05 to replace with streaming/orchestration if needed.
- `SyncFeedClientConfig`
  - `base_url: Url`
  - `content_mode: SyncFeedContentMode`
  - `request_timeout: Duration`
  - `connect_timeout: Duration`
  - `max_retries: u32`
  - `initial_retry_delay: Duration`
  - `max_retry_delay: Duration`
  - `max_concurrent_requests: NonZeroUsize`
- `SyncFeedCursor`
  - `category: String`
  - `url: Url`
  - Constructed only through validated constructors:
    - `SyncFeedCursor::first_page(base_url, category, content_mode)`
    - `SyncFeedCursor::from_url(base_url, category, url)`
  - Validates path is `/SyncFeed/2.0/Feed`, category is present and matches, `skiptoken` is present for resumed cursors, and `content` matches the configured mode when present.
- `CategoryCursor`
  - `Start`
  - `FromNextUrl(Url)`
- `SyncFeedPage`
  - `category: String`
  - `request_url: Url`
  - `entries: Vec<SyncFeedEntry>`
  - `next_request: Option<SyncFeedCursor>`
  - `resume: Option<SyncFeedCursor>`
  - `outcome: SyncFeedRequestOutcome`
  - `raw_xml: String`
- `SyncFeedEntry`
  - `id: String`
  - `category: String`
  - `updated: String`
  - `next: SyncFeedCursor`
  - `content_xml: Option<String>`
  - `enclosure_url: Option<Url>`
- `SyncFeedRequestOutcome`
  - `status: reqwest::StatusCode`
  - `latency: Duration`
  - `attempts: u32`
  - `rate_limited: bool`
- `SyncFeedContentMode`
  - `Internal`
  - `External`
- `SyncFeedClientError`
  - Explicit variants for invalid base URL, request build failure, timeout, HTTP transport, non-2xx status, invalid/missing XML content type, invalid XML, missing cursor state, malformed cursor URL, category mismatch, missing entry next link, invalid resume page, and retry exhaustion.

Keep parser helpers private unless tests prove a public parser is useful. Public tests should call `SyncFeedClient` and inspect typed results.

## Dependency Plan

Update `crates/opentk-sync/Cargo.toml`:

- Add workspace `reqwest`, `tokio`, and `tracing` dependencies.
- Add `quick-xml` or keep `roxmltree` for Atom parsing. Prefer `roxmltree` initially because it is already in `opentk-sync`; validate structure without introducing a second XML parser.
- Add `httpmock` or `wiremock` as a dev-dependency for local HTTP tests. Prefer `wiremock` if it composes cleanly with async tests and can assert request order/counts.

Update workspace dependencies only if a new dev-dependency needs central versioning or `tokio` needs `sync`/`time` features for `Semaphore`, `sleep`, and timeout tests.

## TDD Execution Plan

Use vertical red/green tracer bullets. For each RED, first add one failing behavior test in `crates/opentk-sync/tests/syncfeed_client.rs`, run the narrow test command to observe failure, then implement the minimum production code. Keep the test through the public `SyncFeedClient` API and local mock server.

- [x] RED 1: Mock a `Document` feed page with one entry whose entry-level `next` points to `skiptoken=1`. Assert `fetch_page(first Document cursor)` returns one entry and the page `next_request` equals the entry-level `next`, not any unrelated feed-level link. Confirm it fails because no client API exists.
- [x] GREEN 1: Add `syncfeed` module, config/cursor/page/error types, first-page URL construction, one HTTP GET, content-type check, Atom XML parsing for entry id/category/updated/content/enclosure/entry-next, latency/status capture, and exact entry-level next cursor validation.
- [x] RED 2: Mock two ordered pages for one category: first has an entry with `next`, second has no entries and a feed-level `resume`. Assert `fetch_category_until_resume("Document", Start)` returns both pages, follows the first page's cursor exactly, and stops at resume. Confirm failure before loop/resume handling.
- [x] GREEN 2: Implement category loop, feed-level resume parsing, no-entry resume stop semantics, and invalid-resume-page error when an empty page lacks `resume`.
- [x] RED 3: Add malformed cursor URL cases: wrong path, missing category, wrong category, missing `skiptoken` for a resumed cursor, and unparsable URL in XML. Assert every case returns a distinct explicit error variant with useful context. Confirm failure before validation is complete.
- [x] GREEN 3: Centralize cursor URL validation in `SyncFeedCursor`, remove ad hoc URL parsing from page parser, and wire explicit error variants.
- [x] RED 4: Add response validation tests for non-2xx, XML content type missing/wrong, malformed XML, XML with entry missing `next`, and timeout. Assert each returns the matching explicit error and exposes status/latency where available. Confirm failures before all classifications exist.
- [x] GREEN 4: Implement status handling, content-type validation, timeout classification, parse errors, and request outcome retention without swallowing transport errors.
- [x] RED 5: Add retry behavior tests: a transient 429/503 sequence retries idempotently and then succeeds; a permanent 500/429 exhaustion reports retry exhaustion with last status and attempt count. Confirm failure before retry policy exists.
- [x] GREEN 5: Implement bounded retry with backoff, retry only idempotent GETs for 429, 500, 502, 503, 504, and request timeouts, and record attempts in `SyncFeedRequestOutcome`.
- [x] RED 6: Add concurrency test with two categories and a mock server delay. Assert with `max_concurrent_requests = 2`, both categories overlap, while each category's observed request sequence remains `skiptoken=0` then `skiptoken=1` then resume. Confirm failure before global limiter/category worker helper exists.
- [x] GREEN 6: Add global `Semaphore` limiter shared by cloned clients, implement `fetch_categories_until_resume(categories)` or an equivalent helper that runs category workers concurrently while each category loops sequentially.
- [x] RED 7: Add limiter feedback test proving request outcomes feed the adaptive limiter: a 429 response reduces available pacing for subsequent requests and successful low-latency responses can recover toward the configured max. Confirm failure before adaptive behavior exists.
- [x] GREEN 7: Implement small adaptive limiter state behind the same sync boundary. Keep the public interface to observations/outcomes, not raw semaphore internals.
- [x] REFACTOR: Run the improve-code-boundaries review before broad checks. Remove duplicate cursor structs, raw URL strings in page/entry public types, parser decisions embedded in tests, and any helper module that merely forwards to another helper. Ensure `opentk-sync::syncfeed` is a deep module: callers pass category/cursor and receive typed pages plus explicit errors.

## Behavior Notes

- Do not add live SyncFeed tests in this task. Local mock-server tests are enough; live API belongs in the story-ending `make test-long` gate.
- Do not write database rows or progress rows here. Task 05 will decide durable transaction semantics.
- Do not silently default missing cursor state. Missing feed/category/entry cursor state is an error because restart correctness depends on exact cursor persistence.
- Do not parse entity payloads beyond extracting raw embedded content XML. Complete entity parsing belongs to task 04.
- Do not add backwards-compatible deprecated APIs. This is greenfield.

## Documentation

Add or update a focused doc if the implementation needs it, preferably `docs/syncfeed-client.md`, covering:

- official SyncFeed URL/cursor rules used by this client;
- why the last entry-level `next` is the durable page cursor;
- how `resume` is represented as caught-up state;
- retry/timeout/status classifications;
- concurrency and adaptive limiter behavior;
- why tests use a local mock server and live API validation is deferred to story-end verification.

## Verification

Run the narrow test after each RED/GREEN cycle. Before completion, run in order:

- `cargo fmt`
- `make check`
- `make lint`
- `make test`

Do not run `make test-long`; this is not the story-ending task and the task does not explicitly require long/e2e validation.

After all checks pass, update `task-03-implement-syncfeed-client-and-rate-limits.md` acceptance boxes and set `<passes>true</passes>`, run `/bin/bash .ralph/task_switch.sh`, commit all files with `task finished task-03-implement-syncfeed-client-and-rate-limits: ...`, push, and quit immediately.

NOW EXECUTE
