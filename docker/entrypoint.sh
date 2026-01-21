#!/bin/bash
#
# Copyright 2018-2023 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
#
# Usage: $ ./entrypoint.sh

set -e

if [[ -n "${MARQUEZ_DB_HOST}" ]]; then
  export POSTGRES_HOST="${MARQUEZ_DB_HOST}"
elif [[ -n "${POSTGRESQL_HOST}" ]]; then
  export POSTGRES_HOST="${POSTGRESQL_HOST}"
fi

if [[ -n "${MARQUEZ_DB_PORT}" ]]; then
  export POSTGRES_PORT="${MARQUEZ_DB_PORT}"
fi

if [[ -n "${MARQUEZ_DB}" ]]; then
  export POSTGRES_DB="${MARQUEZ_DB}"
fi

if [[ -n "${MARQUEZ_DB_USER}" ]]; then
  export POSTGRES_USER="${MARQUEZ_DB_USER}"
fi

if [[ -n "${MARQUEZ_DB_PASSWORD}" ]]; then
  export POSTGRES_PASSWORD="${MARQUEZ_DB_PASSWORD}"
fi

if [[ -z "${MARQUEZ_CONFIG}" ]]; then
  MARQUEZ_CONFIG='marquez.dev.yml'
  echo "WARNING 'MARQUEZ_CONFIG' not set, using development configuration."
fi

# Adjust java options for the http server
JAVA_OPTS="${JAVA_OPTS} -Duser.timezone=UTC -Dlog4j2.formatMsgNoLookups=true"

# Start http server with java options and configuration
java ${JAVA_OPTS} -jar marquez-*.jar server ${MARQUEZ_CONFIG}
