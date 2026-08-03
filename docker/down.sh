#!/bin/bash
#
# Copyright 2018-2023 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
#
# Usage: $ ./down.sh [FLAGS]

# This script uses bash features (`+=`, `${RANDOM}`) that POSIX shells do not
# implement. Re-exec under bash so `sh ./docker/down.sh` works too (see up.sh).
if [ -z "${BASH_VERSION:-}" ]; then
  if ! command -v bash > /dev/null 2>&1; then
    echo "ERROR: ./docker/down.sh requires bash, but bash was not found on PATH." >&2
    exit 1
  fi
  exec bash "$0" "$@"
fi

set -e

title() {
  echo -e "\033[1m${1}\033[0m"
}

usage() {
  echo "usage: ./$(basename -- ${0}) [FLAGS]"
  echo "A script used to bring down Marquez when run via Docker"
  echo
  title "EXAMPLES:"
  echo "  # Stop and remove all containers"
  echo "  $ ./down.sh"
  echo
  echo "  # Stop and remove all containers, remove volumes"
  echo "  $ ./down.sh -v"
  echo
  title "FLAGS:"
  echo "  -v, --volumes         remove created volumes"
  echo "  -h, --help            show help for script"
  echo
}

# Change working directory to project root
project_root=$(git rev-parse --show-toplevel)
cd "${project_root}/"

compose_files="-f docker-compose.yml"

# Default args
compose_args="--remove-orphans"

# Parse args
while [ $# -gt 0 ]; do
  case $1 in
    -h|'--help')
       usage
       exit 0
       ;;
    -v|'--volumes')
       compose_args+=" -v"
       ;;
  esac
  shift
done

# We can ignore the tag and port(s) when cleaning up running
# containers and volumes
API_PORT=${RANDOM} API_ADMIN_PORT=${RANDOM} WEB_PORT=${RANDOM} TAG=${RANDOM} POSTGRES_PORT=${RANDOM} SEARCH_ENABLED=${RANDOM} \
  docker compose $compose_files down $compose_args
