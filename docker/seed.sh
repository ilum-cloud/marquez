#!/bin/bash
#
# Copyright 2018-2023 contributors to the Marquez project
# SPDX-License-Identifier: Apache-2.0
#
# Usage: $ ./seed.sh

set -e

# As ISO-8601 format
NOW=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")

RUN_END_TIME_AFTER_5_MINUTES=$(date -u -d "5 minutes" +"%Y-%m-%dT%H:%M:%S.000Z")
RUN_END_TIME_AFTER_6_MINUTES=$(date -u -d "6 minutes" +"%Y-%m-%dT%H:%M:%S.000Z")
RUN_END_TIME_AFTER_7_MINUTES=$(date -u -d "7 minutes" +"%Y-%m-%dT%H:%M:%S.000Z")
RUN_END_TIME_AFTER_8_MINUTES=$(date -u -d "8 minutes" +"%Y-%m-%dT%H:%M:%S.000Z")
RUN_END_TIME_AFTER_9_MINUTES=$(date -u -d "9 minutes" +"%Y-%m-%dT%H:%M:%S.000Z")
RUN_END_TIME_AFTER_10_MINUTES=$(date -u -d "10 minutes" +"%Y-%m-%dT%H:%M:%S.000Z")

# Replace '{{RUN_START_TIME}}' and '{{RUN_END_TIME_AFTER_*_MINUTES}}'.
sed -e "s/{{RUN_START_TIME}}/$NOW/" \
    -e "s/{{RUN_END_TIME_AFTER_5_MINUTES}}/$RUN_END_TIME_AFTER_5_MINUTES/" \
    -e "s/{{RUN_END_TIME_AFTER_6_MINUTES}}/$RUN_END_TIME_AFTER_6_MINUTES/" \
    -e "s/{{RUN_END_TIME_AFTER_7_MINUTES}}/$RUN_END_TIME_AFTER_7_MINUTES/" \
    -e "s/{{RUN_END_TIME_AFTER_8_MINUTES}}/$RUN_END_TIME_AFTER_8_MINUTES/" \
    -e "s/{{RUN_END_TIME_AFTER_9_MINUTES}}/$RUN_END_TIME_AFTER_9_MINUTES/" \
    -e "s/{{RUN_END_TIME_AFTER_10_MINUTES}}/$RUN_END_TIME_AFTER_10_MINUTES/" \
    metadata.template.json > metadata.json

MARQUEZ_URL="${MARQUEZ_URL:-http://localhost:5000}"
TOTAL=$(jq length metadata.json)
echo "Seeding ${TOTAL} OpenLineage events to ${MARQUEZ_URL}..."
for i in $(seq 0 $((TOTAL - 1))); do
  jq ".[$i]" metadata.json | curl -s -X POST "${MARQUEZ_URL}/api/v1/lineage" \
    -H 'Content-Type: application/json' -d @- > /dev/null
done
echo "Successfully seeded ${TOTAL} events."
