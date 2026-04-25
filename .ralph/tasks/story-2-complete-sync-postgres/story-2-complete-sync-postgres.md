# Story 2: Complete Sync To PostgreSQL

Goal: implement a thoroughly tested SyncFeed XML importer that syncs the entire official Tweede Kamer information model into PostgreSQL tables using `sqlx`, with exact cursor tracking, category-level parallelism, API limit respect, and schemas generated or verified against the official informatiemodel and XSDs.

Official source documentation:

- Informatiemodel: https://opendata.tweedekamer.nl/documentatie/informatiemodel
- SyncFeed API: https://opendata.tweedekamer.nl/documentatie/syncfeed-api
- XSD link from informatiemodel page: https://github.com/TweedeKamerDerStaten-Generaal/OpenDataPortaal

Tasks:

- task-1-model-official-informatiemodel.md
- task-2-design-postgres-schema.md
- task-3-implement-syncfeed-client-and-rate-limits.md
- task-4-implement-complete-parser-and-writers.md
- task-5-implement-parallel-sync-runner.md
- task-6-deep-verify-complete-sync.md
