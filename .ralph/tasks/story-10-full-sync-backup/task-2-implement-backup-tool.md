## Task: Story 10 Task 2 - Implement Backup Tool <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the backup subcommand for `opentk-sync`.

Implementation:
1. Add `backup` subcommand to `opentk-sync`:
   - `opentk-sync backup --output /backups/opentk-20260426.pgdump`
   - Optional `--sync-first` flag: if any category is not caught up, trigger full sync first
2. Run `pg_dump -Fc` via `tokio::process::Command` asynchronously
3. Collect metadata:
   - `backup_at`: ISO8601
   - `version`: from Cargo.toml
   - `schema_version`: from `_sqlx_migrations`
   - `categories`: query `sync_state` for latest_skiptoken and `pg_class` for row counts per category table
   - `sizes`: `compressed_bytes` from pgdump file size, `raw_database_bytes` from `pg_database_size()`
4. Write metadata JSON to `{output_path}.meta.json`
5. Report sizes to stdout
6. The backup image needs `pg_dump`; either include in Dockerfile or use `postgres:16` client tools in a separate Dockerfile for backup

In scope: backup subcommand, pg_dump orchestration, metadata, size reporting. Out of scope: S3, scheduling.

</description>


<acceptance_criteria>
- [ ] `opentk-sync backup --output /tmp/test.pgdump` produces valid pgdump
- [ ] Produces `{output}.meta.json` with valid metadata
- [ ] Reports compressed and raw sizes to stdout
- [ ] `--sync-first` triggers sync if not caught up
- [ ] `pg_restore --list` can read the dump file
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
