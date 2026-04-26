## Task: Story 10 Task 3 - Implement Restore and Catch-Up <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the restore subcommand for `opentk-sync`.

Implementation:
1. Add `restore` subcommand:
   - `opentk-sync restore --backup /backups/opentk-20260426.pgdump`
2. Read metadata JSON to validate:
   - Version compatibility (same major version)
   - Schema version matches or is older than target
3. Run `pg_restore --jobs 4 --dbname {url}` via `tokio::process::Command`
   - Create database if not exists
4. Run SQLx migrations to bring schema up to date
5. Launch incremental sync from metadata skiptokens to current:
   - Reuse `CompleteSyncRunner` with `SyncRunMode::UntilCaughtUp`
6. Report progress: restore time, migration time, sync time, final row counts
7. Idempotent: fail cleanly if database already exists and `--force` not given

In scope: restore subcommand, pg_restore, migrations, incremental catch-up. Out of scope: live migration, PITR.

</description>


<acceptance_criteria>
- [ ] `opentk-sync restore --backup /tmp/test.pgdump` restores successfully
- [ ] Database is queryable after restore
- [ ] SQLx migrations bring schema to current
- [ ] Incremental sync catches up from backup skiptokens
- [ ] Progress reported: restore, migration, sync times, final counts
- [ ] Running against existing DB fails without `--force`
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
