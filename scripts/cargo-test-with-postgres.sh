#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PG_DIR="${ROOT_DIR}/target/test-postgres"
PG_DATA="${PG_DIR}/data"
PG_LOG="${PG_DIR}/postgres.log"
PG_SOCKET_DIR="${TMPDIR:-/tmp}/opentk-test-postgres-${USER}"
PG_PORT="${OPENTK_TEST_POSTGRES_PORT:-55432}"
PG_HOST="127.0.0.1"

mkdir -p "${PG_DIR}"
mkdir -p "${PG_SOCKET_DIR}"

if [[ -d "${PG_DATA}" && ! -f "${PG_DATA}/PG_VERSION" ]]; then
  mv "${PG_DATA}" "${PG_DATA}.invalid.$(date +%s)"
fi

if [[ ! -d "${PG_DATA}" ]]; then
  initdb -D "${PG_DATA}" -A trust -U postgres >/dev/null
fi

if ! pg_isready -h "${PG_HOST}" -p "${PG_PORT}" -U postgres >/dev/null 2>&1; then
  pg_ctl \
    -D "${PG_DATA}" \
    -l "${PG_LOG}" \
    -o "-F -p ${PG_PORT} -h ${PG_HOST} -k ${PG_SOCKET_DIR}" \
    start >/dev/null
fi

export OPENTK_TEST_DATABASE_URL="${OPENTK_TEST_DATABASE_URL:-postgres://postgres@${PG_HOST}:${PG_PORT}/postgres}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
exec cargo test --workspace "$@"
