// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for searching datasets and jobs by name pattern.
//!
//! Uses a single parameterized query with ILIKE for case-insensitive
//! substring matching. The search is a UNION of `datasets_view` and
//! `jobs_view` results, with optional namespace, date range, type filter,
//! and sort order.

use sqlx::PgPool;

use crate::models::db::{SearchResultRow, SimpleDatasetRow, SimpleJobRow};

/// Search datasets and jobs by name pattern.
///
/// - `query`: Substring to match (ILIKE).
/// - `filter`: Optional type filter -- `"DATASET"` or `"JOB"`. If `None`,
///   searches both.
/// - `sort`: Optional sort -- `"name"` for alphabetical, otherwise `updated_at` DESC.
/// - `limit`: Maximum number of results.
/// - `namespace`: Optional namespace filter.
/// - `before`: Optional date upper bound (YYYY-MM-DD).
/// - `after`: Optional date lower bound (YYYY-MM-DD).
pub async fn search(
    pool: &PgPool,
    query: &str,
    filter: Option<&str>,
    sort: Option<&str>,
    limit: i32,
    namespace: Option<&str>,
    before: Option<&str>,
    after: Option<&str>,
) -> Result<Vec<SearchResultRow>, sqlx::Error> {
    // Normalize sort to lowercase for comparison in SQL
    let sort_lower = sort.map(|s| s.to_lowercase());
    let sort_ref = sort_lower.as_deref();

    sqlx::query_as::<_, SearchResultRow>(
        "SELECT type, name, updated_at, namespace_name \
         FROM ( \
           SELECT 'DATASET' AS type, d.name, d.updated_at, d.namespace_name \
             FROM datasets_view d \
            WHERE (d.namespace_name = $3 OR $3 IS NULL) \
              AND (d.updated_at < $4::date OR $4 IS NULL) \
              AND (d.updated_at > $5::date OR $5 IS NULL) \
              AND d.name ILIKE '%' || $1 || '%' \
           UNION \
           SELECT DISTINCT ON (j.namespace_name, j.name) \
             'JOB' AS type, j.name, j.updated_at, j.namespace_name \
             FROM ( \
               SELECT namespace_name, name, \
                      UNNEST(COALESCE(aliases, ARRAY[NULL]::varchar[])) AS alias, \
                      updated_at \
                 FROM jobs_view \
                WHERE symlink_target_uuid IS NULL \
                ORDER BY updated_at DESC \
             ) j \
            WHERE (j.namespace_name = $3 OR $3 IS NULL) \
              AND (j.updated_at < $4::date OR $4 IS NULL) \
              AND (j.updated_at > $5::date OR $5 IS NULL) \
              AND (j.name ILIKE '%' || $1 || '%' OR j.alias ILIKE '%' || $1 || '%') \
         ) results \
         WHERE type = $6 OR $6 IS NULL \
         ORDER BY \
           CASE WHEN $7 = 'name' THEN name END ASC, \
           CASE WHEN $7 != 'name' OR $7 IS NULL THEN updated_at END DESC \
         LIMIT $2",
    )
    .bind(query)
    .bind(limit)
    .bind(namespace)
    .bind(before)
    .bind(after)
    .bind(filter)
    .bind(sort_ref)
    .fetch_all(pool)
    .await
}

/// Simple search for datasets and jobs by name pattern (SimpleSearchDao).
///
/// Similar to `search()` but:
/// - Datasets are filtered with `is_deleted = false`
/// - Jobs query `jobs_view` directly (no alias subquery), filtered by
///   `symlink_target_uuid IS NULL`
/// - Uses `UNION ALL` instead of `UNION`
/// - Sort supports `UPDATED_AT` (desc) and `NAME` (asc)
/// - Filter comparison uses `UPPER(type) = UPPER($4)`
pub async fn simple_search(
    pool: &PgPool,
    query: &str,
    filter: Option<&str>,
    sort: Option<&str>,
    limit: i32,
    namespace: Option<&str>,
) -> Result<Vec<SearchResultRow>, sqlx::Error> {
    sqlx::query_as::<_, SearchResultRow>(
        "SELECT type, name, namespace_name, updated_at \
         FROM ( \
           SELECT 'DATASET' AS type, d.name, d.namespace_name, d.updated_at \
             FROM datasets_view AS d \
            WHERE (d.namespace_name = $3 OR $3 IS NULL) \
              AND (d.name ILIKE '%' || $1 || '%') \
              AND d.is_deleted = false \
           UNION ALL \
           SELECT 'JOB' AS type, j.name, j.namespace_name, j.updated_at \
             FROM jobs_view AS j \
            WHERE (j.namespace_name = $3 OR $3 IS NULL) \
              AND (j.name ILIKE '%' || $1 || '%') \
              AND j.symlink_target_uuid IS NULL \
         ) AS results \
         WHERE ($4 IS NULL OR UPPER(type) = UPPER($4)) \
         ORDER BY \
           CASE WHEN UPPER($5) = 'UPDATED_AT' THEN updated_at END DESC, \
           CASE WHEN UPPER($5) = 'NAME' THEN name END ASC \
         LIMIT $2",
    )
    .bind(query)
    .bind(limit)
    .bind(namespace)
    .bind(filter)
    .bind(sort)
    .fetch_all(pool)
    .await
}

/// Count datasets matching a name pattern in an optional namespace.
///
/// Only counts non-deleted datasets.
pub async fn count_datasets(
    pool: &PgPool,
    query: &str,
    namespace: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) \
         FROM datasets_view d \
         WHERE (d.namespace_name = $2 OR $2 IS NULL) \
           AND (d.name ILIKE '%' || COALESCE($1, '') || '%') \
           AND d.is_deleted = false",
    )
    .bind(query)
    .bind(namespace)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Count jobs matching a name pattern in an optional namespace.
///
/// Only counts non-symlinked jobs. Matches Java `FullSearchDao.countJobs()`.
pub async fn count_jobs(
    pool: &PgPool,
    query: &str,
    namespace: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) \
         FROM jobs_view j \
         WHERE (j.namespace_name = $2 OR $2 IS NULL) \
           AND (j.name ILIKE '%' || COALESCE($1, '') || '%') \
           AND j.symlink_target_uuid IS NULL",
    )
    .bind(query)
    .bind(namespace)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Search datasets with facets, tags, fields, and lifecycle state.
///
/// Returns paginated dataset results with optional facet inclusion.
/// Sort supports `"UPDATED_AT"` (desc) and `"NAME"` (asc).
pub async fn search_datasets(
    pool: &PgPool,
    query: &str,
    sort: &str,
    limit: i32,
    offset: i32,
    namespace: Option<&str>,
    include_facets: bool,
    facet_names: Option<&[String]>,
) -> Result<Vec<SimpleDatasetRow>, sqlx::Error> {
    sqlx::query_as::<_, SimpleDatasetRow>(
        "WITH facets_t AS ( \
             SELECT df.dataset_version_uuid, df.facet, df.\"name\", df.created_at, \
                    rank() OVER (PARTITION BY df.dataset_version_uuid, \"name\" \
                                 ORDER BY created_at DESC) AS r \
             FROM dataset_facets AS df \
             WHERE (LOWER(df.type) IN ('dataset', 'unknown', 'input')) \
               AND ($6 = true) \
               AND (CARDINALITY(COALESCE($7, ARRAY[]::text[])) = 0 OR df.name = ANY(COALESCE($7, ARRAY[]::text[]))) \
               AND df.dataset_uuid IN ( \
                   SELECT uuid FROM datasets_view \
                   WHERE (namespace_name = $5 OR $5 IS NULL) \
                     AND (name ILIKE '%' || COALESCE($1, '') || '%') \
                     AND is_deleted = false \
                   ORDER BY \
                     CASE WHEN $2 = 'UPDATED_AT' THEN updated_at END DESC, \
                     CASE WHEN $2 = 'NAME' THEN name END \
                   LIMIT $3 OFFSET $4 \
               ) \
         ) \
         SELECT d.uuid, d.type, d.created_at, d.updated_at, d.namespace_name, d.name, \
                d.physical_name, d.source_name, d.description, d.current_version_uuid, \
                d.last_modified_at::timestamptz AS last_modified_at, d.is_deleted, \
                dv.lifecycle_state, dv.fields, \
                COALESCE(t.tags, ARRAY[]::text[]) AS tags, \
                CASE WHEN $6 = false THEN '[]'::jsonb \
                     ELSE COALESCE(f.facets, '[]'::jsonb) \
                END AS facets, \
                (d.current_version_uuid IS NOT NULL) AS is_current_version \
         FROM datasets_view d \
         LEFT JOIN dataset_versions dv ON d.current_version_uuid = dv.uuid \
         LEFT JOIN ( \
             SELECT ARRAY_AGG(t.name) AS tags, m.dataset_uuid \
             FROM tags AS t \
             INNER JOIN datasets_tag_mapping AS m ON m.tag_uuid = t.uuid \
             GROUP BY m.dataset_uuid \
         ) t ON t.dataset_uuid = d.uuid \
         LEFT JOIN ( \
             SELECT df.dataset_version_uuid, \
                    JSONB_AGG(df.facet ORDER BY df.created_at DESC) AS facets \
             FROM facets_t AS df WHERE r = 1 \
             GROUP BY df.dataset_version_uuid \
         ) f ON f.dataset_version_uuid = d.current_version_uuid \
         WHERE (d.namespace_name = $5 OR $5 IS NULL) \
           AND (d.name ILIKE '%' || COALESCE($1, '') || '%') \
           AND d.is_deleted = false \
         ORDER BY \
           CASE WHEN $2 = 'UPDATED_AT' THEN d.updated_at END DESC, \
           CASE WHEN $2 = 'NAME' THEN d.name END \
         LIMIT $3 OFFSET $4",
    )
    .bind(query)       // $1
    .bind(sort)        // $2
    .bind(limit)       // $3
    .bind(offset)      // $4
    .bind(namespace)   // $5
    .bind(include_facets) // $6
    .bind(facet_names) // $7
    .fetch_all(pool)
    .await
}

/// Search jobs with facets, tags, and labels.
///
/// Returns paginated job results with optional facet inclusion.
/// Sort supports `"UPDATED_AT"` (desc) and `"NAME"` (asc).
pub async fn search_jobs(
    pool: &PgPool,
    query: &str,
    sort: &str,
    limit: i32,
    offset: i32,
    namespace: Option<&str>,
    include_facets: bool,
    facet_names: Option<&[String]>,
) -> Result<Vec<SimpleJobRow>, sqlx::Error> {
    sqlx::query_as::<_, SimpleJobRow>(
        "WITH jobs_view_page AS ( \
             SELECT * FROM jobs_view AS j \
             WHERE (j.namespace_name = $5 OR $5 IS NULL) \
               AND (j.name ILIKE '%' || COALESCE($1, '') || '%') \
               AND j.symlink_target_uuid IS NULL \
         ), \
         job_versions_temp AS ( \
             SELECT * FROM job_versions AS jv \
             WHERE ($5 IS NULL OR jv.namespace_name = $5) \
               AND ($6 = true) \
         ), \
         facets_temp AS ( \
             SELECT run_uuid, \
                    JSON_AGG(e.facet ORDER BY e.lineage_event_time ASC) AS facets \
             FROM ( \
                 SELECT jf.run_uuid, jf.facet, jf.lineage_event_time \
                 FROM job_facets AS jf \
                 INNER JOIN job_versions_temp jv2 ON jv2.latest_run_uuid = jf.run_uuid \
                 INNER JOIN jobs_view_page j2 ON j2.current_version_uuid = jv2.uuid \
                 WHERE ($6 = true) \
                   AND (CARDINALITY(COALESCE($7, ARRAY[]::text[])) = 0 OR jf.name = ANY(COALESCE($7, ARRAY[]::text[]))) \
                 ORDER BY jf.lineage_event_time ASC \
             ) e \
             GROUP BY e.run_uuid \
         ), \
         job_tags AS ( \
             SELECT j.uuid, ARRAY_AGG(t.name) as tags \
             FROM jobs j \
             INNER JOIN jobs_tag_mapping jtm ON jtm.job_uuid = j.uuid \
             INNER JOIN tags t ON jtm.tag_uuid = t.uuid \
             WHERE ($5 IS NULL OR j.namespace_name = $5) \
             GROUP BY j.uuid \
         ) \
         SELECT j.uuid, j.type, j.created_at, j.updated_at, j.namespace_name, \
                j.name, j.simple_name, j.parent_job_name, j.parent_job_uuid, \
                j.current_location AS location, j.description, j.current_version_uuid, \
                COALESCE(jt.tags, ARRAY[]::text[]) AS tags, \
                ARRAY[]::text[] AS labels, \
                CASE WHEN $6 = false THEN '[]'::jsonb \
                     ELSE COALESCE(f.facets::jsonb, '[]'::jsonb) \
                END AS facets \
         FROM jobs_view_page j \
         LEFT OUTER JOIN job_versions_temp AS jv ON jv.uuid = j.current_version_uuid \
         LEFT OUTER JOIN facets_temp AS f ON f.run_uuid = jv.latest_run_uuid \
         LEFT OUTER JOIN job_tags jt ON j.uuid = jt.uuid \
         ORDER BY \
           CASE WHEN $2 = 'UPDATED_AT' THEN j.updated_at END DESC, \
           CASE WHEN $2 = 'NAME' THEN j.name END \
         LIMIT $3 OFFSET $4",
    )
    .bind(query)       // $1
    .bind(sort)        // $2
    .bind(limit)       // $3
    .bind(offset)      // $4
    .bind(namespace)   // $5
    .bind(include_facets) // $6
    .bind(facet_names) // $7
    .fetch_all(pool)
    .await
}
