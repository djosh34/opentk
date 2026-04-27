## Task: Story 11 Task 02 - Structured Logging, Metrics, and JSON Log Format <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add structured logging for the search CDC daemon and a simple metrics endpoint.

Requirements:
1. **Structured logging:** Use `tracing` in the CDC daemon:
   - `search_sync.batch.start` — category, skiptoken range
   - `search_sync.batch.complete` — indexed, deleted, failed, duration_ms
   - `search_sync.batch.error` — error details
   - `search_sync.notification.received` — category, source_id
2. **JSON log format:** When `log.format = "json"` is set in the TOML config file, use `tracing-subscriber` JSON layer. Default is `pretty`.
3. **Metrics endpoint `GET /metrics`:**
   ```
   search_sync_batches_total 42
   search_sync_records_indexed_total 15000
   search_sync_records_failed_total 3
   search_sync_last_success_timestamp 1714132800
   ```
   Simple atomic counters, no Prometheus dependency.
4. Document in `docs/operations.md`

In scope: tracing events, JSON logs, metrics endpoint, docs. Out of scope: Prometheus server, Grafana.

</description>


<acceptance_criteria>
- [ ] CDC daemon emits structured tracing events for every batch
- [ ] `log.format = "json"` in TOML config switches to JSON output
- [ ] `GET /metrics` returns text-based counters
- [ ] `docs/operations.md` documents observability endpoints
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
