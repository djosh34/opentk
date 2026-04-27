#!/usr/bin/env bash
set -euo pipefail

cd /workspace

if command -v sqlx >/dev/null 2>&1; then
  sqlx migrate run --source /workspace/migrations
  exit 0
fi

echo "sqlx CLI is unavailable in postgres:16; applying SQLx .up.sql migrations with psql" >&2
for migration in /workspace/migrations/*.up.sql; do
  psql \
    --set ON_ERROR_STOP=1 \
    --username "${POSTGRES_USER}" \
    --dbname "${POSTGRES_DB}" \
    --file "${migration}"
done
