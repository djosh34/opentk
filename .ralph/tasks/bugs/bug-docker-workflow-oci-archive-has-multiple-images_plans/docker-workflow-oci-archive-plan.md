## Plan: Fix Docker Workflow OCI Archive Publishing

Task: `.ralph/tasks/bugs/bug-docker-workflow-oci-archive-has-multiple-images.md`

Plan path: `.ralph/tasks/bugs/bug-docker-workflow-oci-archive-has-multiple-images_plans/docker-workflow-oci-archive-plan.md`

### Current State

- Required skills were read:
  - `$tdd`: this task is explicitly excepted from TDD because it is a workflow/container publishing bug. Do not add source-text or YAML-shape tests. Verify by running the real publishing workflow and inspecting authenticated GitHub logs.
  - `$improve-code-boundaries`: remove the fragile workflow boundary where shell code manually constructs an OCI layout and hands it to tooling with ambiguous image selection. Prefer a workflow-native/container-tool-native publish path with fewer bespoke layout transformations.
- The active task has no existing `TO BE VERIFIED` or `NOW EXECUTE` marker, so this plan is the required planning artifact before implementation.
- The failure matches the publish job in `.github/workflows/docker.yml`:
  - Build jobs upload per-image/per-platform OCI archives.
  - Publish job extracts the amd64 and arm64 OCI archives.
  - Publish job hand-merges blobs and `index.json` into `/tmp/${image_name}.oci.tar`.
  - Publish job runs `skopeo copy --all "oci-archive:${merged_archive}" "docker://${image}:${tag}"`.
  - The archive contains an OCI index with more than one manifest, while `oci-archive:` without an image reference is ambiguous, producing `more than one image in oci, choose an image`.

### Intended Publishing Contract

- Keep two final images: `ghcr.io/${owner}/opentk-api:${GITHUB_SHA}` and `ghcr.io/${owner}/opentk-sync:${GITHUB_SHA}`.
- Each final tag must be a single multi-platform image index containing `linux/amd64` and `linux/arm64`.
- Do not publish permanent architecture-specific `-amd64` or `-arm64` tags.
- Architecture-specific files may be temporary runner artifacts only.
- Preserve cache behavior:
  - BuildKit GHA cache restore must happen.
  - BuildKit GHA cache save/export must happen when applicable.
  - The explicit cache-backed rebuild step must remain, so logs can prove the second build reuses cache instead of recompiling/redownloading the whole project.

### Boundary Cleanup

Use `$improve-code-boundaries` by deleting the hand-built OCI archive boundary instead of patching around it with more stringly shell layout manipulation:

- Do not manually merge `index.json` files with `jq`.
- Do not copy OCI blob directories by hand.
- Do not create a multi-manifest OCI archive and then ask `skopeo` to infer which image to copy.
- Prefer one of these publish designs after confirming locally from tool behavior:
  1. Use `skopeo copy` to push temporary architecture-specific images to runner-local Docker storage or another non-final local transport, then create and push the final manifest list with `docker buildx imagetools create`.
  2. Or, if local tool support is simpler and deterministic, copy each single-platform OCI archive to GHCR by digest or temporary tag, create the final manifest list, then remove temporary tags in the same publish job. This is acceptable only if logs and registry inspection prove no permanent `-amd64` or `-arm64` final tags remain.
  3. Or use an explicit image reference for `oci-archive:` and a tool-supported manifest/index copy path if it preserves exactly one final multi-platform tag without temporary published tags.
- Choose the smallest design that removes the ambiguous multi-image archive source and keeps the workflow readable.

### Execution Steps

1. Inspect current workflow syntax and action/tool availability.
   - Confirm whether `skopeo`, `docker buildx imagetools`, `oras`, or Docker local image store can safely assemble a multi-platform final tag from the downloaded single-platform OCI artifacts.
   - Prefer installed or apt-installed tools already present in the job over adding a larger dependency.
2. Change `.github/workflows/docker.yml`.
   - Keep the build matrix and artifact upload contract unless a simpler safe publish design requires changing artifact names.
   - Replace the publish job's manual OCI layout merge with the chosen tool-native publish path.
   - Keep final tag calculation as `tag="${GITHUB_SHA}"`.
   - Ensure the only final tags inspected/published are `${image}:${tag}` for `opentk-api` and `opentk-sync`.
3. Run local validation that is meaningful for a workflow task.
   - Run `make lint` because the task requires it and the repository treats lint as mandatory.
   - Run any available workflow syntax/format command if present. Do not add brittle tests that assert YAML strings.
4. Commit and push the workflow fix.
   - Use a commit message starting with `task finished bug-docker-workflow-oci-archive-has-multiple-images:` only after real workflow verification passes; before that, use a normal work-in-progress commit only if needed to trigger the workflow.
   - If a pushed attempt fails, inspect logs, fix, commit, push again, and repeat until the publishing path succeeds.
5. Verify through authenticated GitHub logs using `/home/joshazimullah.linux/github-api-curl`.
   - Find the workflow run triggered by the pushed commit.
   - Confirm the Docker workflow build jobs ran for both images and both platforms.
   - Confirm the publish job ran and no longer contains `more than one image in oci, choose an image`.
   - Confirm `docker buildx imagetools inspect` or equivalent output shows `linux/amd64` and `linux/arm64` under the same commit-SHA tag for both final images.
   - Confirm no permanent `-amd64` or `-arm64` tags were created as final published tags. If temporary registry tags are used, confirm they are deleted or otherwise not left as final tags.
   - Confirm cache restore and cache save/export behavior appears in logs.
   - Confirm the cache-backed rebuild does not recompile/redownload the entire project when cache should be reused.
6. Run required local gates after the final fix is in the working tree:
   - `make check`
   - `make test`
   - `make lint`
   - Do not run `make test-long`; this bug is not a story-end validation and does not explicitly require the long/e2e lane.
7. Final boundary review.
   - Re-read the changed workflow and remove any unnecessary shell complexity or duplicate image-shape representations.
   - If the publish design still depends on ambiguous OCI archive inference or swallowed cleanup errors, fix it before completion.

### Completion

- Tick all acceptance criteria in `.ralph/tasks/bugs/bug-docker-workflow-oci-archive-has-multiple-images.md` only after the real workflow run succeeds and logs prove cache behavior.
- Set `<passes>true</passes>` only after all required local gates and GitHub workflow verification pass.
- Add verification notes to the task file, including:
  - local commands run,
  - pushed commit SHA,
  - GitHub workflow run id/url,
  - evidence for multi-platform final tags,
  - evidence for cache restore and save/export.
- Run `/bin/bash .ralph/task_switch.sh`.
- Add all files, including `.ralph` files.
- Commit with `task finished bug-docker-workflow-oci-archive-has-multiple-images: fix multi-platform image publishing` and include verification evidence in the commit body.
- Push.

NOW EXECUTE
