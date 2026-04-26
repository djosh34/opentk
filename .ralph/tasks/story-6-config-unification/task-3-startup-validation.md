## Task: Story 6 Task 3 - Add Startup Dependency Validation <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add startup validation that fails fast for required dependencies and degrades cleanly for optional search in `opentk-api`. Currently binaries start and only fail later when they first try to use the database or search engine.

Requirements:
1. Each binary verifies its database URL is valid and reachable during startup (`SELECT 1` with 5s timeout).
2. `opentk-api` attempts to verify Meilisearch during startup (lightweight stats call, 5s timeout), but Meilisearch failure is non-fatal. The API must log the failure, mark search as unavailable/degraded, and continue serving non-search routes.
3. `search-sync` verifies both PostgreSQL and Meilisearch before indexing because search is its primary function.
4. `complete-sync` verifies PostgreSQL and SyncFeed base URL reachability.
5. All errors are typed and human-readable:
   - "Database at postgres://postgres@postgres:5432/opentk is unreachable: connection refused"
   - "Meilisearch at http://meilisearch:7700 returned 401: invalid API key"
   - "SyncFeed base URL https://... is unreachable: timeout"
6. Add `--validate-config` flag to all binaries that performs all checks and exits 0 without doing work. For `opentk-api`, `--validate-config` exits 0 when required dependencies are reachable even if search is unavailable, and must print the degraded search status. Add a stricter `--validate-config --require-search` mode if operators need search to be fatal in deployment gates.

In scope: startup validation, typed errors, `--validate-config`, tests. Out of scope: runtime monitoring.

</description>


<acceptance_criteria>
- [ ] `opentk-api --validate-config` exits 0 when DB is reachable, including when search is unavailable
- [ ] `opentk-api --validate-config --require-search` exits non-zero when search is unavailable
- [ ] `complete-sync --validate-config` exits 0 when DB and SyncFeed reachable
- [ ] `search-sync --validate-config` exits 0 when DB and Meilisearch reachable
- [ ] All binaries produce clear dependency errors on startup, with optional API search reported as degraded instead of fatal
- [ ] Validation uses 5s timeout for fast failure
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
