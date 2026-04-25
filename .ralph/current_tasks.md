# Current Tasks Summary

Generated: Sun Apr 26 12:38:44 AM CEST 2026

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-1-rust-project-setup/story-1-rust-project-setup.md`

```
# Story 1: Rust Project Setup

Goal: initialize the repository as a production Rust workspace for OpenTK with repeatable local commands, CI-ready checks, and a clean base for the PostgreSQL SyncFeed importer and later HTTP API work.

Tasks:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-1-rust-project-setup/task-2-developer-commands-and-ci.md`

```
## Task: Story 1 Task 2 - Add Developer Commands and CI-Ready Checks <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-1-rust-project-setup/task-3-verify-setup-end-to-end.md`

```
## Task: Story 1 Task 3 - Deep Verify Rust Setup End To End <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-2-complete-sync-postgres/story-2-complete-sync-postgres.md`

```
# Story 2: Complete Sync To PostgreSQL

Goal: implement a thoroughly tested SyncFeed XML importer that syncs the entire official Tweede Kamer information model into PostgreSQL tables using `sqlx`, with exact cursor tracking, category-level parallelism, API limit respect, and schemas generated or verified against the official informatiemodel and XSDs.

Official source documentation:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-2-complete-sync-postgres/task-1-model-official-informatiemodel.md`

```
## Task: Story 2 Task 1 - Model Official Informatiemodel and XSDs <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-2-complete-sync-postgres/task-2-design-postgres-schema.md`

```
## Task: Story 2 Task 2 - Design Complete PostgreSQL Schema <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-2-complete-sync-postgres/task-3-implement-syncfeed-client-and-rate-limits.md`

```
## Task: Story 2 Task 3 - Implement SyncFeed Client and Rate Limit Respect <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-2-complete-sync-postgres/task-4-implement-complete-parser-and-writers.md`

```
## Task: Story 2 Task 4 - Implement Complete XML Parser and PostgreSQL Writers <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-2-complete-sync-postgres/task-5-implement-parallel-sync-runner.md`

```
## Task: Story 2 Task 5 - Implement Parallel Complete Sync Runner <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-2-complete-sync-postgres/task-6-deep-verify-complete-sync.md`

```
## Task: Story 2 Task 6 - Deep Verify Complete PostgreSQL Sync <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-3-http-api/story-3-http-api.md`

```
# Story 3: HTTP API

Goal: expose the synced PostgreSQL data through a small Rust HTTP API with an OpenAPI specification and tests proving the API responses are backed directly by synced tables.

Tasks:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-3-http-api/task-1-add-http-server-and-openapi.md`

```
## Task: Story 3 Task 1 - Add HTTP Server and OpenAPI Spec <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-3-http-api/task-2-implement-core-read-endpoints.md`

```
## Task: Story 3 Task 2 - Implement Core Read Endpoints <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-3-http-api/task-3-deep-verify-http-api.md`

```
## Task: Story 3 Task 3 - Deep Verify HTTP API <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-4-document-text-support/story-4-document-text-support.md`

```
# Story 4: Document Text Support

Goal: fetch linked document assets, store upstream links in PostgreSQL, prefer official text/HTML/transcript versions from Tweede Kamer or other official government sources when available, extract text from binary formats only as a fallback, store document text/HTML in PostgreSQL, and expose the content through HTTP. Binary source files remain linked assets; official or extracted text/HTML becomes queryable API data.

Tasks:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-4-document-text-support/task-1-design-document-content-schema.md`

```
## Task: Story 4 Task 1 - Design Document Content Schema <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-4-document-text-support/task-2-fetch-and-classify-document-assets.md`

```
## Task: Story 4 Task 2 - Fetch and Classify Document Assets <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-4-document-text-support/task-3-extract-document-text-and-html.md`

```
## Task: Story 4 Task 3 - Extract Document Text and HTML <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-4-document-text-support/task-4-expose-document-content-api.md`

```
## Task: Story 4 Task 4 - Expose Document Content API <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-4-document-text-support/task-5-deep-verify-document-content.md`

```
## Task: Story 4 Task 5 - Deep Verify Document Content Extraction <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-5-search/story-5-search.md`

```
# Story 5: Search

Goal: add high-quality fuzzy search over documents and API data after the sync, HTTP API, and document text stories exist. The task must investigate the best engine using the current repository state and real data, then implement the selected engine with deep quality verification.

Tasks:
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-5-search/task-1-investigate-search-engine.md`

```
## Task: Story 5 Task 1 - Investigate Search Engine With Current Repo State <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-5-search/task-2-design-search-indexing.md`

```
## Task: Story 5 Task 2 - Design Search Indexing Pipeline <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-5-search/task-3-implement-search-sync.md`

```
## Task: Story 5 Task 3 - Implement Search Sync Pipeline <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-5-search/task-4-expose-search-api.md`

```
## Task: Story 5 Task 4 - Expose Search API <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/opentk/.ralph/tasks/story-5-search/task-5-deep-verify-search-quality.md`

```
## Task: Story 5 Task 5 - Deep Verify Search Quality <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

