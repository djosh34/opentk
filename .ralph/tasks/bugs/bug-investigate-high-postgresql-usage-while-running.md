## Bug: Investigate High PostgreSQL Usage While Running <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
When OpenTK is running in the Kubernetes cluster, PostgreSQL resource usage can become unexpectedly high. The likely surface area includes API query patterns, sync query patterns, connection churn, pool sizing, transaction behavior, missing or ineffective indexes, unnecessary repeated reads/writes, or long-running database work triggered by normal runtime traffic.

This bug is owned by the OpenTK application repo because the root cause may require code, schema, query, connection-pool, or runtime behavior changes in OpenTK. The live deployment and cluster test loop can use the sibling infrastructure repo at `../hetzner-vps`: build and publish a new OpenTK image from this repo, then use `../hetzner-vps` and its Kubernetes manifests/tools to apply that image to the cluster and verify behavior under real deployment conditions.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD for any application-code, database-query, schema, or configuration behavior fix.
You must make ONE test, and then make ONE test green at the time.

Then verify if the high PostgreSQL usage still holds. If yes, create a new Red test for the next proven cause, and continue with Red-Green TDD until normal runtime behavior no longer causes excessive PostgreSQL usage.
</mandatory_red_green_tdd>

<mandatory_live_verification>
Do not mark this bug passing from local tests, code inspection, or assumed performance improvements alone.

Manually inspect the running OpenTK deployment and PostgreSQL instance under real or representative traffic. Record concrete before/after evidence such as Kubernetes resource usage, PostgreSQL activity/statistics, connection counts, slow or frequent queries, logs, API/sync behavior, and any Grafana or equivalent metrics available.

When testing a fix in the live cluster, build and publish the fixed OpenTK image from this repo, then use the sibling `../hetzner-vps` repo to apply the new image to Kubernetes and verify the deployed workload. Do not read, print, or commit secrets while using the infrastructure repo.
</mandatory_live_verification>

<acceptance_criteria>
- [ ] I reproduced or inspected the high PostgreSQL usage enough to identify the dominant cause or causes.
- [ ] I created a Red unit, integration, or performance-regression test that captures the confirmed OpenTK-side cause.
- [ ] I made the test green by fixing the application code, database query/schema, pooling/config behavior, or other confirmed cause.
- [ ] I manually verified the fix against a real or representative PostgreSQL workload and created another Red test if high usage still remains unexplained.
- [ ] I built and published a new OpenTK image when the fix needs cluster verification.
- [ ] I used `../hetzner-vps` to apply the fixed image to the Kubernetes cluster for live verification when needed.
- [ ] I recorded before/after evidence showing PostgreSQL CPU/load, connection churn, query frequency, or other relevant usage is reduced without breaking API or sync correctness.
- [ ] `make check` — passes cleanly.
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`).
- [ ] `make lint` — passes cleanly.
- [ ] If this bug impacts ultra-long tests, full sync behavior, or long-running performance verification: `make test-long` — passes cleanly or the task records why a stronger live verification supersedes it.
- [ ] No secrets are read, printed, copied into logs, or committed while testing through `../hetzner-vps`.
</acceptance_criteria>
