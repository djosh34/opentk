# Done Tasks Summary

Generated: Mon Apr 27 20:19:15 CEST 2026

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-cargo-run-opentk-sync-linker-missing-objects.md`

```
## Bug: cargo run opentk-sync can fail linking with missing target object files <status>done</status> <passes>true</passes> <priority>medium</priority>

<description>
While executing Story 10 Task 01 on 2026-04-26, the preliminary command
`cargo run -p opentk-db --bin opentk-sync -- --help` failed before any sync work
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-docker-workflow-oci-archive-has-multiple-images.md`

```
## Bug: Docker Workflow OCI Archive Has Multiple Images <status>done</status> <passes>true</passes> <priority>ultra high</priority>

<description>
The Docker publishing workflow fails while initializing an OCI archive:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-full-sync-fails-on-duplicate-toezegging-kamerbrief-nakoming.md`

```
## Bug: full sync fails on duplicate Toezegging kamerbriefNakoming field <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted a clean full SyncFeed-to-PostgreSQL sync from a fresh
database and the public sync runner exited nonzero after 305 seconds.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-full-sync-fails-on-duplicate-toezegging-toegezegd-aan.md`

```
## Bug: Full Sync Fails on Duplicate Toezegging toegezegdAan <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 clean full sync run `20260427-030235` failed while parsing the live SyncFeed `Toezegging` category.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-full-sync-fractie-zetel-vacature-initial-feed-timeout.md`

```
## Bug: full sync fails on FractieZetelVacature initial SyncFeed timeout <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted one clean full SyncFeed-to-PostgreSQL sync from a
fresh migrated database on 2026-04-27. The actual sync command failed after
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-full-sync-plan-assumes-usr-bin-time-exists.md`

```
## Bug: full sync operational plan assumes /usr/bin/time exists <status>done</status> <passes>true</passes> <priority>medium</priority>

<description>
While executing Story 10 Task 01 on 2026-04-26, the sync command wrapper failed
before starting the sync binary because `/usr/bin/time` was not available:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-full-sync-post-measurement-query-used-invalid-columns.md`

```
## Bug: Full Sync Post-Measurement Query Used Invalid Columns <status>not_started</status> <passes>true</passes> <priority>medium</priority>

<description>
During Story 10 Task 01 full-sync post-run measurement for run `20260427-044908`,
the first post-sync evidence command guessed two column names that do not exist
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-full-sync-toezegging-http-transport-decode-error.md`

```
## Bug: Full sync fails on Toezegging HTTP transport decode error <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted a fresh one-time clean full sync from an empty
database and the public sync runner exited nonzero on the initial `Toezegging`
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-full-sync-toezegging-initial-feed-timeout.md`

```
## Bug: full sync fails on Toezegging initial SyncFeed timeout <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted one clean full SyncFeed-to-PostgreSQL sync from a
fresh migrated database on 2026-04-26. The actual sync command failed after
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-remove-bs-text-assert-tests.md`

```
## Bug: Remove bs text-assert tests <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
This is a non-code file/test-suite cleanup bug. Do not use Red-Green TDD for this task, because the work is to remove brittle text-assert tests rather than to add new behavior. Do not add new Rust tests that assert source/docs/config text fragments as acceptance coverage for this cleanup.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/bug-remove-swallowed-env-and-duration-errors.md`

```
## Bug: Remove Swallowed Env And Duration Errors <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
Final boundary review for Story 03 Task 01 detected pre-existing swallowed errors outside the new API path:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/database-url-redaction-ignores-set-password-error.md`

```
## Bug: Database URL redaction ignores set_password error <status>not_started</status> <passes>true</passes> <priority>medium</priority>

<description>
During the boundary review for the SyncFeed startup validation body-read fix,
inspection found `crates/opentk-db/src/startup_validation.rs` ignoring the
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/full-sync-deadlocks-writing-persoon-nevenfunctie.md`

```
## Bug: Full sync deadlocks writing PersoonNevenfunctie <status>not_started</status> <passes>true</passes> <priority>high</priority>

<description>
While manually verifying
`.ralph/tasks/bugs/full-sync-fails-on-deleted-persoon-nevenfunctie-body.md`,
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/full-sync-fails-on-deleted-persoon-nevenfunctie-body.md`

```
## Bug: Full sync fails on deleted PersoonNevenfunctie body content <status>not_started</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted one clean full SyncFeed-to-PostgreSQL sync from an
empty dedicated database:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/full-sync-fails-on-persoon-verwijderd-true.md`

```
## Bug: full sync fails on Persoon verwijderd boolean value True <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Manual verification for `.ralph/tasks/bugs/bug-full-sync-toezegging-initial-feed-timeout.md`
reran the captured full-sync command on 2026-04-27:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/full-sync-fails-on-unknown-toezegging-toegezegd-aan.md`

```
## Bug: Full sync fails on unknown Toezegging toegezegdAan field <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Story 10 Task 01 attempted one clean full SyncFeed-to-PostgreSQL sync from an
empty dedicated database after the previous PersoonNevenfunctie deleted-body
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/bugs/startup-validation-swallowing-response-body-errors.md`

```
## Bug: Startup validation swallows response body errors <status>done</status> <passes>true</passes> <priority>medium</priority>

<description>
During Story 10 Task 01 operational sync preparation, inspection found
`crates/opentk-db/src/startup_validation.rs` using
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-02-complete-sync-postgres/task-01-model-official-informatiemodel.md`

```
## Task: Story 02 Task 01 - Model Official Informatiemodel and XSDs <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-02-complete-sync-postgres/task-02-design-postgres-schema.md`

```
## Task: Story 02 Task 02 - Design Complete PostgreSQL Schema <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-02-complete-sync-postgres/task-03-implement-syncfeed-client-and-rate-limits.md`

```
## Task: Story 02 Task 03 - Implement SyncFeed Client and Rate Limit Respect <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-02-complete-sync-postgres/task-04-implement-complete-parser-and-writers.md`

```
## Task: Story 02 Task 04 - Implement Complete XML Parser and PostgreSQL Writers <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-02-complete-sync-postgres/task-05-implement-parallel-sync-runner.md`

```
## Task: Story 02 Task 05 - Implement Parallel Complete Sync Runner <status>done</status> <passes>true</passes>

<plan>
.ralph/tasks/story-02-complete-sync-postgres/task-05-implement-parallel-sync-runner_plans/parallel-sync-runner-plan.md
</plan>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-02-complete-sync-postgres/task-06-deep-verify-complete-sync.md`

```
## Task: Story 02 Task 06 - Deep Verify Complete PostgreSQL Sync <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-03-http-api/task-01-add-http-server-and-openapi.md`

```
## Task: Story 03 Task 01 - Add HTTP Server and OpenAPI Spec <status>not_started</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-03-http-api/task-02-implement-core-read-endpoints.md`

```
## Task: Story 03 Task 02 - Implement Core Read Endpoints <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-03-http-api/task-03-deep-verify-http-api.md`

```
## Task: Story 03 Task 03 - Deep Verify HTTP API <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-04-document-text-support/task-01-design-document-content-schema.md`

```
## Task: Story 04 Task 01 - Design Document Content Schema <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-04-document-text-support/task-02-fetch-and-classify-document-assets.md`

```
## Task: Story 04 Task 02 - Fetch and Classify Document Assets <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-04-document-text-support/task-03-extract-document-text-and-html.md`

```
## Task: Story 04 Task 03 - Extract Document Text and HTML <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-04-document-text-support/task-04-expose-document-content-api.md`

```
## Task: Story 04 Task 04 - Expose Document Content API <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-04-document-text-support/task-05-deep-verify-document-content.md`

```
## Task: Story 04 Task 05 - Deep Verify Document Content Extraction <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-05-search/task-01-investigate-search-engine.md`

```
## Task: Story 05 Task 01 - Investigate Search Engine With Current Repo State <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-05-search/task-02-design-search-indexing.md`

```
## Task: Story 05 Task 02 - Design Search Indexing Pipeline <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-05-search/task-03-implement-search-sync.md`

```
## Task: Story 05 Task 03 - Implement Search Sync Pipeline <status>completed</status> <passes>true</passes>

<plan>
.ralph/tasks/story-05-search/task-03-implement-search-sync_plans/search-sync-pipeline-plan.md
</plan>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-05-search/task-04-expose-search-api.md`

```
## Task: Story 05 Task 04 - Expose Search API <status>done</status> <passes>true</passes>

<plan>
.ralph/tasks/story-05-search/task-04-expose-search-api_plans/search-api-plan.md
</plan>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-05-search/task-05-deep-verify-search-quality.md`

```
## Task: Story 05 Task 05 - Deep Verify Search Quality <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-06-config-unification/task-01-create-config-crate.md`

```
## Task: Story 06 Task 01 - Create Centralized Configuration Crate <status>done</status> <passes>true</passes>

<plan>
.ralph/tasks/story-06-config-unification/task-01-create-config-crate_plans/centralized-config-crate-plan.md
</plan>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-06-config-unification/task-02-refactor-binaries-to-config.md`

```
## Task: Story 06 Task 02 - Refactor All Binaries to Use Unified Config <status>done</status> <passes>true</passes>

<plan>
.ralph/tasks/story-06-config-unification/task-02-refactor-binaries-to-config_plans/refactor-binaries-to-config-plan.md
</plan>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-06-config-unification/task-03-startup-validation.md`

```
## Task: Story 06 Task 03 - Add Startup Dependency Validation <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-07-binary-rename-and-cdc/task-01-rename-to-opentk-sync.md`

```
## Task: Story 07 Task 01 - Rename complete-sync Binary to opentk-sync <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-07-binary-rename-and-cdc/task-02-implement-listen-notify-cdc.md`

```
## Task: Story 07 Task 02 - Implement LISTEN/NOTIFY CDC for Search Sync <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-07-binary-rename-and-cdc/task-03-integrate-cdc-into-api.md`

```
## Task: Story 07 Task 03 - Integrate Search CDC Background Task into opentk-api <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-08-docker-local-dev/task-01-dockerfiles.md`

```
## Task: Story 08 Task 01 - Dockerfiles for Local Development <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-08-docker-local-dev/task-02-docker-compose.md`

```
## Task: Story 08 Task 02 - Docker Compose Local Development Stack <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-08-docker-local-dev/task-03-multi-arch-scratch.md`

```
## Task: Story 08 Task 03 - Multi-Arch Scratch Dockerfiles <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-08-docker-local-dev/task-04-health-checks-and-shutdown.md`

```
## Task: Story 08 Task 04 - Docker Health Checks and Graceful Shutdown <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-10-full-sync-backup/task-01-run-one-time-clean-full-sync-report.md`

```
## Task: Story 10 Task 01 - Run One-Time Clean Full Sync and Report Results <status>done</status> <passes>true</passes>

<description>
**Goal:** Run one clean full SyncFeed-to-PostgreSQL sync once, measure the result, write a human-readable report under `.ralph/reports/`, and email that report to the user.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-10-full-sync-backup/task-02-create-one-time-dumb-database-dump.md`

```
## Task: Story 10 Task 02 - Create One-Time Dumb Dump of Fully Synced Database <status>done</status> <passes>true</passes>

<description>
**Goal:** After Story 10 Task 01 has produced a fully synced database, take one plain operational database dump of that exact state so development can revert after accidental wipe/truncate or other destructive mistakes.
```

