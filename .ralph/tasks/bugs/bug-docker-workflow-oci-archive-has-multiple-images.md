## Bug: Docker Workflow OCI Archive Has Multiple Images <status>not_started</status> <passes>false</passes> <priority>ultra high</priority>

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
- [ ] The Docker publishing workflow no longer fails with `more than one image in oci, choose an image`
- [ ] The published image is still multi-platform under the same single tag
- [ ] The workflow publishes exactly one final tag, not permanent `-arm64` and `-amd64` tags
- [ ] Any architecture-specific image names or archives are temporary runner-only implementation details and are not published as final tags
- [ ] The solver committed and git pushed the workflow fix
- [ ] The solver checked the resulting GitHub workflow run with authenticated logs, not just local commands
- [ ] The successful workflow run includes the image publishing path, not only an unrelated lint/test path
- [ ] Cache restore is verified in the successful workflow run logs
- [ ] Cache save/export is verified in the successful workflow run logs when applicable
- [ ] The workflow must not recompile or redownload the entire project when cache should be reused
- [ ] If the workflow recompiles the whole project or redownloads all external dependencies despite an expected cache hit, this bug remains failing and `<passes>` must stay `false`
- [ ] Do not set `<passes>true</passes>` unless the workflow fully succeeds, including the caching behavior above
- [ ] `make lint` passes cleanly for any workflow or script formatting/linting that applies in this repository
</acceptance_criteria>

<plan>
.ralph/tasks/bugs/bug-docker-workflow-oci-archive-has-multiple-images_plans/docker-workflow-oci-archive-plan.md
</plan>
