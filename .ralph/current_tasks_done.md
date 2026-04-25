# Done Tasks Summary

Generated: Sat Apr 25 10:29:23 PM CEST 2026

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-long-lane-crash-recovery-under-blocked-reconcile-still-fails.md`

```
## Bug: Long-lane blocked-reconcile crash recovery still fails during story-23 validation <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
While validating `.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch.md`, the repo gates reached a remaining blocker in `make test-long`.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-readme-public-image-quick-start-omits-secure-runner-and-verify-config.md`

```
## Bug: README public-image quick start omits secure runner and verify config <status>done</status> <passes>true</passes> <priority>ultra high</priority>

<description>
Story 24 execution is blocked because the README cannot currently serve as the only operator document for the secure public-image migration flow.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-readme-public-image-verify-compose-exhausts-docker-address-pools.md`

```
## Bug: README public-image verify compose verification exhausts Docker address pools <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
Story 24 verification found a blocking defect before the README-only public-image flow could be completed.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-runner-test-port-selection-flakes-webhook-bind.md`

```
## Bug: Runner test port selection flakes and can fail webhook bind during parallel test runs <status>done</status> <passes>true</passes> <priority>medium</priority>

<description>
The runner test suite still has a time-of-check/time-of-use port-allocation race.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-schema-mismatch-reconcile-failure-missing-operator-stderr.md`

```
## Bug: Schema mismatch reconcile failure is persisted but not logged to stderr for operators <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
Schema mismatch verification in story 23 task 01 exposed a real operator-surface defect.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-verify-http-allows-warning-only-insecure-listener-modes.md`

```
## Bug: Verify HTTP allows warning-only insecure listener modes <status>done</status> <passes>true</passes> <priority>ultra high</priority>

<description>
The verify HTTP audit found that the listener accepts insecure remote-service modes such as plain HTTP and no client authentication. The CLI only prints `warning: no extra built-in protection is being provided by the verify service` and still treats those configurations as valid.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-verify-http-exposes-job-results-and-metrics-without-auth.md`

```
## Bug: Verify HTTP exposes job results and metrics without auth <status>completed</status> <passes>true</passes> <priority>ultra high</priority>

<description>
The verify HTTP audit found that `GET /jobs/{job_id}` and `GET /metrics` expose operational details to any caller on the listener. The current behavior includes job IDs, timestamps, failure reasons, mismatch details, source and destination database names, schema names, table names, and mismatch counts, with no authentication or authorization layer in the service itself.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-verify-http-https-runtime-does-not-load-server-certificate.md`

```
## Bug: Verify HTTP HTTPS runtime does not load the configured server certificate <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
The verify HTTP audit found that the HTTPS runtime path builds `server.TLSConfig` without loading `listener.tls.cert_path` and `listener.tls.key_path` into `TLSConfig.Certificates`. `Run(...)` then calls `ListenAndServeTLS("", "")`, which per the standard-library contract requires the certificate to already be present in `TLSConfig` when empty filenames are used.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-verify-http-request-body-size-is-unbounded.md`

```
## Bug: Verify HTTP request body size is unbounded <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
The verify HTTP audit found that `POST /jobs` and `POST /stop` decode directly from the full request body without a size cap. The new strict decoder rejects unknown fields and trailing documents, but it still allows arbitrarily large request bodies to be read into memory before validation completes.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-verify-http-retains-completed-jobs-and-metrics-forever.md`

```
## Bug: Verify HTTP retains completed jobs and metrics forever <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
The verify HTTP audit found that completed jobs are never pruned from `Service.jobs`. Each finished job keeps its full in-memory progress snapshot, including status messages, summary events, mismatch records, and error strings. `/metrics` then iterates every remembered job on every scrape.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-verify-http-runtime-failures-are-not-reported-in-json-logs.md`

```
## Bug: Verify HTTP runtime failures are not reported in JSON logs <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
During the receive-mail investigation for "How to use it?", the real verify HTTP service was run locally with `--log-format json` and exercised through curl against real PostgreSQL databases.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-verify-image-arm64-has-quay-vulnerability-findings.md`

```
## Bug: Verify image arm64 publish is blocked by real Quay vulnerability findings <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
Hosted GitHub Actions run `#39` on commit `4852f61843a8f3c1dbb89fbe5cf8bed5a09d9c25` proved the
publish workflow changes are working, and then exposed a real product-security defect outside the
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-webhook-row-batch-persistence-regression.md`

```
## Bug: Webhook row-batch persistence contract fails with HTTP 501 instead of 200 <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Detected on 2026-04-18 during a reporting audit by running `make test`.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/verify-compose-missing-client-ca-config-mount.md`

```
## Bug: Verify Compose novice-user contract omits the listener client CA mount <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
The documented registry-only novice-user flow for `verify.compose.yml` is broken.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-01-runner-config-ergonomics/task-01-runner-http-webhook-mode.md`

```
## Task: Add optional HTTP mode to runner webhook listener <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-01-runner-config-ergonomics/task-02-runner-destination-connection-string.md`

```
## Task: Allow runner destination to accept PostgreSQL connection strings <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-01-runner-config-ergonomics/task-03-runner-config-error-context.md`

```
## Task: Improve runner config validation error messages with actual values <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-01-runner-config-ergonomics/task-04-runner-deep-validation.md`

```
## Task: Add deep validation mode to runner validate-config <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-02-rust-foundation/01-task-scaffold-rust-workspace-and-dependency-policy.md`

```
## Task: Scaffold the Rust workspace and dependency policy for the runner <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-02-rust-foundation/02-task-build-single-binary-container-contract.md`

```
## Task: Build the single-binary container contract for the destination runner <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-02-verify-service-ergonomics/task-05-verify-cli-run-subcommand.md`

```
## Task: Add explicit `run` subcommand to verify service for CLI consistency <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-02-verify-service-ergonomics/task-06-standardize-tls-config-patterns.md`

```
## Task: Standardize TLS config field naming and structure across runner and verify <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-03-docs-api-contracts/task-07-docs-webhook-payload-format.md`

```
## Task: Document runner webhook payload format for API consumers <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-03-operator-ux-config/01-task-define-single-config-yaml-and-multi-db-mapping.md`

```
## Task: Define the single config YAML and multi-database mapping model <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-03-operator-ux-config/02-task-generate-postgresql-grant-sql-and-operator-artifacts.md`

```
## Task: Generate PostgreSQL grant SQL and operator-facing bootstrap artifacts <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-04-source-bootstrap/01-task-build-cockroach-bootstrap-command-and-script-output.md`

```
## Task: Build the Cockroach bootstrap command and emitted setup script <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-04-source-bootstrap/02-task-apply-postgresql-helper-bootstrap-automatically.md`

```
## Task: Apply helper-schema bootstrap inside PostgreSQL automatically from the runner <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-05-schema-validation/01-task-compare-schema-exports-semantically.md`

```
## Task: Compare Cockroach and PostgreSQL schema exports semantically <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-05-schema-validation/02-task-generate-helper-shadow-ddl-and-dependency-order.md`

```
## Task: Generate helper shadow DDL and dependency order from the validated schema <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-06-destination-ingest/01-task-build-https-webhook-server-and-routing.md`

```
## Task: Build the HTTPS webhook server and table-routing runtime <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-06-destination-ingest/02-task-persist-row-batches-into-helper-shadow-tables.md`

```
## Task: Persist row batches idempotently into helper shadow tables <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-06-destination-ingest/03-task-persist-resolved-watermarks-and-stream-state.md`

```
## Task: Persist resolved watermarks and stream state <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-07-reconcile/01-task-build-continuous-upsert-reconcile-loop.md`

```
## Task: Build the continuous upsert reconcile loop from shadow to real tables <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-07-reconcile/02-task-build-continuous-delete-reconcile-pass.md`

```
## Task: Build the continuous SQL-driven delete reconcile pass <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-07-reconcile/03-task-track-reconciled-watermarks-and-repeatable-sync-state.md`

```
## Task: Track reconciled watermarks and repeatable sync state <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-08-multi-db-orchestration/01-task-run-multiple-db-mappings-from-one-destination-container.md`

```
## Task: Run multiple database mappings from one destination container <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-09-verification-cutover/01-task-wrap-molt-verify-and-fail-on-log-detected-mismatches.md`

```
## Task: Wrap MOLT verify and fail on log-detected mismatches <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-09-verification-cutover/02-task-build-drain-to-zero-and-cutover-readiness-check.md`

```
## Task: Build drain-to-zero and cutover readiness checks <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-09-verification-cutover/03-task-document-api-write-freeze-cutover-runbook.md`

```
## Task: Document the API write-freeze cutover runbook <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-10-e2e-baseline/01-task-e2e-default-database-bootstrap-from-scratch.md`

```
## Task: End-to-end test a default database bootstrap from scratch <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-10-e2e-baseline/02-task-e2e-fk-heavy-initial-scan-and-live-catchup.md`

```
## Task: End-to-end test FK-heavy initial scan and live catch-up <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-10-e2e-baseline/03-task-e2e-delete-propagation-through-shadow-and-real-tables.md`

```
## Task: End-to-end test delete propagation through helper shadow and real tables <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-10-e2e-baseline/04-task-e2e-composite-pk-and-excluded-table-handling.md`

```
## Task: End-to-end test composite primary keys and excluded tables <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-10-e2e-baseline/05-task-e2e-multiple-large-multi-db-migrations.md`

```
## Task: End-to-end test multiple large multi-database migrations under one container <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-11-e2e-chaos/01-task-e2e-http-retry-chaos-imposed-externally.md`

```
## Task: End-to-end test HTTP retry chaos imposed externally <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-11-e2e-chaos/02-task-e2e-receiver-crash-and-restart-recovery.md`

```
## Task: End-to-end test receiver crash and restart recovery <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-11-e2e-chaos/03-task-e2e-network-fault-injection-imposed-externally.md`

```
## Task: End-to-end test externally imposed network fault injection <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-11-e2e-chaos/04-task-e2e-transaction-failure-recovery.md`

```
## Task: End-to-end test transaction-failure recovery <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-11-e2e-chaos/05-task-e2e-source-high-write-churn-during-transfer.md`

```
## Task: End-to-end test high source write churn during transfer <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-12-verify-e2e-integrity/01-task-assert-e2e-suite-has-no-cheating.md`

```
## Task: Assert the end-to-end suite has no cheating <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-12-verify-e2e-integrity/02-task-assert-single-container-tls-and-scoped-role-integrity.md`

```
## Task: Assert single-container, TLS, and scoped-role integrity in end-to-end tests <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-12-verify-e2e-integrity/03-task-assert-no-post-setup-source-commands-in-e2e.md`

```
## Task: Assert there are no post-setup source commands in end-to-end tests <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-13-verify-novice-user/01-task-verify-readme-alone-is-sufficient-for-novice-user.md`

```
## Task: Verify the README alone is sufficient for a novice user <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-13-verify-novice-user/02-task-verify-direct-docker-build-and-run-without-wrapper-scripts.md`

```
## Task: Verify direct Docker build and run works without wrapper scripts <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-13-verify-novice-user/03-task-verify-copyable-config-example-and-quick-start-clarity.md`

```
## Task: Verify the copyable config example and quick start are directly useful <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-14-reports/01-task-report-novice-user-manual-experience.md`

```
## Task: Produce an exhaustive novice-user manual experience report <status>completed</status> <passes>true</passes>

<description>
**Goal:** Manually try the whole system as a novice user and produce a very exhaustive, deeply investigative Markdown report of the actual experience. The higher order goal is to measure operator friction honestly from the user's perspective rather than from the implementer's assumptions.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-14-reports/02-task-report-code-complexity-and-kiss-assessment.md`

```
## Task: Produce an exhaustive code-complexity and KISS assessment report <status>completed</status> <passes>true</passes>

<description>
**Goal:** Read the code as it actually exists and produce a very exhaustive Markdown report on code complexity, structure, module interactions, simplicity, stability, and signs of overengineering. The higher order goal is to evaluate whether the implementation is staying faithful to KISS rather than drifting into complexity for its own sake.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-15-ci-build-test-image-pipeline/01-task-build-master-only-pipeline-for-full-tests-and-scratch-ghcr-image.md`

```
## Task: Build a push-to-master-only pipeline that runs the full test suite and publishes a commit-tagged scratch image to GHCR <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-15-ci-build-test-image-pipeline/02-task-add-ci-sanity-check-for-workflow-attack-vectors-and-secret-safety.md`

```
## Task: Add a CI sanity and security check task that audits workflow attack vectors, secret exposure, and untrusted PR behavior <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-15-ci-build-test-image-pipeline/03-task-add-free-vulnerability-scan-to-image-publish-workflow.md`

```
## Task: Add a free vulnerability scan to the image publish workflow so unsafe images fail before release <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config.md`

```
## Task: Remove all runner access to the source CockroachDB and delete the related config surface <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-16-runtime-split-removals/02-task-remove-runner-side-verify-capability-and-code-paths.md`

```
## Task: Remove verify behavior from the runner and delete every in-runner verification path <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-16-runtime-split-removals/03-task-remove-bash-bootstrap-flows-and-script-based-source-setup.md`

```
## Task: Remove bash-based bootstrap flows and replace the old script contract with generated SQL output only <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-16-runtime-split-removals/04-task-remove-novice-user-dependence-on-repo-clone-and-local-tooling.md`

```
## Task: Remove any novice-user path that requires a repo checkout, local installs, or build-from-source steps <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-16-runtime-split-removals/05-task-remove-contributor-rules-from-readme-and-keep-them-in-contributors-docs.md`

```
## Task: Remove contributor-only coding rules from README and keep the operator path free of internal project structure assumptions <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal.md`

```
## Task: Prune the codebase down to the verify-only source slice and prove all unrelated code was removed <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/02-task-upgrade-the-verify-slice-to-go-1-26-and-bump-all-dependencies.md`

```
## Task: Upgrade the verify-only slice to Go 1.26 and bump its dependencies before packaging the image <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source.md`

```
## Task: Build a scratch verify image from the pruned verify-only source <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection.md`

```
## Task: Add a dedicated verify-service config with source and destination TLS support and explicit verify mode selection <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`

```
## Task: Build an ultra-scoped HTTP job API for single active verify runs using config-defined connections only <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution.md`

```
## Task: Prove HTTP request inputs cannot cause command injection in verify execution <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/07-task-route-all-correctness-tests-through-the-verify-http-image-only.md`

```
## Task: Route all correctness verification through the verify HTTP image only and remove all alternate test-harness paths <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/08-task-expose-verify-job-progress-and-result-metrics.md`

```
## Task: Expose verify job progress and result metrics from the HTTP verify service <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/09-task-run-a-five-pass-security-audit-of-the-verify-http-surface-and-file-bugs-for-each-issue.md`

```
## Task: Run a five-pass security audit of the verify HTTP surface and file bugs for every issue found <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/10-task-prune-cockroachdb-molt-down-to-the-postgresql-cockroachdb-verify-hot-path-and-add-root-license-notices.md`

```
## Task: Prune `cockroachdb_molt` down to the PostgreSQL/CockroachDB verify hot path and add root license notices <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-18-verify-http-image/11-task-add-config-gated-raw-source-and-destination-table-json-output-to-verify-http.md`

```
## Task: Complete the verify HTTP JSON read surface with structured job results and config-gated raw table output <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-19-source-sql-emitter-image/01-task-build-a-one-time-sql-emitter-image-that-prints-required-sql-to-logs.md`

```
## Task: Build a one-time setup image that prints all required SQL to logs instead of executing bash scripts <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-19-source-sql-emitter-image/02-task-emit-the-required-cockroach-changefeed-sql-from-the-one-time-setup-image.md`

```
## Task: Emit the required Cockroach changefeed SQL from the one-time setup image <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-19-source-sql-emitter-image/03-task-emit-the-absolute-minimum-postgresql-role-grants-needed-by-the-runner.md`

```
## Task: Emit the absolute minimum PostgreSQL role grants needed by the runner from the one-time setup image <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-20-runner-scratch-image/01-task-build-the-runner-as-a-scratch-image-with-one-binary-that-applies-webhook-requests-to-postgresql.md`

```
## Task: Build the runner as a scratch image with one binary that only applies webhook requests to PostgreSQL <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-20-runner-scratch-image/02-task-enforce-the-runner-postgresql-only-runtime-contract.md`

```
## Task: Enforce the runner PostgreSQL-only runtime contract and prove it cannot access source or verify responsibilities <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/01-task-fix-github-workflows-to-build-test-and-publish-the-three-image-split.md`

```
## Task: Fix GitHub workflows to build, test, and publish the three-image split in the right order <status>completed</status> <passes>true</passes>

<blocked_by>.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure.md</blocked_by>

<description>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure.md`

```
## Task: Drive the full three-image GitHub pipeline under fifteen minutes with native `arm64` execution and aggressive workflow restructuring <status>completed</status> <passes>true</passes>

<priority>ultra_high</priority>

<description>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/02-task-massively-improve-image-build-speed-with-docker-layer-and-build-cache-reuse.md`

```
## Task: Massively improve image build speed with Docker layer reuse and shared Rust/Go dependency caches <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/03-task-require-make-test-long-to-pass-before-any-image-publish.md`

```
## Task: Require `make test-long` to pass before any image publish or release path can proceed <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/04-task-publish-separate-docker-compose-artifacts-for-runner-verify-and-sql-images.md`

```
## Task: Publish separate Docker Compose artifacts for the runner, verify, and SQL-emitter images using modern Compose config features <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/05-task-debug-real-github-image-build-failures-using-authenticated-workflow-api-log-access.md`

```
## Task: Debug real GitHub image-build failures using authenticated workflow API log access until the published runs succeed <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/06-task-publish-the-three-image-split-to-quay-with-strict-secret-redaction-and-master-only-access.md`

```
## Task: Publish the three-image split to Quay with strict secret redaction and `master`-only secret access <status>not_started</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-21-github-workflows-image-publish/08-task-make-the-default-branch-push-publish-workflow-succeed-from-a-plain-git-push.md`

```
## Task: Make the default-branch publish workflow succeed from a plain `git push` <status>not_started</status> <passes>true</passes>

<blocked_by>.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure.md</blocked_by>

<description>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-22a-runner-metrics/01-task-expose-low-cardinality-runner-activity-timing-and-failure-metrics.md`

```
## Task: Expose low-cardinality runner activity, timing, and failure metrics at `/metrics` <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-22a-runner-metrics/02-task-add-cached-shadow-vs-real-row-count-and-current-reconcile-state-metrics.md`

```
## Task: Add cached shadow-versus-real row-count and current reconcile-state metrics <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-22-structured-json-logging/01-task-verify-all-images-support-structured-json-logging.md`

```
## Task: Verify every shipped image supports structured JSON logging and add any missing support needed for a consistent operator-facing logging contract <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch.md`

```
## Task: Audit full end-to-end coverage for duplicate CDC delivery, recreated feeds, and source-destination schema mismatch, then add any missing cases to the full e2e suite <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-24-readme-only-novice-e2e/01-task-verify-readme-alone-enables-a-full-public-image-migration-with-zero-repo-access.md`

```
## Task: Verify the README alone enables a full public-image migration with zero repo access <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-24-readme-only-novice-e2e/02-task-verify-readme-stays-short-user-facing-and-inline-config-driven.md`

```
## Task: Verify the README stays short, user-facing, and driven by inline config examples instead of project philosophy <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-24-readme-only-novice-e2e/03-task-verify-cli-command-complexity-stays-low-and-help-works-everywhere.md`

```
## Task: Verify CLI command complexity stays low and `--help` works everywhere a user would expect it <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-25-verify-novice-user-registry-only/01-task-verify-registry-only-novice-user-can-complete-the-supported-flow.md`

```
## Task: Verify a novice user can complete the supported flow from published images alone with zero repo access <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-26-hosted-workflow-failure-investigation/01-task-debug-hosted-github-workflow-failures-parallelize-image-builds-and-surface-quay-security-findings.md`

```
## Task: Debug hosted GitHub workflow failures, parallelize image builds with the test lanes, and surface Quay security findings clearly <status>completed</status> <passes>true</passes>

<blocked_by>.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch.md</blocked_by>

<description>
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-27-verify-operator-ux-reset/01-task-reset-verify-service-config-to-operator-chosen-security-and-remove-redundant-tls-knobs.md`

```
## Task: Reset verify-service config to operator-chosen security and remove redundant TLS knobs <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-27-verify-operator-ux-reset/02-task-simplify-the-verify-http-contract-and-publish-curl-first-operator-docs.md`

```
## Task: Simplify the verify HTTP contract and publish curl-first operator docs <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-27-verify-operator-ux-reset/03-task-make-verify-http-errors-and-logs-actionable-at-startup-and-runtime.md`

```
## Task: Make verify HTTP errors and logs actionable at startup and runtime <status>not_started</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-27-verify-operator-ux-reset/04-task-return-full-verify-job-findings-mismatches-and-human-usable-result-json.md`

```
## Task: Return full verify job findings, mismatches, and human-usable result JSON <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete
```

