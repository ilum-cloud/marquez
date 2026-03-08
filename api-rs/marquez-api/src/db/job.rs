// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `jobs` table and `jobs_view`.
//!
//! CRITICAL: Job inserts/upserts go through `jobs_view` (not `jobs`).
//! The database has an INSTEAD OF trigger on `jobs_view` that resolves
//! symlinks and manages the FQN naming convention.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{JobRow, JobWithFacetsRow};

/// Column list used in SELECT queries against `jobs_view`.
///
/// `jobs_view` includes extra columns (`parent_job_name`, `parent_job_uuid_string`)
/// that are not in `JobRow`, and lacks `is_hidden` (it filters hidden rows).
/// We select only the columns that match `JobRow`.
const JOB_VIEW_COLUMNS: &str = "\
    uuid, type, created_at, updated_at, namespace_uuid, namespace_name, \
    name, simple_name, parent_job_uuid, description, \
    current_version_uuid, current_job_context_uuid, \
    current_location, current_inputs, symlink_target_uuid, \
    false AS is_hidden, current_run_uuid, aliases";

/// Qualified variant for queries that JOIN other tables (e.g. `runs`).
const JOB_VIEW_COLUMNS_Q: &str = "\
    jobs_view.uuid, jobs_view.type, jobs_view.created_at, jobs_view.updated_at, \
    jobs_view.namespace_uuid, jobs_view.namespace_name, \
    jobs_view.name, jobs_view.simple_name, jobs_view.parent_job_uuid, jobs_view.description, \
    jobs_view.current_version_uuid, jobs_view.current_job_context_uuid, \
    jobs_view.current_location, jobs_view.current_inputs, jobs_view.symlink_target_uuid, \
    false AS is_hidden, jobs_view.current_run_uuid, jobs_view.aliases";

/// Upsert a job via `jobs_view`.
///
/// The INSTEAD OF trigger on `jobs_view` handles symlink resolution,
/// FQN computation, and conflict on `(name, namespace_uuid)`.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    type_: &str,
    now: DateTime<Utc>,
    namespace_uuid: Uuid,
    namespace_name: &str,
    name: &str,
    description: Option<&str>,
    current_location: Option<&str>,
    current_inputs: Option<serde_json::Value>,
    simple_name: Option<&str>,
    parent_job_uuid: Option<Uuid>,
    current_run_uuid: Option<Uuid>,
) -> Result<JobRow, sqlx::Error> {
    let q = format!(
        "INSERT INTO jobs_view (\
            uuid, type, created_at, updated_at, \
            namespace_uuid, namespace_name, \
            name, description, current_location, \
            current_inputs, simple_name, parent_job_uuid, \
            current_run_uuid\
        ) VALUES (\
            $1, $2, $3, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12\
        ) RETURNING {JOB_VIEW_COLUMNS}"
    );
    sqlx::query_as::<_, JobRow>(&q)
        .bind(uuid)
        .bind(type_)
        .bind(now)
        .bind(namespace_uuid)
        .bind(namespace_name)
        .bind(name)
        .bind(description)
        .bind(current_location)
        .bind(current_inputs)
        .bind(simple_name)
        .bind(parent_job_uuid)
        .bind(current_run_uuid)
        .fetch_one(exec)
        .await
}

/// Returns `true` if a non-hidden job with the given namespace and name
/// exists in `jobs_view`.
pub async fn exists(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (\
         SELECT 1 FROM jobs_view \
         WHERE namespace_name = $1 AND name = $2\
         )",
    )
    .bind(namespace_name)
    .bind(job_name)
    .fetch_one(pool)
    .await?;
    Ok(result)
}

/// Find a job row by namespace and name from `jobs_view`.
///
/// Also matches job aliases (renamed jobs), matching Java's
/// `JobDao.findJobByName()` which uses `j.name=:jobName OR :jobName = ANY(j.aliases)`.
pub async fn find_by_name_as_row(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
) -> Result<Option<JobRow>, sqlx::Error> {
    let q = format!(
        "SELECT {JOB_VIEW_COLUMNS} FROM jobs_view \
         WHERE namespace_name = $1 AND (name = $2 OR $2 = ANY(aliases))"
    );
    sqlx::query_as::<_, JobRow>(&q)
        .bind(namespace_name)
        .bind(job_name)
        .fetch_optional(pool)
        .await
}

/// Simplified find_by_name. Returns `JobRow` from `jobs_view`.
/// Complex joins (tags, facets, latest_run) are deferred to the service layer.
pub async fn find_by_name(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
) -> Result<Option<JobRow>, sqlx::Error> {
    find_by_name_as_row(pool, namespace_name, job_name).await
}

/// All valid run states, used as default when no filter is specified.
const ALL_RUN_STATES: &[&str] = &["NEW", "RUNNING", "COMPLETED", "FAILED", "ABORTED"];

/// List jobs in a namespace ordered by `updated_at` descending, with limit/offset pagination.
/// Optionally filters by last run state.
pub async fn find_all(
    pool: &PgPool,
    namespace_name: &str,
    limit: i32,
    offset: i32,
    states: &[String],
) -> Result<Vec<JobRow>, sqlx::Error> {
    let effective_states: Vec<String> = if states.is_empty() {
        ALL_RUN_STATES.iter().map(|s| s.to_string()).collect()
    } else {
        states.to_vec()
    };
    let q = format!(
        "SELECT {JOB_VIEW_COLUMNS_Q} FROM jobs_view \
         LEFT JOIN runs r ON r.uuid = jobs_view.current_run_uuid \
         WHERE jobs_view.namespace_name = $1 \
         AND (r.current_run_state = ANY($4) OR r.uuid IS NULL) \
         ORDER BY jobs_view.updated_at DESC \
         LIMIT $2 OFFSET $3"
    );
    sqlx::query_as::<_, JobRow>(&q)
        .bind(namespace_name)
        .bind(limit)
        .bind(offset)
        .bind(&effective_states)
        .fetch_all(pool)
        .await
}

/// List all jobs (no namespace filter), ordered by `updated_at` descending.
/// Optionally filters by last run state.
pub async fn find_all_jobs(
    pool: &PgPool,
    limit: i32,
    offset: i32,
    states: &[String],
) -> Result<Vec<JobRow>, sqlx::Error> {
    let effective_states: Vec<String> = if states.is_empty() {
        ALL_RUN_STATES.iter().map(|s| s.to_string()).collect()
    } else {
        states.to_vec()
    };
    let q = format!(
        "SELECT {JOB_VIEW_COLUMNS_Q} FROM jobs_view \
         LEFT JOIN runs r ON r.uuid = jobs_view.current_run_uuid \
         WHERE (r.current_run_state = ANY($3) OR r.uuid IS NULL) \
         ORDER BY jobs_view.updated_at DESC \
         LIMIT $1 OFFSET $2"
    );
    sqlx::query_as::<_, JobRow>(&q)
        .bind(limit)
        .bind(offset)
        .bind(&effective_states)
        .fetch_all(pool)
        .await
}

/// Count all non-hidden jobs (no namespace filter).
pub async fn count_all(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs_view")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Count non-hidden jobs in a namespace.
pub async fn count(pool: &PgPool, namespace_name: &str) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs_view WHERE namespace_name = $1")
        .bind(namespace_name)
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Soft-delete a job by setting `is_hidden = true` in the `jobs` table
/// (not through the view).
pub async fn delete(
    exec: impl Executor<'_, Database = Postgres>,
    namespace_name: &str,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET is_hidden = true \
         WHERE namespace_name = $1 AND name = $2",
    )
    .bind(namespace_name)
    .bind(name)
    .execute(exec)
    .await?;
    Ok(())
}

/// Update `current_version_uuid` for a job.
pub async fn update_version(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    version_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET current_version_uuid = $1, updated_at = $2 \
         WHERE uuid = $3",
    )
    .bind(version_uuid)
    .bind(now)
    .bind(uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Update `current_run_uuid` for a job.
pub async fn update_current_run(
    exec: impl Executor<'_, Database = Postgres>,
    job_uuid: Uuid,
    run_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET current_run_uuid = $1, updated_at = $2 \
         WHERE uuid = $3",
    )
    .bind(run_uuid)
    .bind(now)
    .bind(job_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Remove a tag from a job.
pub async fn delete_job_tag(
    exec: impl Executor<'_, Database = Postgres>,
    job_uuid: Uuid,
    tag_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM jobs_tag_mapping \
         WHERE job_uuid = $1 AND tag_uuid = $2",
    )
    .bind(job_uuid)
    .bind(tag_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Insert a tag mapping for a job (ON CONFLICT DO NOTHING).
pub async fn update_job_tag(
    exec: impl Executor<'_, Database = Postgres>,
    job_uuid: Uuid,
    tag_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO jobs_tag_mapping (job_uuid, tag_uuid, tagged_at) \
         VALUES ($1, $2, $3) \
         ON CONFLICT DO NOTHING",
    )
    .bind(job_uuid)
    .bind(tag_uuid)
    .bind(now)
    .execute(exec)
    .await?;
    Ok(())
}

/// Find a job by namespace and name with facets and tags.
///
/// Uses CTEs to aggregate job facets and tags, then joins them with
/// `jobs_view`. Matches Java `JobDao.findJobByName()` (the enriched variant).
pub async fn find_job_by_name_with_facets(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
) -> Result<Option<JobWithFacetsRow>, sqlx::Error> {
    sqlx::query_as::<_, JobWithFacetsRow>(
        "WITH job_versions_facets AS ( \
            SELECT f.job_version_uuid, JSON_AGG(f.facet) as facets \
            FROM job_facets f \
            LEFT JOIN jobs_view j ON j.current_version_uuid = f.job_version_uuid \
            WHERE j.namespace_name = $1 AND (j.name = $2 OR $2 = ANY(j.aliases)) \
            GROUP BY job_version_uuid \
        ), \
        job_tags AS ( \
            SELECT j.uuid, ARRAY_AGG(t.name) as tags \
            FROM jobs_view j \
            INNER JOIN jobs_tag_mapping jtm ON jtm.job_uuid = j.uuid \
            INNER JOIN tags t ON jtm.tag_uuid = t.uuid \
            WHERE j.namespace_name = $1 AND (j.name = $2 OR $2 = ANY(j.aliases)) \
            GROUP BY j.uuid \
        ) \
        SELECT j.uuid, j.type, j.created_at, j.updated_at, j.namespace_uuid, j.namespace_name, \
               j.name, j.simple_name, j.parent_job_uuid, j.description, \
               j.current_version_uuid, j.current_job_context_uuid, j.current_location, \
               j.current_inputs, j.symlink_target_uuid, j.current_run_uuid, j.aliases, \
               f.facets, jt.tags \
        FROM jobs_view j \
        LEFT OUTER JOIN job_versions_facets f ON j.current_version_uuid = f.job_version_uuid \
        LEFT OUTER JOIN job_tags jt ON j.uuid = jt.uuid \
        WHERE j.namespace_name = $1 AND (j.name = $2 OR $2 = ANY(j.aliases))",
    )
    .bind(namespace_name)
    .bind(job_name)
    .fetch_optional(pool)
    .await
}

/// Find a job row by UUID from `jobs_view`, joining with `namespaces`.
pub async fn find_job_by_uuid_as_row(
    pool: &PgPool,
    job_uuid: Uuid,
) -> Result<Option<JobRow>, sqlx::Error> {
    let q = format!(
        "SELECT {JOB_VIEW_COLUMNS} FROM jobs_view j \
         INNER JOIN namespaces n ON j.namespace_uuid = n.uuid \
         WHERE j.uuid = $1"
    );
    sqlx::query_as::<_, JobRow>(&q)
        .bind(job_uuid)
        .fetch_optional(pool)
        .await
}

/// Create a tag if it doesn't exist, then insert a job-tag mapping.
///
/// Uses CTEs to upsert the tag and look up the job by namespace/name,
/// then inserts into `jobs_tag_mapping` with ON CONFLICT DO NOTHING.
pub async fn update_job_tags_now(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
    tag_name: &str,
    now: DateTime<Utc>,
    uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "WITH new_tag AS ( \
            INSERT INTO tags (uuid, created_at, updated_at, name, description) \
            SELECT $5, $4, $4, $3, NULL \
            WHERE NOT EXISTS (SELECT 1 FROM tags WHERE name = $3) \
            RETURNING uuid \
        ), \
        existing_tag AS ( \
            SELECT uuid FROM tags WHERE name = $3 \
        ), \
        job AS ( \
            SELECT uuid FROM jobs \
            WHERE simple_name = $2 AND namespace_name = $1 \
        ) \
        INSERT INTO jobs_tag_mapping (job_uuid, tag_uuid, tagged_at) \
        SELECT \
            (SELECT uuid FROM job), \
            COALESCE((SELECT uuid FROM new_tag), (SELECT uuid FROM existing_tag)), \
            $4 \
        ON CONFLICT DO NOTHING",
    )
    .bind(namespace_name)
    .bind(job_name)
    .bind(tag_name)
    .bind(now)
    .bind(uuid)
    .execute(pool)
    .await?;
    Ok(())
}

/// Count the number of runs for a specific job.
///
/// Matches Java `JobDao.countJobRuns(namespaceName, jobName)`.
pub async fn count_job_runs(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM runs \
         WHERE namespace_name = $1 AND job_name = $2",
    )
    .bind(namespace_name)
    .bind(job_name)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Update `current_inputs` JSON for a job.
///
/// Called after input datasets are processed during lineage event handling
/// to match Java's behavior of storing the current input dataset references.
pub async fn update_current_inputs(
    exec: impl Executor<'_, Database = Postgres>,
    job_uuid: Uuid,
    current_inputs: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE jobs SET current_inputs = $1 WHERE uuid = $2")
        .bind(current_inputs)
        .bind(job_uuid)
        .execute(exec)
        .await?;
    Ok(())
}

/// Batch-fetch tags for multiple jobs.
///
/// Returns a map from job UUID → tag names.
pub async fn find_tags_batch(
    pool: &PgPool,
    job_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<String>>, sqlx::Error> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT jtm.job_uuid, t.name \
         FROM tags t \
         INNER JOIN jobs_tag_mapping jtm ON jtm.tag_uuid = t.uuid \
         WHERE jtm.job_uuid = ANY($1) \
         ORDER BY t.name",
    )
    .bind(job_uuids)
    .fetch_all(pool)
    .await?;

    let mut map: std::collections::HashMap<Uuid, Vec<String>> = std::collections::HashMap::new();
    for (uuid, name) in rows {
        map.entry(uuid).or_default().push(name);
    }
    Ok(map)
}

/// Find a job's name by its UUID.
pub async fn find_name_by_uuid(pool: &PgPool, uuid: Uuid) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT name FROM jobs WHERE uuid = $1")
        .bind(uuid)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(n,)| n))
}

/// Batch-fetch job names by UUIDs.
///
/// Returns a map from job UUID → job name.
pub async fn find_names_by_uuids(
    pool: &PgPool,
    uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, String>, sqlx::Error> {
    if uuids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT uuid, name FROM jobs WHERE uuid = ANY($1)")
            .bind(uuids)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().collect())
}

/// Soft-delete all jobs in a namespace by setting `is_hidden = true`.
pub async fn delete_by_namespace_name(
    exec: impl Executor<'_, Database = Postgres>,
    namespace_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET is_hidden = true \
         FROM namespaces n \
         WHERE jobs.namespace_uuid = n.uuid AND n.name = $1",
    )
    .bind(namespace_name)
    .execute(exec)
    .await?;
    Ok(())
}
