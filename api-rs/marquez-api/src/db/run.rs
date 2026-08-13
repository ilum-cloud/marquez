// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `runs` and `runs_input_mapping` tables.
//!
//! IMPORTANT: `runs.transitioned_at`, `runs.started_at`, and `runs.ended_at`
//! are `TIMESTAMP` (not `TIMESTAMPTZ`). When SELECTing these columns we cast
//! them to `TIMESTAMPTZ` so that sqlx can decode them into
//! `Option<DateTime<Utc>>`.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{ExtendedRunWithFacetsRow, JobRow, RunRow};

/// Column list used in SELECT queries.
///
/// Casts `transitioned_at`, `started_at`, and `ended_at` from TIMESTAMP to
/// TIMESTAMPTZ so they match the `RunRow` field types.
const RUN_COLUMNS: &str = "\
    uuid, created_at, updated_at, job_uuid, job_version_uuid, \
    parent_run_uuid, run_args_uuid, \
    nominal_start_time::timestamptz AS nominal_start_time, \
    nominal_end_time::timestamptz AS nominal_end_time, \
    current_run_state, \
    started_at::timestamptz AS started_at, \
    start_run_state_uuid, \
    ended_at::timestamptz AS ended_at, \
    end_run_state_uuid, \
    job_name, namespace_name, external_id, location, \
    transitioned_at::timestamptz AS transitioned_at, \
    job_context_uuid";

/// Enriched run columns with LATERAL JOINs for run args, facets, job version,
/// input/output dataset versions, and dataset facets.
///
/// Used by `find_run_by_uuid_with_facets` and `find_runs_by_uuids_with_facets`.
/// Each LATERAL subquery is correlated to `r.uuid`, ensuring index usage instead
/// of full-table scans on `run_facets`, `dataset_versions`, or `dataset_facets`.
const ENRICHED_RUN_LATERAL_SQL: &str = "\
    SELECT r.uuid, r.created_at, r.updated_at, r.job_uuid, r.job_version_uuid, \
           r.parent_run_uuid, r.run_args_uuid, \
           r.nominal_start_time::timestamptz AS nominal_start_time, \
           r.nominal_end_time::timestamptz AS nominal_end_time, \
           r.current_run_state, \
           r.started_at::timestamptz AS started_at, \
           r.start_run_state_uuid, \
           r.ended_at::timestamptz AS ended_at, \
           r.end_run_state_uuid, \
           r.job_name, r.namespace_name, r.external_id, r.location, \
           r.transitioned_at::timestamptz AS transitioned_at, \
           r.job_context_uuid, \
           ra.args, f.facets, \
           jv.version AS job_version, \
           ri.input_versions, ro.output_versions, df.dataset_facets \
    FROM runs_view AS r \
    LEFT JOIN run_args AS ra ON ra.uuid = r.run_args_uuid \
    LEFT JOIN job_versions jv ON jv.uuid = r.job_version_uuid \
    LEFT JOIN LATERAL ( \
        SELECT im.run_uuid, \
               JSON_AGG(json_build_object('namespace', dv.namespace_name, \
                   'name', dv.dataset_name, 'version', dv.version, \
                   'dataset_version_uuid', dv.uuid)) AS input_versions \
        FROM runs_input_mapping im \
        INNER JOIN dataset_versions dv ON im.dataset_version_uuid = dv.uuid \
        WHERE im.run_uuid = r.uuid \
        GROUP BY im.run_uuid \
    ) ri ON ri.run_uuid = r.uuid \
    LEFT JOIN LATERAL ( \
        SELECT r.uuid AS run_uuid, \
               (SELECT jsonb_object_agg(kv.key, kv.value) \
                FROM (SELECT DISTINCT ON (kv2.key) kv2.key, kv2.value \
                      FROM run_facets rf2 \
                      CROSS JOIN LATERAL jsonb_each(rf2.facet) AS kv2(key, value) \
                      WHERE rf2.run_uuid = r.uuid \
                      ORDER BY kv2.key, rf2.lineage_event_time DESC) kv \
               ) AS facets \
    ) AS f ON f.run_uuid = r.uuid \
    LEFT JOIN LATERAL ( \
        SELECT run_uuid, \
               JSON_AGG(json_build_object('namespace', namespace_name, \
                   'name', dataset_name, 'version', version, \
                   'dataset_version_uuid', uuid)) AS output_versions \
        FROM dataset_versions \
        WHERE run_uuid = r.uuid \
        GROUP BY run_uuid \
    ) ro ON ro.run_uuid = r.uuid \
    LEFT JOIN LATERAL ( \
        SELECT run_uuid, \
               JSON_AGG(json_build_object( \
                   'dataset_version_uuid', dataset_version_uuid, \
                   'name', name, 'type', type, 'facet', facet \
               ) ORDER BY created_at ASC) AS dataset_facets \
        FROM dataset_facets \
        WHERE run_uuid = r.uuid AND (LOWER(type) IN ('output', 'input')) \
        GROUP BY run_uuid \
    ) AS df ON r.uuid = df.run_uuid";

/// Upsert a run row.
///
/// INSERT INTO runs ... ON CONFLICT(uuid) DO UPDATE. Returns the row.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    job_uuid: Option<Uuid>,
    job_version_uuid: Option<Uuid>,
    parent_run_uuid: Option<Uuid>,
    run_args_uuid: Option<Uuid>,
    nominal_start_time: Option<DateTime<Utc>>,
    nominal_end_time: Option<DateTime<Utc>>,
    current_run_state: Option<&str>,
    started_at: Option<DateTime<Utc>>,
    start_run_state_uuid: Option<Uuid>,
    ended_at: Option<DateTime<Utc>>,
    end_run_state_uuid: Option<Uuid>,
    namespace_name: &str,
    job_name: &str,
    location: Option<&str>,
    external_id: Option<&str>,
) -> Result<RunRow, sqlx::Error> {
    let q = format!(
        "INSERT INTO runs (\
            uuid, created_at, updated_at, job_uuid, job_version_uuid, \
            parent_run_uuid, run_args_uuid, \
            nominal_start_time, nominal_end_time, \
            current_run_state, started_at, start_run_state_uuid, \
            ended_at, end_run_state_uuid, \
            namespace_name, job_name, location, external_id\
        ) VALUES (\
            $1, $2, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17\
        ) \
        ON CONFLICT(uuid) DO UPDATE SET \
            updated_at = EXCLUDED.updated_at, \
            current_run_state = EXCLUDED.current_run_state, \
            job_version_uuid = COALESCE(EXCLUDED.job_version_uuid, runs.job_version_uuid), \
            transitioned_at = EXCLUDED.updated_at, \
            nominal_start_time = COALESCE(EXCLUDED.nominal_start_time, runs.nominal_start_time), \
            nominal_end_time = COALESCE(EXCLUDED.nominal_end_time, runs.nominal_end_time), \
            started_at = COALESCE(EXCLUDED.started_at, runs.started_at), \
            ended_at = COALESCE(EXCLUDED.ended_at, runs.ended_at), \
            location = EXCLUDED.location, \
            external_id = EXCLUDED.external_id \
        RETURNING {RUN_COLUMNS}"
    );
    sqlx::query_as::<_, RunRow>(&q)
        .bind(uuid)
        .bind(now)
        .bind(job_uuid)
        .bind(job_version_uuid)
        .bind(parent_run_uuid)
        .bind(run_args_uuid)
        .bind(nominal_start_time)
        .bind(nominal_end_time)
        .bind(current_run_state)
        .bind(started_at)
        .bind(start_run_state_uuid)
        .bind(ended_at)
        .bind(end_run_state_uuid)
        .bind(namespace_name)
        .bind(job_name)
        .bind(location)
        .bind(external_id)
        .fetch_one(exec)
        .await
}

/// Returns `true` if a run with the given UUID exists.
pub async fn exists(pool: &PgPool, uuid: Uuid) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM runs WHERE uuid = $1)")
        .bind(uuid)
        .fetch_one(pool)
        .await?;
    Ok(result)
}

/// Find a run by UUID.
pub async fn find_by_uuid(
    pool: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
) -> Result<Option<RunRow>, sqlx::Error> {
    let q = format!("SELECT {RUN_COLUMNS} FROM runs WHERE uuid = $1");
    sqlx::query_as::<_, RunRow>(&q)
        .bind(uuid)
        .fetch_optional(pool)
        .await
}

/// Update `current_run_state` and `transitioned_at` for a run.
pub async fn update_run_state(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    transitioned_at: DateTime<Utc>,
    state: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE runs SET updated_at = $2, current_run_state = $1, transitioned_at = $2 \
         WHERE uuid = $3",
    )
    .bind(state)
    .bind(transitioned_at)
    .bind(uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Update `started_at` and `start_run_state_uuid` for a run.
pub async fn update_start_state(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    started_at: DateTime<Utc>,
    start_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE runs SET updated_at = $1, started_at = $1, start_run_state_uuid = $2 \
         WHERE uuid = $3",
    )
    .bind(started_at)
    .bind(start_uuid)
    .bind(uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Update `ended_at` and `end_run_state_uuid` for a run.
pub async fn update_end_state(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    ended_at: DateTime<Utc>,
    end_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE runs SET updated_at = $1, ended_at = $1, end_run_state_uuid = $2 \
         WHERE uuid = $3",
    )
    .bind(ended_at)
    .bind(end_uuid)
    .bind(uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// List runs ordered by `started_at` descending (nulls last), with limit/offset pagination.
pub async fn find_all(pool: &PgPool, limit: i32, offset: i32) -> Result<Vec<RunRow>, sqlx::Error> {
    let q = format!(
        "SELECT {RUN_COLUMNS} FROM runs ORDER BY started_at DESC NULLS LAST LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, RunRow>(&q)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

/// Find runs for a given job (namespace + job name), ordered by `started_at`
/// descending (nulls last).
pub async fn find_by_job(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<RunRow>, sqlx::Error> {
    let q = format!(
        "SELECT {RUN_COLUMNS} FROM runs \
         WHERE namespace_name = $1 AND job_name = $2 \
         ORDER BY started_at DESC NULLS LAST LIMIT $3 OFFSET $4"
    );
    sqlx::query_as::<_, RunRow>(&q)
        .bind(namespace_name)
        .bind(job_name)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

/// Update `job_version_uuid` for a run.
pub async fn update_job_version(
    exec: impl Executor<'_, Database = Postgres>,
    run_uuid: Uuid,
    job_version_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE runs SET job_version_uuid = $1 WHERE uuid = $2")
        .bind(job_version_uuid)
        .bind(run_uuid)
        .execute(exec)
        .await?;
    Ok(())
}

/// Insert a run-to-dataset-version input mapping.
///
/// ON CONFLICT DO NOTHING to make it idempotent.
pub async fn update_input_mapping(
    exec: impl Executor<'_, Database = Postgres>,
    run_uuid: Uuid,
    dataset_version_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO runs_input_mapping (run_uuid, dataset_version_uuid) \
         VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(run_uuid)
    .bind(dataset_version_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Find a run by UUID with full facets, args, input/output versions, and
/// dataset facets.
///
/// Uses LATERAL JOINs so each subquery is correlated to the single run UUID
/// and uses indexes instead of scanning entire tables.
pub async fn find_run_by_uuid_with_facets(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<Option<ExtendedRunWithFacetsRow>, sqlx::Error> {
    let q = format!("{ENRICHED_RUN_LATERAL_SQL} WHERE r.uuid = $1");
    sqlx::query_as::<_, ExtendedRunWithFacetsRow>(&q)
        .bind(run_uuid)
        .fetch_optional(pool)
        .await
}

/// Batch-fetch multiple runs by UUID with full facets, args, input/output
/// versions, and dataset facets.
///
/// Uses LATERAL JOINs so each subquery is correlated to `r.uuid` and uses
/// indexes. Used by `RunService::list()` to avoid N+1 enrichment.
pub async fn find_runs_by_uuids_with_facets(
    pool: &PgPool,
    uuids: &[Uuid],
) -> Result<Vec<ExtendedRunWithFacetsRow>, sqlx::Error> {
    let q = format!("{ENRICHED_RUN_LATERAL_SQL} WHERE r.uuid = ANY($1)");
    sqlx::query_as::<_, ExtendedRunWithFacetsRow>(&q)
        .bind(uuids)
        .fetch_all(pool)
        .await
}

/// Find runs for the latest version of a job (by namespace + job name),
/// including full facets.
///
/// Resolves job aliases via `jobs_view.aliases`. Uses a CTE to pre-filter
/// run UUIDs, then LATERAL JOINs for enrichment — avoiding full-table scans
/// on `run_facets`, `dataset_versions`, and `dataset_facets`.
pub async fn find_by_latest_job(
    pool: &PgPool,
    namespace: &str,
    job_name: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<ExtendedRunWithFacetsRow>, sqlx::Error> {
    sqlx::query_as::<_, ExtendedRunWithFacetsRow>(
        "WITH filtered_jobs AS ( \
            SELECT jv.uuid FROM jobs_view jv \
            WHERE jv.namespace_name = $1 AND (jv.name = $2 OR $2 = ANY(jv.aliases)) \
        ), \
        target_runs AS ( \
            SELECT r.uuid FROM runs_view r \
            INNER JOIN filtered_jobs fj ON r.job_uuid = fj.uuid \
            ORDER BY r.transitioned_at DESC NULLS LAST, r.started_at DESC NULLS LAST, r.uuid ASC \
            LIMIT $3 OFFSET $4 \
        ) \
        SELECT r.uuid, r.created_at, r.updated_at, r.job_uuid, r.job_version_uuid, \
               r.parent_run_uuid, r.run_args_uuid, \
               r.nominal_start_time::timestamptz AS nominal_start_time, \
               r.nominal_end_time::timestamptz AS nominal_end_time, \
               r.current_run_state, \
               r.started_at::timestamptz AS started_at, \
               r.start_run_state_uuid, \
               r.ended_at::timestamptz AS ended_at, \
               r.end_run_state_uuid, \
               r.job_name, r.namespace_name, r.external_id, r.location, \
               r.transitioned_at::timestamptz AS transitioned_at, \
               r.job_context_uuid, \
               ra.args, f.facets, \
               jv.version AS job_version, \
               ri.input_versions, ro.output_versions, df.dataset_facets \
        FROM runs_view AS r \
        INNER JOIN target_runs tr ON tr.uuid = r.uuid \
        LEFT JOIN run_args AS ra ON ra.uuid = r.run_args_uuid \
        LEFT JOIN job_versions jv ON jv.uuid = r.job_version_uuid \
        LEFT JOIN LATERAL ( \
            SELECT im.run_uuid, \
                   JSON_AGG(json_build_object('namespace', dv.namespace_name, \
                       'name', dv.dataset_name, 'version', dv.version, \
                       'dataset_version_uuid', dv.uuid)) AS input_versions \
            FROM runs_input_mapping im \
            INNER JOIN dataset_versions dv ON im.dataset_version_uuid = dv.uuid \
            WHERE im.run_uuid = r.uuid \
            GROUP BY im.run_uuid \
        ) ri ON ri.run_uuid = r.uuid \
        LEFT JOIN LATERAL ( \
            SELECT r.uuid AS run_uuid, \
                   (SELECT jsonb_object_agg(kv.key, kv.value) \
                    FROM (SELECT DISTINCT ON (kv2.key) kv2.key, kv2.value \
                          FROM run_facets rf2 \
                          CROSS JOIN LATERAL jsonb_each(rf2.facet) AS kv2(key, value) \
                          WHERE rf2.run_uuid = r.uuid \
                          ORDER BY kv2.key, rf2.lineage_event_time DESC) kv \
                   ) AS facets \
        ) AS f ON f.run_uuid = r.uuid \
        LEFT JOIN LATERAL ( \
            SELECT run_uuid, \
                   JSON_AGG(json_build_object('namespace', namespace_name, \
                       'name', dataset_name, 'version', version, \
                       'dataset_version_uuid', uuid)) AS output_versions \
            FROM dataset_versions \
            WHERE run_uuid = r.uuid \
            GROUP BY run_uuid \
        ) ro ON ro.run_uuid = r.uuid \
        LEFT JOIN LATERAL ( \
            SELECT run_uuid, \
                   JSON_AGG(json_build_object( \
                       'dataset_version_uuid', dataset_version_uuid, \
                       'name', name, 'type', type, 'facet', facet \
                   ) ORDER BY created_at ASC) AS dataset_facets \
            FROM dataset_facets \
            WHERE run_uuid = r.uuid AND (LOWER(type) IN ('output', 'input')) \
            GROUP BY run_uuid \
        ) AS df ON r.uuid = df.run_uuid \
        ORDER BY r.transitioned_at DESC NULLS LAST, r.started_at DESC NULLS LAST, r.uuid ASC",
    )
    .bind(namespace)
    .bind(job_name)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Batch-fetch the latest run UUIDs for multiple jobs in a single query.
///
/// For each job UUID, returns up to `limit` run UUIDs ordered by
/// `transitioned_at DESC, started_at DESC`. Resolves job aliases via
/// `jobs_view.aliases`.
///
/// Returns `(run_uuid, job_name, namespace_name)` tuples so the caller
/// can group results by job.
pub async fn find_latest_run_uuids_for_jobs(
    pool: &PgPool,
    job_uuids: &[Uuid],
    limit: i32,
) -> Result<Vec<(Uuid, String, String)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, String, String)>(
        "WITH target_runs AS ( \
            SELECT r.uuid, r.job_name, r.namespace_name, \
                   ROW_NUMBER() OVER ( \
                       PARTITION BY r.namespace_name, r.job_name \
                       ORDER BY r.transitioned_at DESC NULLS LAST, \
                              r.started_at DESC NULLS LAST \
                   ) AS rn \
            FROM runs_view r \
            INNER JOIN jobs_view j ON j.namespace_name = r.namespace_name \
                AND (j.name = r.job_name OR r.job_name = ANY(j.aliases)) \
            WHERE j.uuid = ANY($1) \
        ) \
        SELECT uuid, job_name, namespace_name \
        FROM target_runs \
        WHERE rn <= $2",
    )
    .bind(job_uuids)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Find the `JobRow` associated with a run UUID.
///
/// Joins `jobs_view` to `runs_view` on `job_uuid`.
pub async fn find_job_row_by_run_uuid(
    pool: impl Executor<'_, Database = Postgres>,
    run_uuid: Uuid,
) -> Result<Option<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        "SELECT j.uuid, j.type, j.created_at, j.updated_at, \
                j.namespace_uuid, j.namespace_name, j.name, j.simple_name, \
                j.parent_job_uuid, j.description, j.current_version_uuid, \
                j.current_job_context_uuid, j.current_location, j.current_inputs, \
                j.symlink_target_uuid, false AS is_hidden, j.current_run_uuid, j.aliases \
         FROM jobs_view j \
         INNER JOIN runs_view r ON r.job_uuid = j.uuid \
         WHERE r.uuid = $1",
    )
    .bind(run_uuid)
    .fetch_optional(pool)
    .await
}

/// Update `job_name` and `namespace_name` on a run row.
pub async fn update_run_name(
    exec: impl Executor<'_, Database = Postgres>,
    run_uuid: Uuid,
    job_name: &str,
    namespace_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE runs SET job_name = $1, namespace_name = $2 WHERE uuid = $3")
        .bind(job_name)
        .bind(namespace_name)
        .bind(run_uuid)
        .execute(exec)
        .await?;
    Ok(())
}
