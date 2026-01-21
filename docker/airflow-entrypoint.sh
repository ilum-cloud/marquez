#!/bin/bash
set -e

# Install OpenLineage client if missing (needed for DAGs)
pip install openlineage-python apache-airflow-providers-openlineage

# Function to trigger example DAG
trigger_dag() {
  echo "Waiting for Airflow to be ready..."
  # Wait for a bit to ensure scheduler and webserver are up
  sleep 30
  echo "Triggering example DAG: example_lineage_dag..."
  airflow dags trigger example_lineage_dag || echo "Failed to trigger DAG (it might not be loaded yet)"
}

# Run trigger in background
trigger_dag &

# Run Airflow in standalone mode (handles db init, user creation, and starts services)
airflow standalone