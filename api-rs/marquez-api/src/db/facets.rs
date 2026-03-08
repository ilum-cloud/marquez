// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `dataset_facets`, `job_facets`, and `run_facets` tables.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

/// Insert a dataset facet row (ON CONFLICT DO NOTHING).
///
/// `run_uuid` is `Option<Uuid>` — pass `None` for DatasetEvent/JobEvent paths
/// where no real run exists (stored as NULL in the DB). Matches Java's behaviour
/// where `run_uuid` is nullable in the `dataset_facets` table.
pub async fn insert_dataset_facet(
    exec: impl Executor<'_, Database = Postgres>,
    now: DateTime<Utc>,
    dataset_uuid: Uuid,
    dataset_version_uuid: Uuid,
    run_uuid: Option<Uuid>,
    event_time: DateTime<Utc>,
    event_type: Option<&str>,
    type_: &str,
    name: &str,
    facet: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO dataset_facets (\
            created_at, dataset_uuid, dataset_version_uuid, run_uuid, \
            lineage_event_time, lineage_event_type, type, name, facet\
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
        ON CONFLICT DO NOTHING",
    )
    .bind(now)
    .bind(dataset_uuid)
    .bind(dataset_version_uuid)
    .bind(run_uuid)
    .bind(event_time)
    .bind(event_type)
    .bind(type_)
    .bind(name)
    .bind(facet)
    .execute(exec)
    .await?;
    Ok(())
}

/// Insert a job facet row keyed by `job_version_uuid` (no `run_uuid`).
///
/// Used for JobEvent processing where no run exists. Matches Java's
/// `insertJobFacet(createdAt, jobUuid, jobVersionUuid, lineageEventTime, name, facet)`
/// overload which omits both `run_uuid` and `lineage_event_type`.
pub async fn insert_job_facet_for_version(
    exec: impl Executor<'_, Database = Postgres>,
    now: DateTime<Utc>,
    job_uuid: Uuid,
    job_version_uuid: Uuid,
    event_time: DateTime<Utc>,
    name: &str,
    facet: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO job_facets (\
            created_at, job_uuid, job_version_uuid, \
            lineage_event_time, name, facet\
        ) VALUES ($1, $2, $3, $4, $5, $6) \
        ON CONFLICT DO NOTHING",
    )
    .bind(now)
    .bind(job_uuid)
    .bind(job_version_uuid)
    .bind(event_time)
    .bind(name)
    .bind(facet)
    .execute(exec)
    .await?;
    Ok(())
}

/// Insert a job facet row (ON CONFLICT DO NOTHING).
pub async fn insert_job_facet(
    exec: impl Executor<'_, Database = Postgres>,
    now: DateTime<Utc>,
    job_uuid: Uuid,
    run_uuid: Uuid,
    event_time: DateTime<Utc>,
    event_type: Option<&str>,
    name: &str,
    facet: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO job_facets (\
            created_at, job_uuid, run_uuid, \
            lineage_event_time, lineage_event_type, name, facet\
        ) VALUES ($1, $2, $3, $4, $5, $6, $7) \
        ON CONFLICT DO NOTHING",
    )
    .bind(now)
    .bind(job_uuid)
    .bind(run_uuid)
    .bind(event_time)
    .bind(event_type)
    .bind(name)
    .bind(facet)
    .execute(exec)
    .await?;
    Ok(())
}

/// Insert a run facet row (ON CONFLICT DO NOTHING).
pub async fn insert_run_facet(
    exec: impl Executor<'_, Database = Postgres>,
    now: DateTime<Utc>,
    run_uuid: Uuid,
    event_time: DateTime<Utc>,
    event_type: &str,
    name: &str,
    facet: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO run_facets (\
            created_at, run_uuid, \
            lineage_event_time, lineage_event_type, name, facet\
        ) VALUES ($1, $2, $3, $4, $5, $6) \
        ON CONFLICT DO NOTHING",
    )
    .bind(now)
    .bind(run_uuid)
    .bind(event_time)
    .bind(event_type)
    .bind(name)
    .bind(facet)
    .execute(exec)
    .await?;
    Ok(())
}

/// Returns `true` if a run facet exists with the given run_uuid, event_time,
/// event_type, and name.
pub async fn run_facet_exists(
    pool: &PgPool,
    run_uuid: Uuid,
    event_time: DateTime<Utc>,
    event_type: &str,
    name: &str,
) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (\
         SELECT 1 FROM run_facets \
         WHERE run_uuid = $1 AND lineage_event_time = $2 \
         AND lineage_event_type = $3 AND name = $4\
         )",
    )
    .bind(run_uuid)
    .bind(event_time)
    .bind(event_type)
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(result)
}

/// Find aggregated job facets by run UUID.
///
/// Reads from `job_facets` and merges all unique keys into a single
/// flat JSON object — matching Java's `MapperUtils.toFacetsOrNull()`.
pub async fn find_job_facets_by_run(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    let row: (Option<serde_json::Value>,) = sqlx::query_as(
        "SELECT ( \
            SELECT jsonb_object_agg(kv.key, kv.value) \
            FROM ( \
                SELECT DISTINCT ON (kv2.key) kv2.key, kv2.value \
                FROM job_facets jf \
                CROSS JOIN LATERAL jsonb_each(jf.facet) AS kv2(key, value) \
                WHERE jf.run_uuid = $1 \
                ORDER BY kv2.key, jf.lineage_event_time DESC \
            ) kv \
        ) AS facets",
    )
    .bind(run_uuid)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Batch-fetch job facets for multiple run UUIDs.
///
/// Returns a map from run UUID → merged facets JSON object.
pub async fn find_job_facets_by_runs_batch(
    pool: &PgPool,
    run_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, serde_json::Value>, sqlx::Error> {
    let rows: Vec<(Uuid, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT jf.run_uuid, \
                jsonb_object_agg(kv.key, kv.value) AS facets \
         FROM ( \
             SELECT DISTINCT ON (jf2.run_uuid, kv2.key) \
                    jf2.run_uuid, kv2.key, kv2.value \
             FROM job_facets jf2 \
             CROSS JOIN LATERAL jsonb_each(jf2.facet) AS kv2(key, value) \
             WHERE jf2.run_uuid = ANY($1) \
             ORDER BY jf2.run_uuid, kv2.key, jf2.lineage_event_time DESC \
         ) kv \
         INNER JOIN job_facets jf ON jf.run_uuid = kv.run_uuid \
         GROUP BY jf.run_uuid",
    )
    .bind(run_uuids)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(uuid, facets)| facets.map(|f| (uuid, f)))
        .collect())
}

/// Find aggregated job facets by job version UUID.
///
/// Fallback for runless jobs — reads from `job_facets` where
/// `job_version_uuid` matches, merging all unique keys into a single
/// flat JSON object.
pub async fn find_job_facets_by_job_version(
    pool: &PgPool,
    job_version_uuid: Uuid,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    let row: (Option<serde_json::Value>,) = sqlx::query_as(
        "SELECT ( \
            SELECT jsonb_object_agg(kv.key, kv.value) \
            FROM ( \
                SELECT DISTINCT ON (kv2.key) kv2.key, kv2.value \
                FROM job_facets jf \
                CROSS JOIN LATERAL jsonb_each(jf.facet) AS kv2(key, value) \
                WHERE jf.job_version_uuid = $1 \
                ORDER BY kv2.key, jf.lineage_event_time DESC \
            ) kv \
        ) AS facets",
    )
    .bind(job_version_uuid)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Find aggregated run facets by run UUID.
///
/// Reads from `run_facets` (which extracts per-key facet rows from
/// `lineage_events`), expands each facet JSON with `jsonb_each`, then
/// merges all unique keys into a single flat JSON object — matching
/// Java's `MapperUtils.toFacetsOrNull()` behaviour.
pub async fn find_run_facets_by_run(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    let row: (Option<serde_json::Value>,) = sqlx::query_as(
        "SELECT ( \
            SELECT jsonb_object_agg(kv.key, kv.value) \
            FROM ( \
                SELECT DISTINCT ON (kv2.key) kv2.key, kv2.value \
                FROM run_facets rf \
                CROSS JOIN LATERAL jsonb_each(rf.facet) AS kv2(key, value) \
                WHERE rf.run_uuid = $1 \
                ORDER BY kv2.key, rf.lineage_event_time DESC \
            ) kv \
        ) AS facets",
    )
    .bind(run_uuid)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Find dataset facets for a run, grouped by dataset version's external UUID.
///
/// Reads from `dataset_facets` where `type` is 'input' or 'output',
/// groups by `dataset_version_uuid`, and returns a map from the dataset
/// version's external `version` UUID to merged facets — matching Java's
/// `RunMapper` behaviour for input/output dataset version facets.
pub async fn find_dataset_facets_by_run(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<std::collections::HashMap<Uuid, serde_json::Value>, sqlx::Error> {
    let rows: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT dv.version AS version_uuid, \
                jsonb_object_agg(kv.key, kv.value) AS facets \
         FROM ( \
             SELECT DISTINCT ON (df.dataset_version_uuid, kv2.key) \
                    df.dataset_version_uuid, kv2.key, kv2.value \
             FROM dataset_facets df \
             CROSS JOIN LATERAL jsonb_each(df.facet) AS kv2(key, value) \
             WHERE df.run_uuid = $1 \
               AND (LOWER(df.type) IN ('input', 'output')) \
             ORDER BY df.dataset_version_uuid, kv2.key, df.lineage_event_time DESC \
         ) kv \
         JOIN dataset_versions dv ON dv.uuid = kv.dataset_version_uuid \
         GROUP BY dv.version",
    )
    .bind(run_uuid)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().collect())
}

/// Compute and insert lineage statistics for a dataset, looking up
/// `current_version_uuid` from the `datasets` table.
///
/// Statistics include in/out edge counts, consuming/producing namespaces,
/// and producing/consuming job types.
pub async fn update_lineage_statistics(
    exec: impl Executor<'_, Database = Postgres>,
    dataset_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "WITH cv AS ( \
            SELECT current_version_uuid FROM datasets WHERE uuid = $1 \
        ), \
        io AS ( \
            SELECT io.io_type, io.job_uuid \
            FROM job_versions_io_mapping io \
            JOIN job_versions jv ON jv.uuid = io.job_version_uuid \
            CROSS JOIN cv \
            WHERE io.dataset_uuid = $1 AND io.is_current_job_version = TRUE \
            AND cv.current_version_uuid IS NOT NULL \
            AND ( \
                (io.io_type = 'INPUT' AND EXISTS ( \
                    SELECT 1 FROM runs_input_mapping rim \
                    WHERE rim.run_uuid = jv.latest_run_uuid \
                    AND rim.dataset_version_uuid = cv.current_version_uuid \
                )) \
                OR \
                (io.io_type = 'OUTPUT' AND EXISTS ( \
                    SELECT 1 FROM dataset_versions dv \
                    WHERE dv.run_uuid = jv.latest_run_uuid \
                    AND dv.uuid = cv.current_version_uuid \
                )) \
            ) \
        ), \
        job_info AS ( \
            SELECT io.io_type, io.job_uuid, j.namespace_name, j.type \
            FROM io \
            JOIN jobs_view j ON j.uuid = io.job_uuid \
        ), \
        stats AS ( \
            SELECT \
                count(DISTINCT CASE WHEN io_type = 'OUTPUT' THEN job_uuid END) AS \"inEdges\", \
                count(DISTINCT CASE WHEN io_type = 'INPUT' THEN job_uuid END) AS \"outEdges\", \
                array_agg(DISTINCT CASE WHEN io_type = 'INPUT' THEN namespace_name END) \
                    FILTER (WHERE namespace_name IS NOT NULL) AS \"consumingNamespaces\", \
                array_agg(DISTINCT CASE WHEN io_type = 'OUTPUT' THEN namespace_name END) \
                    FILTER (WHERE namespace_name IS NOT NULL) AS \"producingNamespaces\", \
                array_agg(DISTINCT CASE WHEN io_type = 'OUTPUT' THEN COALESCE(type, 'UNKNOWN') END) \
                    FILTER (WHERE type IS NOT NULL) AS \"producingJobTypes\", \
                array_agg(DISTINCT CASE WHEN io_type = 'INPUT' THEN COALESCE(type, 'UNKNOWN') END) \
                    FILTER (WHERE type IS NOT NULL) AS \"consumingJobTypes\" \
            FROM job_info \
        ) \
        INSERT INTO dataset_facets ( \
            created_at, dataset_uuid, dataset_version_uuid, run_uuid, \
            lineage_event_time, lineage_event_type, type, name, facet \
        ) \
        SELECT \
            NOW(), $1, d.current_version_uuid, NULL, \
            NOW(), 'LINEAGE_UPDATE', 'DATASET', 'lineageStatistics', \
            json_build_object( \
              'lineageStatistics', json_build_object( \
                'inEdges', s.\"inEdges\", \
                'outEdges', s.\"outEdges\", \
                'consumingNamespaces', COALESCE(s.\"consumingNamespaces\", ARRAY[]::varchar[]), \
                'producingNamespaces', COALESCE(s.\"producingNamespaces\", ARRAY[]::varchar[]), \
                'producingJobTypes', COALESCE(s.\"producingJobTypes\", ARRAY[]::varchar[]), \
                'consumingJobTypes', COALESCE(s.\"consumingJobTypes\", ARRAY[]::varchar[]), \
                '_producer', 'https://github.com/ilum-cloud/marquez', \
                '_schema', 'https://github.com/ilum-cloud/marquez/spec/facets/lineage-statistics.json' \
              ) \
            ) \
        FROM datasets d, stats s \
        WHERE d.uuid = $1 AND d.current_version_uuid IS NOT NULL",
    )
    .bind(dataset_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Compute and insert lineage statistics for a dataset with an explicit
/// `dataset_version_uuid`.
///
/// Similar to `update_lineage_statistics` but takes the version UUID directly
/// and omits job type arrays.
pub async fn update_lineage_statistics_versioned(
    exec: impl Executor<'_, Database = Postgres>,
    dataset_uuid: Uuid,
    dataset_version_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "WITH io AS ( \
            SELECT io.io_type, io.job_uuid \
            FROM job_versions_io_mapping io \
            JOIN job_versions jv ON jv.uuid = io.job_version_uuid \
            WHERE io.dataset_uuid = $1 AND io.is_current_job_version = TRUE \
            AND ( \
                (io.io_type = 'INPUT' AND EXISTS ( \
                    SELECT 1 FROM runs_input_mapping rim \
                    WHERE rim.run_uuid = jv.latest_run_uuid \
                    AND rim.dataset_version_uuid = $2 \
                )) \
                OR \
                (io.io_type = 'OUTPUT' AND EXISTS ( \
                    SELECT 1 FROM dataset_versions dv \
                    WHERE dv.run_uuid = jv.latest_run_uuid \
                    AND dv.uuid = $2 \
                )) \
            ) \
        ), \
        job_info AS ( \
            SELECT io.io_type, io.job_uuid, j.namespace_name, j.type \
            FROM io \
            JOIN jobs_view j ON j.uuid = io.job_uuid \
        ), \
        stats AS ( \
            SELECT \
                count(DISTINCT CASE WHEN io_type = 'OUTPUT' THEN job_uuid END) AS \"inEdges\", \
                count(DISTINCT CASE WHEN io_type = 'INPUT' THEN job_uuid END) AS \"outEdges\", \
                array_agg(DISTINCT CASE WHEN io_type = 'INPUT' THEN namespace_name END) \
                    FILTER (WHERE namespace_name IS NOT NULL) AS \"consumingNamespaces\", \
                array_agg(DISTINCT CASE WHEN io_type = 'OUTPUT' THEN namespace_name END) \
                    FILTER (WHERE namespace_name IS NOT NULL) AS \"producingNamespaces\" \
            FROM job_info \
        ) \
        INSERT INTO dataset_facets ( \
            created_at, dataset_uuid, dataset_version_uuid, run_uuid, \
            lineage_event_time, lineage_event_type, type, name, facet \
        ) \
        SELECT \
            NOW(), $1, $2, NULL, \
            NOW(), 'LINEAGE_UPDATE', 'DATASET', 'lineageStatistics', \
            json_build_object( \
              'lineageStatistics', json_build_object( \
                'inEdges', s.\"inEdges\", \
                'outEdges', s.\"outEdges\", \
                'consumingNamespaces', COALESCE(s.\"consumingNamespaces\", ARRAY[]::varchar[]), \
                'producingNamespaces', COALESCE(s.\"producingNamespaces\", ARRAY[]::varchar[]), \
                '_producer', 'https://github.com/ilum-cloud/marquez', \
                '_schema', 'https://github.com/ilum-cloud/marquez/spec/facets/lineage-statistics.json' \
              ) \
            ) \
        FROM stats s",
    )
    .bind(dataset_uuid)
    .bind(dataset_version_uuid)
    .execute(exec)
    .await?;
    Ok(())
}
