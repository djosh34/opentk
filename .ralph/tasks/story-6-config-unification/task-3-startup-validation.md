## Task: Story 6 Task 3 - Add Startup Dependency Validation <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add comprehensive startup validation so every binary fails fast with a clear error if its dependencies are unreachable. Currently binaries start and only fail later when they first try to use the database or search engine.

Requirements:
1. Each binary verifies its database URL is valid and reachable during startup (`SELECT 1` with 5s timeout).
2. `opentk-api` verifies Meilisearch is reachable during startup (lightweight stats call, 5s timeout).
3. `search-sync` verifies both PostgreSQL and Meilisearch before indexing.
4. `complete-sync` verifies PostgreSQL and SyncFeed base URL reachability.
5. All errors are typed and human-readable:
   - "Database at postgres://postgres@postgres:5432/opentk is unreachable: connection refused"
   - "Meilisearch at http://meilisearch:7700 returned 401: invalid API key"
   - "SyncFeed base URL https://... is unreachable: timeout"
6. Add `--validate-config` flag to all binaries that performs all checks and exits 0 without doing work.

In scope: startup validation, typed errors, `--validate-config`, tests. Out of scope: runtime monitoring.

</description>


<acceptance_criteria>
- [ ] `opentk-api --validate-config` exits 0 when DB and search reachable, non-zero otherwise
- [ ] `complete-sync --validate-config` exits 0 when DB and SyncFeed reachable
- [ ] `search-sync --validate-config` exits 0 when DB and Meilisearch reachable
- [ ] All binaries produce clear dependency errors on startup
- [ ] Validation uses 5s timeout for fast failure
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
