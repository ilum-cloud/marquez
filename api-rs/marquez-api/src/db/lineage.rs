// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for lineage graph traversal using recursive CTEs.
//!
//! The lineage graph connects jobs via shared datasets. A job's outputs become
//! inputs for downstream jobs. The `job_versions_io_mapping` table tracks
//! which datasets are inputs/outputs for each job version.

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::db::{DatasetDataRow, JobDataRow, RunRow, RunWithFacetsRow, UpstreamRunRow};

/// Get the UUIDs of all jobs in the lineage graph starting from the given
/// job IDs, traversing up to `depth` hops via shared datasets.
///
/// Uses a recursive CTE with cycle detection (path tracking).
pub async fn get_lineage_job_uuids(
    pool: &PgPool,
    depth: i32,
    job_ids: &[Uuid],
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "WITH RECURSIVE lineage AS ( \
            SELECT j.uuid AS job_uuid, 0 AS depth, \
                   ARRAY[j.uuid] AS path \
            FROM jobs j \
            WHERE j.uuid = ANY($1) \
            \
            UNION ALL \
            \
            SELECT DISTINCT j2.uuid AS job_uuid, l.depth + 1 AS depth, \
                   l.path || j2.uuid \
            FROM lineage l \
            JOIN job_versions_io_mapping io1 \
                ON io1.job_uuid = l.job_uuid AND io1.is_current_job_version = true \
            JOIN job_versions_io_mapping io2 \
                ON io2.dataset_uuid = io1.dataset_uuid \
                AND io2.job_uuid != l.job_uuid \
                AND io2.is_current_job_version = true \
            JOIN jobs j2 ON j2.uuid = io2.job_uuid \
            WHERE l.depth < $2 \
            AND NOT j2.uuid = ANY(l.path) \
        ) \
        SELECT DISTINCT job_uuid FROM lineage",
    )
    .bind(job_ids)
    .bind(depth)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(uuid,)| uuid).collect())
}

/// Get upstream runs by tracing backwards through dataset lineage.
///
/// Starting from a given run, finds the datasets it consumed (inputs),
/// then finds the runs that produced those datasets, and recurses.
/// Returns enriched rows with run metadata and input dataset info,
/// matching Java's `RunLineageDao.getUpstreamRuns()`.
pub async fn get_upstream_runs(
    pool: &PgPool,
    run_id: Uuid,
    depth: i32,
) -> Result<Vec<UpstreamRunRow>, sqlx::Error> {
    sqlx::query_as::<_, UpstreamRunRow>(
        "WITH RECURSIVE upstream_runs( \
            r_uuid, \
            dataset_uuid, dataset_version_uuid, dataset_namespace, dataset_name, \
            u_r_uuid, \
            depth \
        ) AS ( \
            SELECT r.uuid, \
                   dv.dataset_uuid, dv.version, dv.namespace_name, dv.dataset_name, \
                   dv.run_uuid, \
                   0 AS depth \
            FROM (SELECT $1::uuid AS uuid) r \
            LEFT JOIN runs_input_mapping rim ON rim.run_uuid = r.uuid \
            LEFT JOIN dataset_versions dv ON dv.uuid = rim.dataset_version_uuid \
            \
            UNION \
            \
            SELECT ur.u_r_uuid, \
                   dv2.dataset_uuid, dv2.version, dv2.namespace_name, dv2.dataset_name, \
                   dv2.run_uuid, \
                   ur.depth + 1 AS depth \
            FROM upstream_runs ur \
            LEFT JOIN runs_input_mapping rim2 ON rim2.run_uuid = ur.u_r_uuid \
            LEFT JOIN dataset_versions dv2 ON dv2.uuid = rim2.dataset_version_uuid \
            WHERE ur.u_r_uuid IS NOT NULL \
              AND ur.u_r_uuid <> ur.r_uuid \
              AND depth < $2 \
        ) \
        SELECT * FROM ( \
            SELECT DISTINCT ON (upstream_runs.r_uuid, upstream_runs.dataset_version_uuid, upstream_runs.u_r_uuid) \
                upstream_runs.r_uuid, \
                upstream_runs.dataset_uuid, \
                upstream_runs.dataset_version_uuid, \
                upstream_runs.dataset_namespace, \
                upstream_runs.dataset_name, \
                upstream_runs.u_r_uuid, \
                upstream_runs.depth, \
                r.started_at::timestamptz AS started_at, \
                r.ended_at::timestamptz AS ended_at, \
                r.current_run_state AS state, \
                r.job_uuid, \
                r.job_version_uuid, \
                r.namespace_name AS job_namespace, \
                r.job_name \
            FROM upstream_runs, runs r \
            WHERE upstream_runs.r_uuid = r.uuid \
        ) sub \
        ORDER BY depth ASC, job_name ASC",
    )
    .bind(run_id)
    .bind(depth)
    .fetch_all(pool)
    .await
}

/// Column list for `jobs_view` matching `JobDataRow` fields (without `is_hidden`).
const JOB_DATA_COLUMNS: &str = "\
    j.uuid, j.type, j.created_at, j.updated_at, j.namespace_uuid, \
    j.namespace_name, j.name, j.simple_name, j.parent_job_uuid, \
    j.parent_job_name, \
    j.description, j.current_version_uuid, j.current_job_context_uuid, \
    j.current_location, j.current_inputs, j.symlink_target_uuid, \
    j.current_run_uuid, j.aliases";

/// Column list for `RunRow` with TIMESTAMP-to-TIMESTAMPTZ casts.
///
/// Used in `get_current_runs()` where we select directly from the `runs`
/// table (which stores some columns as plain `TIMESTAMP`).
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

/// Get the full lineage graph starting from the given job IDs, traversing
/// up to `depth` hops via shared datasets.
///
/// Returns `JobDataRow` rows (from `jobs_view`) enriched with
/// `input_uuids` and `output_uuids` arrays from the IO mapping.
///
/// Uses a recursive CTE that mirrors the Java `LineageDao.getLineage()`
/// algorithm: `array_cat` + `&&` overlap to find connected jobs.
/// A secondary `lineage_outside_job_io` CTE catches seed jobs that have
/// no IO mapping at all (so they are still returned).
pub async fn get_lineage(
    pool: &PgPool,
    depth: i32,
    job_ids: &[Uuid],
) -> Result<Vec<JobDataRow>, sqlx::Error> {
    let q = format!(
        "WITH RECURSIVE \
            job_io AS ( \
                SELECT io.job_uuid AS job_uuid, \
                       io.job_symlink_target_uuid AS job_symlink_target_uuid, \
                       ARRAY_AGG(DISTINCT io.dataset_uuid) \
                           FILTER (WHERE io.io_type = 'INPUT') AS inputs, \
                       ARRAY_AGG(DISTINCT io.dataset_uuid) \
                           FILTER (WHERE io.io_type = 'OUTPUT') AS outputs \
                FROM job_versions_io_mapping io \
                WHERE io.is_current_job_version = TRUE \
                GROUP BY io.job_symlink_target_uuid, io.job_uuid \
            ), \
            lineage(job_uuid, job_symlink_target_uuid, inputs, outputs) AS ( \
                SELECT job_uuid, job_symlink_target_uuid, \
                       COALESCE(inputs, ARRAY[]::uuid[]) AS inputs, \
                       COALESCE(outputs, ARRAY[]::uuid[]) AS outputs, \
                       0 AS depth \
                FROM job_io \
                WHERE job_uuid = ANY($1) \
                   OR job_symlink_target_uuid = ANY($1) \
                UNION \
                SELECT io.job_uuid, io.job_symlink_target_uuid, \
                       io.inputs, io.outputs, l.depth + 1 \
                FROM job_io io, lineage l \
                WHERE io.job_uuid != l.job_uuid \
                  AND array_cat(io.inputs, io.outputs) \
                      && array_cat(l.inputs, l.outputs) \
                  AND depth < $2 \
            ), \
            lineage_outside_job_io(job_uuid, job_symlink_target_uuid, \
                                   inputs, outputs, depth) AS ( \
                SELECT param_jobs.param_job_uuid AS job_uuid, \
                       jj.symlink_target_uuid, \
                       ARRAY[]::uuid[] AS inputs, \
                       ARRAY[]::uuid[] AS outputs, \
                       0 AS depth \
                FROM (SELECT unnest($1::UUID[]) AS param_job_uuid) param_jobs \
                LEFT JOIN lineage l ON param_jobs.param_job_uuid = l.job_uuid \
                INNER JOIN jobs jj ON jj.uuid = param_jobs.param_job_uuid \
                WHERE l.job_uuid IS NULL \
            ) \
        SELECT DISTINCT ON (j.uuid) {JOB_DATA_COLUMNS}, \
               inputs AS input_uuids, outputs AS output_uuids \
        FROM (SELECT * FROM lineage UNION SELECT * FROM lineage_outside_job_io) l2 \
        INNER JOIN jobs_view j \
            ON (j.uuid = l2.job_uuid OR j.uuid = l2.job_symlink_target_uuid)"
    );
    sqlx::query_as::<_, JobDataRow>(&q)
        .bind(job_ids)
        .bind(depth)
        .fetch_all(pool)
        .await
}

/// Get a parent job's data row by child job UUID.
///
/// Returns a single `JobDataRow` with NULL input/output UUIDs.
pub async fn get_parent_job_data(
    pool: &PgPool,
    job_id: Uuid,
) -> Result<Option<JobDataRow>, sqlx::Error> {
    let q = format!(
        "SELECT {JOB_DATA_COLUMNS}, \
                NULL::uuid[] AS input_uuids, \
                NULL::uuid[] AS output_uuids \
         FROM jobs_view j \
         WHERE j.parent_job_uuid = $1 \
         ORDER BY j.created_at DESC, j.uuid DESC \
         LIMIT 1"
    );
    sqlx::query_as::<_, JobDataRow>(&q)
        .bind(job_id)
        .fetch_optional(pool)
        .await
}

/// Get dataset data rows for the given dataset UUIDs.
///
/// Joins `datasets_view` with `dataset_versions` and `dataset_symlinks`
/// to include version fields and lifecycle state.
pub async fn get_dataset_data(
    pool: &PgPool,
    ds_uuids: &[Uuid],
) -> Result<Vec<DatasetDataRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetDataRow>(
        "SELECT ds.uuid, ds.type, ds.created_at, ds.updated_at, \
                ds.namespace_uuid, ds.namespace_name, \
                ds.source_uuid, ds.source_name, \
                ds.name, ds.physical_name, ds.description, \
                ds.current_version_uuid, \
                ds.last_modified_at::timestamptz AS last_modified_at, \
                dv.fields, dv.lifecycle_state \
         FROM datasets_view ds \
         LEFT JOIN dataset_versions dv ON dv.uuid = ds.current_version_uuid \
         LEFT JOIN dataset_symlinks dsym \
             ON dsym.namespace_uuid = ds.namespace_uuid AND dsym.name = ds.name \
         WHERE dsym.is_primary = true AND ds.uuid = ANY($1)",
    )
    .bind(ds_uuids)
    .fetch_all(pool)
    .await
}

/// Get dataset data rows by namespace and dataset name (for fallback path).
///
/// Two-step approach: first resolves the dataset UUID via any matching row
/// in `datasets_view` (which may be a symlink), then delegates to
/// `get_dataset_data()` which filters by `is_primary = true` — ensuring
/// we always return the primary symlink row regardless of which
/// namespace/name alias was queried. Matches Java's `LineageDao` behaviour.
pub async fn get_dataset_data_by_name(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
) -> Result<Vec<DatasetDataRow>, sqlx::Error> {
    // Step 1: Find the dataset UUID via any matching row in datasets_view
    let uuid_row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT ds.uuid FROM datasets_view ds \
         WHERE ds.namespace_name = $1 AND ds.name = $2 \
         LIMIT 1",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .fetch_optional(pool)
    .await?;

    // Step 2: Use get_dataset_data() which has the is_primary filter
    match uuid_row {
        Some((uuid,)) => get_dataset_data(pool, &[uuid]).await,
        None => Ok(vec![]),
    }
}

/// Find the job UUID that has a given dataset as input or output.
///
/// Returns at most one UUID, preferring outputs over inputs (ORDER BY
/// `io_type DESC`) and newer job versions.
pub async fn get_job_from_input_or_output(
    pool: &PgPool,
    dataset_name: &str,
    namespace_name: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT j.uuid \
         FROM jobs j \
         INNER JOIN job_versions jv ON jv.job_uuid = j.uuid \
         INNER JOIN job_versions_io_mapping io ON io.job_version_uuid = jv.uuid \
         INNER JOIN datasets_view ds ON ds.uuid = io.dataset_uuid \
         WHERE ds.name = $1 AND ds.namespace_name = $2 \
         ORDER BY io_type DESC, jv.created_at DESC \
         LIMIT 1",
    )
    .bind(dataset_name)
    .bind(namespace_name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(uuid,)| uuid))
}

/// Get the current (latest) runs with facets for the given job UUIDs.
///
/// Uses LATERAL JOINs to aggregate run args, input/output dataset versions,
/// and run facets. TIMESTAMP columns are cast to TIMESTAMPTZ.
pub async fn get_current_runs_with_facets(
    pool: &PgPool,
    job_uuids: &[Uuid],
) -> Result<Vec<RunWithFacetsRow>, sqlx::Error> {
    sqlx::query_as::<_, RunWithFacetsRow>(
        "WITH latest_runs AS ( \
            SELECT DISTINCT ON(r.job_name, r.namespace_name) r.*, jv.version \
            FROM runs_view r \
            INNER JOIN job_versions jv ON jv.uuid = r.job_version_uuid \
            INNER JOIN jobs_view j ON j.uuid = jv.job_uuid \
            WHERE j.uuid = ANY($1) OR j.symlink_target_uuid = ANY($1) \
            ORDER BY r.job_name, r.namespace_name, r.created_at DESC \
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
               r.version AS job_version, ri.input_versions, ro.output_versions \
        FROM latest_runs AS r \
        LEFT JOIN run_args AS ra ON ra.uuid = r.run_args_uuid \
        LEFT JOIN LATERAL ( \
            SELECT im.run_uuid, \
                   JSON_AGG(json_build_object('namespace', dv.namespace_name, \
                                              'name', dv.dataset_name, \
                                              'version', dv.version)) AS input_versions \
            FROM runs_input_mapping im \
            INNER JOIN dataset_versions dv ON im.dataset_version_uuid = dv.uuid \
            WHERE im.run_uuid = r.uuid \
            GROUP BY im.run_uuid \
        ) ri ON ri.run_uuid = r.uuid \
        LEFT JOIN LATERAL ( \
            SELECT r.uuid AS run_uuid, \
                   (SELECT jsonb_object_agg(sub.name, sub.facet) \
                    FROM (SELECT DISTINCT ON (rf2.name) rf2.name, rf2.facet \
                          FROM run_facets rf2 \
                          WHERE rf2.run_uuid = r.uuid \
                          ORDER BY rf2.name, rf2.lineage_event_time DESC) sub \
                   ) AS facets \
        ) AS f ON f.run_uuid = r.uuid \
        LEFT JOIN LATERAL ( \
            SELECT run_uuid, \
                   JSON_AGG(json_build_object('namespace', namespace_name, \
                                              'name', dataset_name, \
                                              'version', version)) AS output_versions \
            FROM dataset_versions \
            WHERE run_uuid = r.uuid \
            GROUP BY run_uuid \
        ) ro ON ro.run_uuid = r.uuid",
    )
    .bind(job_uuids)
    .fetch_all(pool)
    .await
}

/// Get the current (latest) runs for the given job UUIDs.
///
/// Simpler version of `get_current_runs_with_facets` -- no facets, no
/// LATERAL JOINs. Returns plain `RunRow` rows.
pub async fn get_current_runs(
    pool: &PgPool,
    job_uuids: &[Uuid],
) -> Result<Vec<RunRow>, sqlx::Error> {
    let q = format!(
        "WITH latest_runs AS ( \
            SELECT current_run_uuid, current_version_uuid AS job_version \
            FROM jobs j \
            WHERE j.uuid = ANY($1) OR j.symlink_target_uuid = ANY($1) \
        ) \
        SELECT {RUN_COLUMNS} \
        FROM runs \
        INNER JOIN latest_runs ON runs.uuid = latest_runs.current_run_uuid \
        ORDER BY runs.created_at DESC"
    );
    sqlx::query_as::<_, RunRow>(&q)
        .bind(job_uuids)
        .fetch_all(pool)
        .await
}
