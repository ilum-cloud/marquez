#!/bin/bash
#
# Copyright 2024 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
#
# Usage: $ ./entrypoint-rs.sh
# Entrypoint for the Rust Marquez API Docker image.
#
# Configuration precedence, highest first:
#   1. Explicit Figment variables (MARQUEZ_DB__HOST, MARQUEZ_SEARCH__PORT, ...)
#   2. Conventional variables (POSTGRES_*, MARQUEZ_DB_*, POSTGRESQL_HOST, SEARCH_*, ...)
#   3. The configuration file (MARQUEZ_CONFIG, defaults to marquez.dev.yml)

set -e

# Map MARQUEZ_DB_* / POSTGRESQL_HOST aliases onto POSTGRES_* (same logic as the Java entrypoint)
[[ -n "${MARQUEZ_DB_HOST}" ]] && POSTGRES_HOST="${MARQUEZ_DB_HOST}"
[[ -n "${POSTGRESQL_HOST}" && -z "${POSTGRES_HOST}" ]] && POSTGRES_HOST="${POSTGRESQL_HOST}"
[[ -n "${MARQUEZ_DB_PORT}" ]] && POSTGRES_PORT="${MARQUEZ_DB_PORT}"
[[ -n "${MARQUEZ_DB}" ]] && POSTGRES_DB="${MARQUEZ_DB}"
[[ -n "${MARQUEZ_DB_USER}" ]] && POSTGRES_USER="${MARQUEZ_DB_USER}"
[[ -n "${MARQUEZ_DB_PASSWORD}" ]] && POSTGRES_PASSWORD="${MARQUEZ_DB_PASSWORD}"

# Export a Figment variable only when a value was actually provided and the
# Figment variable itself is not already set. Values coming from a mounted
# MARQUEZ_CONFIG file are no longer clobbered by hardcoded defaults.
map_env() {
  local target="$1" value="$2"
  if [[ -n "${value}" && -z "${!target}" ]]; then
    export "${target}=${value}"
  fi
}

map_env MARQUEZ_DB__HOST "${POSTGRES_HOST}"
map_env MARQUEZ_DB__PORT "${POSTGRES_PORT}"
map_env MARQUEZ_DB__NAME "${POSTGRES_DB}"
map_env MARQUEZ_DB__USER "${POSTGRES_USER}"
map_env MARQUEZ_DB__PASSWORD "${POSTGRES_PASSWORD}"
map_env MARQUEZ_DB__MAX_POOL_SIZE "${MARQUEZ_DB_POOL_SIZE}"

map_env MARQUEZ_SERVER__PORT "${MARQUEZ_PORT}"
map_env MARQUEZ_SERVER__ADMIN_PORT "${MARQUEZ_ADMIN_PORT}"

map_env MARQUEZ_MIGRATE_ON_STARTUP "${MIGRATE_ON_STARTUP}"

# Search (OpenSearch) configuration
map_env MARQUEZ_SEARCH__ENABLED "${SEARCH_ENABLED}"
map_env MARQUEZ_SEARCH__HOST "${SEARCH_HOST}"
map_env MARQUEZ_SEARCH__PORT "${SEARCH_PORT}"
map_env MARQUEZ_SEARCH__USERNAME "${SEARCH_USERNAME}"
map_env MARQUEZ_SEARCH__PASSWORD "${SEARCH_PASSWORD}"
map_env MARQUEZ_SEARCH__SCHEME "${SEARCH_SCHEME}"

MARQUEZ_CONFIG="${MARQUEZ_CONFIG:-marquez.dev.yml}"
exec ./marquez-api serve --config "${MARQUEZ_CONFIG}"
