#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_prefix="${OPENTK_IMAGE_PREFIX:-opentk}"

export DOCKER_BUILDKIT="${DOCKER_BUILDKIT:-1}"

docker build \
  -f "${repo_root}/docker/Dockerfile.sync" \
  -t "${image_prefix}-sync:local" \
  "${repo_root}"

docker build \
  -f "${repo_root}/docker/Dockerfile.api" \
  -t "${image_prefix}-api:local" \
  "${repo_root}"
