## Task: Story 10 Task 1 - Design Full Sync Backup and Restore Format <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Design a mechanism to perform a full sync, export as pgdump, and restore to skip initial backfill.

Design decisions:
1. **Trigger:** New subcommand `opentk-sync backup --output /backups/opentk.pgdump`
2. **Format:** `pg_dump -Fc` (custom binary format, compressed, parallel restore)
3. **Storage:** Local filesystem ONLY. No S3, no cloud, no external dependencies. The user explicitly rejected S3.
4. **Metadata JSON alongside backup:**
   ```json
   {
     "backup_at": "2026-04-26T12:00:00Z",
     "version": "0.1.0",
     "schema_version": 42,
     "categories": [
       {"category": "Document", "latest_skiptoken": 1234567, "row_count": 890123}
     ],
     "sizes": {
       "compressed_bytes": 1073741824,
       "raw_database_bytes": 2147483648
     }
   }
   ```
5. **Restore:** `opentk-sync restore --backup /backups/opentk.pgdump`:
   - Runs `pg_restore` into target database
   - Reads metadata to get resume skiptokens
   - Runs SQLx migrations if schema is behind
   - Triggers incremental sync from backup skiptokens to current
6. **Size reporting:** Backup command reports compressed and raw sizes to stdout

Deliverable: `docs/backup-restore.md` design document.

In scope: design doc, metadata schema, restore procedure. Out of scope: implementation.

</description>


<acceptance_criteria>
- [ ] `docs/backup-restore.md` documents backup format, metadata, restore procedure
- [ ] Design uses local filesystem only
- [ ] Metadata schema includes skiptokens, sizes, version, schema_version
- [ ] Restore procedure documented end-to-end
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
