## Task: Story 7 Task 2 - Implement LISTEN/NOTIFY CDC for Search Sync <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement a true CDC mechanism using PostgreSQL LISTEN/NOTIFY so that the inference binary (`opentk-api`) can react to database changes in near-real-time and keep Meilisearch in sync automatically.

Why LISTEN/NOTIFY:
- No wal_level changes needed (unlike logical replication)
- Native PostgreSQL feature, works out of the box
- Near real-time (microsecond latency)
- Simple: one trigger + one listener connection

Implementation:
1. **Database trigger:** Add a migration that creates a trigger on `sync_entity`:
   ```sql
   CREATE OR REPLACE FUNCTION notify_sync_entity_change()
   RETURNS TRIGGER AS $$
   BEGIN
     PERFORM pg_notify('sync_entity_change', json_build_object(
       'source_category', NEW.source_category,
       'source_id', NEW.source_id,
       'latest_skiptoken', NEW.latest_skiptoken,
       'deleted', NEW.deleted
     )::text);
     RETURN NEW;
   END;
   $$ LANGUAGE plpgsql;

   CREATE TRIGGER sync_entity_change_trigger
   AFTER INSERT OR UPDATE ON sync_entity
   FOR EACH ROW EXECUTE FUNCTION notify_sync_entity_change();
   ```
2. **Rust listener:** Add `opentk-db/src/search_cdc.rs` with a `SearchCdcListener` that:
   - Opens a dedicated PostgreSQL connection (not from the pool, since LISTEN requires a persistent connection)
   - Calls `LISTEN sync_entity_change`
   - Receives notifications via `sqlx::postgres::PgListener`
   - Buffers notifications and deduplicates by `(source_category, source_id)` within a time window (e.g., 1 second)
   - Flushes the buffer by calling `incremental_index` from `search_sync` module for the affected records
3. **Batching strategy:** The listener should not call Meilisearch for every single row change. Instead:
   - Collect notifications for 1 second or until 100 unique records
   - Then run a targeted incremental sync for just those categories/source_ids
   - If a category has many changes, fall back to regular `incremental_index` for that category

In scope: migration, trigger, PgListener, buffering/dedup, targeted incremental sync. Out of scope: Prometheus metrics, admin endpoints.

</description>


<acceptance_criteria>
- [ ] Migration adds `notify_sync_entity_change()` trigger on `sync_entity`
- [ ] `SearchCdcListener` receives notifications via `PgListener`
- [ ] Notifications are deduplicated within 1-second window
- [ ] Listener triggers incremental sync for affected records
- [ ] Unit tests verify notification parsing and dedup logic
- [ ] Integration test verifies trigger fires and listener receives it
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>

<plan>
.ralph/tasks/story-7-binary-rename-and-cdc/task-2-implement-listen-notify-cdc_plans/listen-notify-cdc-plan.md
</plan>

NOW EXECUTE
