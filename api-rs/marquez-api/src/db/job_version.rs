// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `job_versions` and `job_versions_io_mapping` tables.
//!
//! `job_versions_io_mapping.made_current_at` is `TIMESTAMP` (NaiveDateTime),
//! not `TIMESTAMPTZ`.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{EnrichedJobVersionRow, JobVersionIoMappingRow, JobVersionRow};

/// Upsert a job version row.
///
/// INSERT ON CONFLICT(version) DO UPDATE SET location. Returns the row.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    job_uuid: Uuid,
    location: Option<&str>,
    version: Uuid,
    job_context_uuid: Option<Uuid>,
    namespace_uuid: Option<Uuid>,
    namespace_name: &str,
    job_name: &str,
) -> Result<JobVersionRow, sqlx::Error> {
    sqlx::query_as::<_, JobVersionRow>(
        "INSERT INTO job_versions (\
            uuid, created_at, updated_at, job_uuid, location, version, \
            job_context_uuid, namespace_uuid, namespace_name, job_name\
        ) VALUES ($1, $2, $2, $3, $4, $5, $6, $7, $8, $9) \
        ON CONFLICT(version) DO UPDATE SET \
            updated_at = EXCLUDED.updated_at \
        RETURNING *",
    )
    .bind(uuid)
    .bind(now)
    .bind(job_uuid)
    .bind(location)
    .bind(version)
    .bind(job_context_uuid)
    .bind(namespace_uuid)
    .bind(namespace_name)
    .bind(job_name)
    .fetch_one(exec)
    .await
}

/// Find a specific job version by namespace, job name, and version UUID.
pub async fn find_job_version(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
    version: Uuid,
) -> Result<Option<JobVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, JobVersionRow>(
        "SELECT * FROM job_versions \
         WHERE namespace_name = $1 AND job_name = $2 AND version = $3",
    )
    .bind(namespace_name)
    .bind(job_name)
    .bind(version)
    .fetch_optional(pool)
    .await
}

/// Find a job version by its internal UUID (row primary key).
pub async fn find_by_internal_uuid(
    pool: &PgPool,
    uuid: Uuid,
) -> Result<Option<JobVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, JobVersionRow>("SELECT * FROM job_versions WHERE uuid = $1")
        .bind(uuid)
        .fetch_optional(pool)
        .await
}

/// List job versions for a given job, ordered by `created_at` descending.
pub async fn find_all(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<JobVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, JobVersionRow>(
        "SELECT * FROM job_versions \
         WHERE namespace_name = $1 AND job_name = $2 \
         ORDER BY created_at DESC, version DESC \
         LIMIT $3 OFFSET $4",
    )
    .bind(namespace_name)
    .bind(job_name)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Upsert an INPUT dataset mapping for a job version.
///
/// Marks previous INPUT mappings for the same `(dataset_uuid, job_uuid)` as
/// not current (excluding the current job_version_uuid), then inserts the
/// new mapping. Matches Java's `JobVersionDao.upsertInputDatasetFor()`.
pub async fn upsert_input_dataset(
    pool: &PgPool,
    job_version_uuid: Uuid,
    dataset_uuid: Uuid,
    job_uuid: Uuid,
    job_symlink_target_uuid: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    // Step 1: Mark previous INPUT mappings as not current.
    // Guard: exclude the current job_version_uuid to avoid clearing the row
    // we're about to insert. Also include symlink targets (matches Java).
    sqlx::query(
        "UPDATE job_versions_io_mapping \
         SET is_current_job_version = false \
         WHERE (job_uuid = $2 OR job_symlink_target_uuid = $2) \
         AND job_version_uuid != $1 \
         AND io_type = 'INPUT' \
         AND is_current_job_version = true",
    )
    .bind(job_version_uuid)
    .bind(job_uuid)
    .execute(pool)
    .await?;

    // Step 2: Insert new mapping.
    sqlx::query(
        "INSERT INTO job_versions_io_mapping (\
            job_version_uuid, dataset_uuid, io_type, job_uuid, \
            job_symlink_target_uuid, is_current_job_version, made_current_at\
        ) VALUES ($1, $2, 'INPUT', $3, $4, true, $5) \
        ON CONFLICT (job_version_uuid, dataset_uuid, io_type, job_uuid) \
        DO UPDATE SET is_current_job_version = TRUE, \
                      made_current_at = EXCLUDED.made_current_at",
    )
    .bind(job_version_uuid)
    .bind(dataset_uuid)
    .bind(job_uuid)
    .bind(job_symlink_target_uuid)
    .bind(Utc::now().naive_utc())
    .execute(pool)
    .await?;

    Ok(())
}

/// Upsert an OUTPUT dataset mapping for a job version.
///
/// Marks previous OUTPUT mappings for the same `(dataset_uuid, job_uuid)` as
/// not current (excluding the current job_version_uuid), then inserts the
/// new mapping. Matches Java's `JobVersionDao.upsertOutputDatasetFor()`.
pub async fn upsert_output_dataset(
    pool: &PgPool,
    job_version_uuid: Uuid,
    dataset_uuid: Uuid,
    job_uuid: Uuid,
    job_symlink_target_uuid: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    // Step 1: Mark previous OUTPUT mappings as not current.
    // Guard: exclude the current job_version_uuid to avoid clearing the row
    // we're about to insert. Also include symlink targets (matches Java).
    sqlx::query(
        "UPDATE job_versions_io_mapping \
         SET is_current_job_version = false \
         WHERE (job_uuid = $2 OR job_symlink_target_uuid = $2) \
         AND job_version_uuid != $1 \
         AND io_type = 'OUTPUT' \
         AND is_current_job_version = true",
    )
    .bind(job_version_uuid)
    .bind(job_uuid)
    .execute(pool)
    .await?;

    // Step 2: Insert new mapping.
    sqlx::query(
        "INSERT INTO job_versions_io_mapping (\
            job_version_uuid, dataset_uuid, io_type, job_uuid, \
            job_symlink_target_uuid, is_current_job_version, made_current_at\
        ) VALUES ($1, $2, 'OUTPUT', $3, $4, true, $5) \
        ON CONFLICT (job_version_uuid, dataset_uuid, io_type, job_uuid) \
        DO UPDATE SET is_current_job_version = TRUE, \
                      made_current_at = EXCLUDED.made_current_at",
    )
    .bind(job_version_uuid)
    .bind(dataset_uuid)
    .bind(job_uuid)
    .bind(job_symlink_target_uuid)
    .bind(Utc::now().naive_utc())
    .execute(pool)
    .await?;

    Ok(())
}

/// Batch upsert INPUT dataset mappings for a job version.
///
/// Marks previous INPUT mappings as not current for the given job (excluding
/// the current job_version_uuid), then inserts all new mappings at once using
/// `unnest()`. Replaces the per-dataset loop in `mark_run_as()`.
pub async fn upsert_input_datasets_batch(
    pool: &PgPool,
    job_version_uuid: Uuid,
    dataset_uuids: &[Uuid],
    job_uuid: Uuid,
    job_symlink_target_uuid: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    if dataset_uuids.is_empty() {
        return Ok(());
    }

    // Step 1: Mark previous INPUT mappings as not current.
    sqlx::query(
        "UPDATE job_versions_io_mapping \
         SET is_current_job_version = false \
         WHERE (job_uuid = $2 OR job_symlink_target_uuid = $2) \
         AND job_version_uuid != $1 \
         AND io_type = 'INPUT' \
         AND is_current_job_version = true",
    )
    .bind(job_version_uuid)
    .bind(job_uuid)
    .execute(pool)
    .await?;

    // Step 2: Batch insert all INPUT mappings using unnest().
    let now = Utc::now().naive_utc();
    let jv_uuids: Vec<Uuid> = vec![job_version_uuid; dataset_uuids.len()];
    let job_uuids: Vec<Uuid> = vec![job_uuid; dataset_uuids.len()];
    let symlink_uuids: Vec<Option<Uuid>> = vec![job_symlink_target_uuid; dataset_uuids.len()];
    let io_types: Vec<&str> = vec!["INPUT"; dataset_uuids.len()];
    let timestamps: Vec<chrono::NaiveDateTime> = vec![now; dataset_uuids.len()];

    sqlx::query(
        "INSERT INTO job_versions_io_mapping (\
            job_version_uuid, dataset_uuid, io_type, job_uuid, \
            job_symlink_target_uuid, is_current_job_version, made_current_at\
        ) SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::uuid[], \
            $5::uuid[], ARRAY_FILL(true, ARRAY[$6::int]), $7::timestamp[]) \
        ON CONFLICT (job_version_uuid, dataset_uuid, io_type, job_uuid) \
        DO UPDATE SET is_current_job_version = TRUE, \
                      made_current_at = EXCLUDED.made_current_at",
    )
    .bind(&jv_uuids)
    .bind(dataset_uuids)
    .bind(&io_types)
    .bind(&job_uuids)
    .bind(&symlink_uuids)
    .bind(dataset_uuids.len() as i32)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(())
}

/// Batch upsert OUTPUT dataset mappings for a job version.
///
/// Same approach as `upsert_input_datasets_batch` but for OUTPUT io_type.
pub async fn upsert_output_datasets_batch(
    pool: &PgPool,
    job_version_uuid: Uuid,
    dataset_uuids: &[Uuid],
    job_uuid: Uuid,
    job_symlink_target_uuid: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    if dataset_uuids.is_empty() {
        return Ok(());
    }

    // Step 1: Mark previous OUTPUT mappings as not current.
    sqlx::query(
        "UPDATE job_versions_io_mapping \
         SET is_current_job_version = false \
         WHERE (job_uuid = $2 OR job_symlink_target_uuid = $2) \
         AND job_version_uuid != $1 \
         AND io_type = 'OUTPUT' \
         AND is_current_job_version = true",
    )
    .bind(job_version_uuid)
    .bind(job_uuid)
    .execute(pool)
    .await?;

    // Step 2: Batch insert all OUTPUT mappings using unnest().
    let now = Utc::now().naive_utc();
    let jv_uuids: Vec<Uuid> = vec![job_version_uuid; dataset_uuids.len()];
    let job_uuids: Vec<Uuid> = vec![job_uuid; dataset_uuids.len()];
    let symlink_uuids: Vec<Option<Uuid>> = vec![job_symlink_target_uuid; dataset_uuids.len()];
    let io_types: Vec<&str> = vec!["OUTPUT"; dataset_uuids.len()];
    let timestamps: Vec<chrono::NaiveDateTime> = vec![now; dataset_uuids.len()];

    sqlx::query(
        "INSERT INTO job_versions_io_mapping (\
            job_version_uuid, dataset_uuid, io_type, job_uuid, \
            job_symlink_target_uuid, is_current_job_version, made_current_at\
        ) SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::uuid[], \
            $5::uuid[], ARRAY_FILL(true, ARRAY[$6::int]), $7::timestamp[]) \
        ON CONFLICT (job_version_uuid, dataset_uuid, io_type, job_uuid) \
        DO UPDATE SET is_current_job_version = TRUE, \
                      made_current_at = EXCLUDED.made_current_at",
    )
    .bind(&jv_uuids)
    .bind(dataset_uuids)
    .bind(&io_types)
    .bind(&job_uuids)
    .bind(&symlink_uuids)
    .bind(dataset_uuids.len() as i32)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(())
}

/// Find INPUT dataset mappings for a given job version.
pub async fn find_input_datasets(
    pool: &PgPool,
    job_version_uuid: Uuid,
) -> Result<Vec<JobVersionIoMappingRow>, sqlx::Error> {
    sqlx::query_as::<_, JobVersionIoMappingRow>(
        "SELECT * FROM job_versions_io_mapping \
         WHERE job_version_uuid = $1 AND io_type = 'INPUT'",
    )
    .bind(job_version_uuid)
    .fetch_all(pool)
    .await
}

/// Find OUTPUT dataset mappings for a given job version.
pub async fn find_output_datasets(
    pool: &PgPool,
    job_version_uuid: Uuid,
) -> Result<Vec<JobVersionIoMappingRow>, sqlx::Error> {
    sqlx::query_as::<_, JobVersionIoMappingRow>(
        "SELECT * FROM job_versions_io_mapping \
         WHERE job_version_uuid = $1 AND io_type = 'OUTPUT'",
    )
    .bind(job_version_uuid)
    .fetch_all(pool)
    .await
}

/// Update the `latest_run_uuid` for a job version.
pub async fn update_latest_run(
    exec: impl Executor<'_, Database = Postgres>,
    job_version_uuid: Uuid,
    run_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE job_versions SET latest_run_uuid = $1, updated_at = $2 \
         WHERE uuid = $3",
    )
    .bind(run_uuid)
    .bind(now)
    .bind(job_version_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Count job versions for a given job.
pub async fn count(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM job_versions \
         WHERE namespace_name = $1 AND job_name = $2",
    )
    .bind(namespace_name)
    .bind(job_name)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Find current output dataset UUIDs for a job.
///
/// Matches Java `JobVersionDao.findCurrentOutputDatasetUuids(jobUuid)`.
pub async fn find_current_output_dataset_uuids(
    exec: impl Executor<'_, Database = Postgres>,
    job_uuid: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT dataset_uuid FROM job_versions_io_mapping \
         WHERE job_uuid = $1 AND io_type = 'OUTPUT' \
         AND is_current_job_version = TRUE",
    )
    .bind(job_uuid)
    .fetch_all(exec)
    .await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

/// Find current input dataset UUIDs for a job.
///
/// Matches Java `JobVersionDao.findCurrentInputDatasetUuids(jobUuid)`.
pub async fn find_current_input_dataset_uuids(
    exec: impl Executor<'_, Database = Postgres>,
    job_uuid: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT dataset_uuid FROM job_versions_io_mapping \
         WHERE job_uuid = $1 AND io_type = 'INPUT' \
         AND is_current_job_version = TRUE",
    )
    .bind(job_uuid)
    .fetch_all(exec)
    .await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

/// Returns `true` if a job version with the given version UUID exists.
pub async fn version_exists(pool: &PgPool, version: Uuid) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM job_versions WHERE version = $1)")
            .bind(version)
            .fetch_one(pool)
            .await?;
    Ok(result)
}

/// Returns `true` if a job version with the given version UUID exists.
///
/// Variant that accepts any SQLx executor (e.g. a transaction connection),
/// unlike `version_exists` which requires `&PgPool`.
pub async fn version_exists_exec(
    exec: impl Executor<'_, Database = Postgres>,
    version: Uuid,
) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM job_versions WHERE version = $1)")
            .bind(version)
            .fetch_one(exec)
            .await?;
    Ok(result)
}

/// Link job facets to a job version by setting `job_version_uuid` on all
/// `job_facets` rows that match the given `run_uuid`.
pub async fn link_job_facets_to_job_version(
    exec: impl Executor<'_, Database = Postgres>,
    run_uuid: Uuid,
    job_version_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE job_facets SET job_version_uuid = $2 WHERE run_uuid = $1")
        .bind(run_uuid)
        .bind(job_version_uuid)
        .execute(exec)
        .await?;
    Ok(())
}

/// Base SQL for enriched job version queries (mirrors Java's
/// `BASE_SELECT_ON_JOB_VERSIONS`).
///
/// Joins job_versions with I/O datasets, latest run, run_args, job facets,
/// and input/output dataset versions. Callers append a WHERE clause
/// (single version) or LIMIT/OFFSET (list).
const BASE_SELECT_ON_JOB_VERSIONS: &str = "\
    WITH job_version_io AS ( \
        SELECT io.job_version_uuid, \
               JSON_AGG(json_build_object('namespace', ds.namespace_name, 'name', ds.name)) \
               FILTER (WHERE io.io_type = 'INPUT') AS input_datasets, \
               JSON_AGG(json_build_object('namespace', ds.namespace_name, 'name', ds.name)) \
               FILTER (WHERE io.io_type = 'OUTPUT') AS output_datasets \
        FROM job_versions_io_mapping io \
        INNER JOIN job_versions jv ON jv.uuid = io.job_version_uuid \
        INNER JOIN datasets_view ds ON ds.uuid = io.dataset_uuid \
        INNER JOIN jobs_view j ON j.uuid = jv.job_uuid \
        WHERE j.namespace_name = $1 AND j.name = $2 \
        GROUP BY io.job_version_uuid \
    ), relevant_job_versions AS ( \
        SELECT jv.uuid, jv.created_at, jv.updated_at, jv.job_uuid, jv.version, \
        jv.location, jv.latest_run_uuid, j.namespace_uuid, \
        j.namespace_name, j.name AS job_name \
        FROM job_versions jv \
        INNER JOIN jobs_view j ON j.uuid = jv.job_uuid \
        WHERE j.name = $2 AND j.namespace_name = $1 \
        ORDER BY jv.created_at DESC, jv.version DESC \
    ) \
    SELECT jv.*, \
           dsio.input_datasets, \
           dsio.output_datasets, \
           r.uuid               AS run_uuid, \
           r.created_at         AS run_created_at, \
           r.updated_at         AS run_updated_at, \
           r.nominal_start_time::timestamptz AS run_nominal_start_time, \
           r.nominal_end_time::timestamptz   AS run_nominal_end_time, \
           r.current_run_state  AS run_current_run_state, \
           r.started_at::timestamptz         AS run_started_at, \
           r.ended_at::timestamptz           AS run_ended_at, \
           r.namespace_name     AS run_namespace_name, \
           r.job_name           AS run_job_name, \
           jv.version           AS run_job_version, \
           r.location           AS run_location, \
           ra.args              AS run_args, \
           f.facets             AS run_facets, \
           ri.input_versions    AS run_input_versions, \
           ro.output_versions   AS run_output_versions \
    FROM relevant_job_versions AS jv \
    LEFT JOIN job_version_io dsio ON dsio.job_version_uuid = jv.uuid \
    LEFT OUTER JOIN runs r ON r.uuid = jv.latest_run_uuid \
    LEFT JOIN LATERAL ( \
        SELECT jf.run_uuid, JSON_AGG(jf.facet ORDER BY jf.lineage_event_time ASC) AS facets \
        FROM job_facets AS jf \
        WHERE jf.run_uuid = jv.latest_run_uuid AND jf.job_uuid = jv.job_uuid \
        GROUP BY jf.run_uuid \
    ) AS f ON r.uuid = f.run_uuid \
    LEFT OUTER JOIN run_args AS ra ON ra.uuid = r.run_args_uuid \
    LEFT JOIN LATERAL ( \
        SELECT im.run_uuid, \
               JSON_AGG(json_build_object('namespace', dv.namespace_name, \
                                          'name', dv.dataset_name, \
                                          'version', dv.version)) AS input_versions \
        FROM runs_input_mapping im \
        INNER JOIN dataset_versions dv ON im.dataset_version_uuid = dv.uuid \
        WHERE im.run_uuid = jv.latest_run_uuid \
        GROUP BY im.run_uuid \
    ) ri ON ri.run_uuid = r.uuid \
    LEFT JOIN LATERAL ( \
        SELECT dv.run_uuid, \
               JSON_AGG(json_build_object('namespace', dv.namespace_name, \
                                          'name', dv.dataset_name, \
                                          'version', dv.version)) AS output_versions \
        FROM dataset_versions dv \
        WHERE dv.run_uuid = jv.latest_run_uuid \
        GROUP BY dv.run_uuid \
    ) ro ON ro.run_uuid = r.uuid";

/// Find a single enriched job version by namespace, job name, and version UUID.
///
/// Mirrors Java's `JobVersionDao.findJobVersion()` with full I/O datasets,
/// latest run details, facets, and input/output versions.
pub async fn find_job_version_enriched(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
    job_version_uuid: Uuid,
) -> Result<Option<EnrichedJobVersionRow>, sqlx::Error> {
    let q = format!("{BASE_SELECT_ON_JOB_VERSIONS} WHERE jv.version = $3");
    sqlx::query_as::<_, EnrichedJobVersionRow>(&q)
        .bind(namespace_name)
        .bind(job_name)
        .bind(job_version_uuid)
        .fetch_optional(pool)
        .await
}

/// List all enriched job versions for a job, ordered by `created_at` descending.
///
/// Mirrors Java's `JobVersionDao.findAllJobVersions()` with full I/O datasets,
/// latest run details, facets, and input/output versions.
pub async fn find_all_job_versions_enriched(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<EnrichedJobVersionRow>, sqlx::Error> {
    let q = format!("{BASE_SELECT_ON_JOB_VERSIONS} LIMIT $3 OFFSET $4");
    sqlx::query_as::<_, EnrichedJobVersionRow>(&q)
        .bind(namespace_name)
        .bind(job_name)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}
