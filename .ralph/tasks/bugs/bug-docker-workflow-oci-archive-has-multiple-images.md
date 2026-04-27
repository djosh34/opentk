## Bug: Docker Workflow OCI Archive Has Multiple Images <status>done</status> <passes>true</passes> <priority>ultra high</priority>

<description>
The Docker publishing workflow fails while initializing an OCI archive:

```text
time="2026-04-27T17:56:19Z" level=fatal msg="initializing source oci-archive:/tmp/opentk-api.oci.tar:: more than one image in oci, choose an image"
```

The workflow is producing or handing off an OCI archive with more than one image where the consuming step expects one selected image. The fix must preserve the intended publishing contract exactly:

- The published image MUST remain multi-platform under the same single tag.
- The workflow MUST publish exactly one final tag.
- It MUST NOT publish separate permanent `-arm64` and `-amd64` tags.
- Temporary architecture-specific images or artifacts may exist inside the runner only if needed, but they must not be published as final tags.

This is a workflow/container publishing bug, not an application-code bug. TDD is not allowed for this task; verify with actual workflow execution and authenticated GitHub workflow logs.
</description>

<manual_verification_required>
Do not mark this bug as passing based on local reasoning, YAML validation alone, or a partially green workflow.

The solver must keep committing and git pushing fixes, then inspect the GitHub workflow run logs, until the workflow fully works.

Use the authenticated GitHub API wrapper where needed:

```sh
/home/joshazimullah.linux/github-api-curl
```

The bug is only fixed when the real workflow run completes successfully and the logs prove the cache behavior is working.
</manual_verification_required>

<acceptance_criteria>
- [x] The Docker publishing workflow no longer fails with `more than one image in oci, choose an image`
- [x] The published image is still multi-platform under the same single tag
- [x] The workflow publishes exactly one final tag, not permanent `-arm64` and `-amd64` tags
- [x] Any architecture-specific image names or archives are temporary runner-only implementation details and are not published as final tags
- [x] The solver committed and git pushed the workflow fix
- [x] The solver checked the resulting GitHub workflow run with authenticated logs, not just local commands
- [x] The successful workflow run includes the image publishing path, not only an unrelated lint/test path
- [x] Cache restore is verified in the successful workflow run logs
- [x] Cache save/export is verified in the successful workflow run logs when applicable
- [x] The workflow must not recompile or redownload the entire project when cache should be reused
- [x] If the workflow recompiles the whole project or redownloads all external dependencies despite an expected cache hit, this bug remains failing and `<passes>` must stay `false`
- [x] Do not set `<passes>true</passes>` unless the workflow fully succeeds, including the caching behavior above
- [x] `make lint` passes cleanly for any workflow or script formatting/linting that applies in this repository
</acceptance_criteria>

<plan>
.ralph/tasks/bugs/bug-docker-workflow-oci-archive-has-multiple-images_plans/docker-workflow-oci-archive-plan.md
</plan>

<verification>
Local gates:

- `make lint` passed.
- `make check` passed.
- `make test` passed.
- `make test-long` was not run, per normal non-story-finishing task instructions.

Workflow verification:

- Pushed verification commit: `73e88c0f23f148a431bad1cd3ab85f62bf130ddb`.
- GitHub Actions Docker run: `25011833019`, https://github.com/djosh34/opentk/actions/runs/25011833019.
- Run conclusion: success.
- Build jobs succeeded for:
  - `Build opentk-api linux/amd64`
  - `Build opentk-api linux/arm64`
  - `Build opentk-sync linux/amd64`
  - `Build opentk-sync linux/arm64`
- Publish job `Publish GHCR manifests` succeeded.
- Downloaded authenticated run logs with `/home/joshazimullah.linux/github-api-curl`.
- Logs contain no `more than one image`, `oci-archive`, or `skopeo` failure.
- Publish logs show exactly the final commit-SHA tags:
  - `ghcr.io/djosh34/opentk-api:73e88c0f23f148a431bad1cd3ab85f62bf130ddb`
  - `ghcr.io/djosh34/opentk-sync:73e88c0f23f148a431bad1cd3ab85f62bf130ddb`
- `docker buildx imagetools inspect` output in the publish logs shows both final tags are manifest lists with `linux/amd64` and `linux/arm64` manifests.
- The workflow no longer publishes permanent architecture-specific tags; matrix builds push untagged digest references only, and the publish job creates one final commit-SHA tag per image.
- Cache-mount restore logs show `Cache restored successfully` for all four build jobs.
- Cache-mount post-job logs show `Cache saved with key` for all four build jobs.
- BuildKit logs show `importing cache manifest from gha:` and `preparing build cache for export`.
- The cache-backed rebuild step logs show the expensive build layers as `CACHED`, so it did not redownload/recompile the whole project on the verification rebuild.
</verification>
