#!/bin/bash
#
# Copyright 2024 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
#
# Usage: $ ./entrypoint-rs.sh
# Entrypoint for the Rust Marquez API Docker image.

set -e

# Map MARQUEZ_DB_* → POSTGRES_* (same logic as Java entrypoint)
[[ -n "${MARQUEZ_DB_HOST}" ]] && export POSTGRES_HOST="${MARQUEZ_DB_HOST}"
[[ -n "${POSTGRESQL_HOST}" && -z "${POSTGRES_HOST}" ]] && export POSTGRES_HOST="${POSTGRESQL_HOST}"
[[ -n "${MARQUEZ_DB_PORT}" ]] && export POSTGRES_PORT="${MARQUEZ_DB_PORT}"
[[ -n "${MARQUEZ_DB}" ]] && export POSTGRES_DB="${MARQUEZ_DB}"
[[ -n "${MARQUEZ_DB_USER}" ]] && export POSTGRES_USER="${MARQUEZ_DB_USER}"
[[ -n "${MARQUEZ_DB_PASSWORD}" ]] && export POSTGRES_PASSWORD="${MARQUEZ_DB_PASSWORD}"

# Map to Figment env vars (MARQUEZ_ prefix, __ for nesting)
# Field names use snake_case to match Rust struct fields (Figment splits on __ for nesting)
export MARQUEZ_DB__HOST="${POSTGRES_HOST:-localhost}"
export MARQUEZ_DB__PORT="${POSTGRES_PORT:-5432}"
export MARQUEZ_DB__NAME="${POSTGRES_DB:-marquez}"
export MARQUEZ_DB__USER="${POSTGRES_USER:-marquez}"
export MARQUEZ_DB__PASSWORD="${POSTGRES_PASSWORD:-marquez}"
export MARQUEZ_DB__MAX_POOL_SIZE="${MARQUEZ_DB_POOL_SIZE:-10}"
export MARQUEZ_SERVER__PORT="${MARQUEZ_PORT:-5000}"
export MARQUEZ_SERVER__ADMIN_PORT="${MARQUEZ_ADMIN_PORT:-5001}"
export MARQUEZ_SERVER__HOST="0.0.0.0"

# Search (OpenSearch) configuration
export MARQUEZ_SEARCH__ENABLED="${SEARCH_ENABLED:-false}"
export MARQUEZ_SEARCH__HOST="${SEARCH_HOST:-opensearch}"
export MARQUEZ_SEARCH__PORT="${SEARCH_PORT:-9200}"
export MARQUEZ_SEARCH__USERNAME="${SEARCH_USERNAME:-admin}"
export MARQUEZ_SEARCH__PASSWORD="${SEARCH_PASSWORD:-CHANGEMEPLEASE1@#a}"
export MARQUEZ_SEARCH__SCHEME="${SEARCH_SCHEME:-http}"

MARQUEZ_CONFIG="${MARQUEZ_CONFIG:-marquez.dev.yml}"
exec ./marquez-api serve --config "${MARQUEZ_CONFIG}"
