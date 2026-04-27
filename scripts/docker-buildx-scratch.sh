#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_prefix="${OPENTK_IMAGE_PREFIX:-opentk}"
artifact_image_prefix="${OPENTK_ARTIFACT_IMAGE_PREFIX:-opentk-scratch-artifacts}"
builder_name="${OPENTK_BUILDX_BUILDER:-default}"
platforms="linux/amd64,linux/arm64"
max_image_size_bytes=50000000

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required command is missing: $1" >&2
    exit 1
  fi
}

ensure_builder() {
  if [[ "${builder_name}" == "default" ]]; then
    docker buildx use default
    docker buildx inspect --bootstrap >/dev/null
    return
  fi

  if ! docker buildx inspect "${builder_name}" >/dev/null 2>&1; then
    docker buildx create --name "${builder_name}" --driver docker-container --use
  else
    docker buildx use "${builder_name}"
  fi

  docker buildx inspect --bootstrap >/dev/null
}

host_platform() {
  case "$(docker version --format '{{.Server.Arch}}')" in
    amd64|x86_64) echo "linux/amd64" ;;
    arm64|aarch64) echo "linux/arm64" ;;
    *)
      echo "unsupported Docker host architecture: $(docker version --format '{{.Server.Arch}}')" >&2
      exit 1
      ;;
  esac
}

assert_no_qemu_builder() {
  local builder_details
  builder_details="$(docker buildx inspect "${builder_name}")"
  if grep -Eqi 'QEMU|emulat' <<<"${builder_details}"; then
    echo "buildx builder reports QEMU/emulation support; scratch artifact builds must use native Rust cross-compilation instead" >&2
    exit 1
  fi
}

build_artifacts() {
  local binary="$1"
  local artifact_image="${artifact_image_prefix}-${binary}:local"

  docker buildx build \
    --builder "${builder_name}" \
    --load \
    --build-arg "BINARY=${binary}" \
    -f "${repo_root}/docker/Dockerfile.scratch-artifacts" \
    -t "${artifact_image}" \
    "${repo_root}"
}

build_final_image() {
  local binary="$1"
  local artifact_image="${artifact_image_prefix}-${binary}:local"
  local tag="${image_prefix}-${binary#opentk-}:local"
  local metadata_file
  metadata_file="$(mktemp)"
  trap 'rm -f "${metadata_file}"' RETURN

  docker buildx build \
    --builder "${builder_name}" \
    --platform "${platforms}" \
    --metadata-file "${metadata_file}" \
    --build-arg "BINARY=${binary}" \
    --build-arg "ARTIFACT_IMAGE=${artifact_image}" \
    -f "${repo_root}/docker/Dockerfile.scratch" \
    -t "${tag}" \
    --load \
    "${repo_root}"

  verify_manifest_platforms "${tag}" "${metadata_file}"
  verify_host_image "${tag}"
  rm -f "${metadata_file}"
  trap - RETURN
}

verify_manifest_platforms() {
  local tag="$1"
  local metadata_file="$2"
  for platform in linux/amd64 linux/arm64; do
    if ! grep -Fq "buildx.build.provenance/${platform}" "${metadata_file}"; then
      echo "image ${tag} is missing manifest platform ${platform}" >&2
      exit 1
    fi
  done
}

verify_host_image() {
  local tag="$1"
  local size

  docker run --rm --platform "$(host_platform)" "${tag}" --help >/dev/null

  size="$(docker image inspect "${tag}" --format '{{.Size}}')"
  if [[ "${size}" -gt "${max_image_size_bytes}" ]]; then
    echo "image ${tag} is ${size} bytes; limit is ${max_image_size_bytes}" >&2
    exit 1
  fi
}

require_command docker
docker buildx version >/dev/null
ensure_builder
assert_no_qemu_builder
build_artifacts opentk-sync
build_final_image opentk-sync
build_artifacts opentk-api
build_final_image opentk-api
