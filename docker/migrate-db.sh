#!/bin/bash
#
# Copyright 2018-2023 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
#
# Usage: $ ./migrate-db.sh [backup|restore] [FLAGS]

set -e

title() {
  echo -e "\033[1m${1}\033[0m"
}

usage() {
  echo "usage: ./$(basename -- ${0}) [backup|restore] [FLAGS]"
  echo "A script used to migrate Marquez database from Postgres 14 to 16"
  echo
  title "EXAMPLES:"
  echo "  # Backup existing Postgres 14 database and remove volume"
  echo "  $ ./migrate-db.sh backup"
  echo
  echo "  # Restore database to running Postgres 16 instance"
  echo "  $ ./migrate-db.sh restore"
  echo
  title "FLAGS:"
  echo "  -v, --volume string         Database volume name (default: marquez_db-backup)"
  echo "  -f, --file string           Dump file path (default: marquez_dump.sql)"
  echo "  -c, --container string      Target container name for restore (default: marquez-db)"
  echo "  -h, --help                  Show help"
  echo
}

# Defaults
VOLUME_NAME="marquez_db-backup"
DUMP_FILE="marquez_dump.sql"
CONTAINER_NAME="marquez-db"
MODE=""

# Parse args
while [ $# -gt 0 ]; do
  case $1 in
    backup)
       MODE="backup"
       ;;
    restore)
       MODE="restore"
       ;;
    -v|--volume)
       shift
       VOLUME_NAME="${1}"
       ;;
    -f|--file)
       shift
       DUMP_FILE="${1}"
       ;;
    -c|--container)
       shift
       CONTAINER_NAME="${1}"
       ;;
    -h|--help)
       usage
       exit 0
       ;;
    *) 
       if [[ -z "$MODE" ]]; then
           usage
           exit 1
       fi
       ;;
  esac
  shift
done

if [[ -z "$MODE" ]]; then
  usage
  exit 1
fi

if [[ "$MODE" == "backup" ]]; then
  title "Starting Backup..."
  
  if ! docker volume inspect "${VOLUME_NAME}" > /dev/null 2>&1; then
    echo "Error: Volume '${VOLUME_NAME}' does not exist."
    exit 1
  fi

  echo "Starting temporary Postgres 14 container..."
  # We need to specify the user as postgres to ensure permission to dump everything
  # We use --network none to ensure isolation
  CONTAINER_ID=$(docker run -d --rm --network none -v "${VOLUME_NAME}:/var/lib/postgresql/data" postgres:14)
  
  # Wait for Postgres to accept connections
  echo "Waiting for database to initialize..."
  timeout=60
  elapsed=0
  while ! docker exec "$CONTAINER_ID" pg_isready -U postgres > /dev/null 2>&1; do
    sleep 1
    elapsed=$((elapsed+1))
    if [ $elapsed -ge $timeout ]; then
      echo "Timed out waiting for Postgres to start"
      docker stop "$CONTAINER_ID"
      exit 1
    fi
  done

  echo "Dumping database to ${DUMP_FILE}..."
  docker exec "$CONTAINER_ID" pg_dumpall -U postgres -c > "${DUMP_FILE}"

  echo "Stopping temporary container..."
  docker stop "$CONTAINER_ID" > /dev/null

  echo "Backup complete: ${DUMP_FILE}"
  echo
  echo "WARNING: The next step will DELETE the volume '${VOLUME_NAME}'."
  read -p "Are you sure you want to proceed? [y/N] " -n 1 -r
  echo
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    docker volume rm "${VOLUME_NAME}"
    echo "Volume '${VOLUME_NAME}' removed."
    echo "You can now run './docker/up.sh' to start the new Postgres 16 instance."
  else
    echo "Volume deletion skipped. You must remove '${VOLUME_NAME}' manually before starting the new version."
  fi

elif [[ "$MODE" == "restore" ]]; then
  title "Starting Restore..."

  if [[ ! -f "${DUMP_FILE}" ]]; then
    echo "Error: Dump file '${DUMP_FILE}' not found."
    exit 1
  fi

  if ! docker ps | grep -q "${CONTAINER_NAME}"; then
    echo "Error: Container '${CONTAINER_NAME}' is not running."
    echo "Please run './docker/up.sh' first."
    exit 1
  fi

  # Stop API container to release locks on the database
  API_CONTAINER="marquez-api"
  API_WAS_RUNNING=false
  if docker ps | grep -q "${API_CONTAINER}"; then
    echo "Stopping ${API_CONTAINER} to ensure exclusive database access..."
    docker stop "${API_CONTAINER}" > /dev/null
    API_WAS_RUNNING=true
  fi

  echo "Waiting for target database to be ready..."
  timeout=60
  elapsed=0
  while ! docker exec "${CONTAINER_NAME}" pg_isready -U postgres > /dev/null 2>&1; do
    sleep 1
    elapsed=$((elapsed+1))
    if [ $elapsed -ge $timeout ]; then
      echo "Timed out waiting for Postgres to start"
      exit 1
    fi
  done

  echo "Restoring from ${DUMP_FILE}..."
  cat "${DUMP_FILE}" | docker exec -i "${CONTAINER_NAME}" psql -U postgres

  if [ "$API_WAS_RUNNING" = true ]; then
    echo "Restarting ${API_CONTAINER}..."
    docker start "${API_CONTAINER}" > /dev/null
  fi

  echo "Restore complete!"
fi