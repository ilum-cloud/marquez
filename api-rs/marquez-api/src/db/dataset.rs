// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `datasets` table and `datasets_view`.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{DatasetRow, DatasetWithFacetsRow};

/// Upsert a dataset row.
///
/// INSERT INTO datasets ON CONFLICT(uuid) DO UPDATE SET type, updated_at,
/// physical_name, description, is_deleted, is_hidden. Returns the row via
/// RETURNING.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    type_: &str,
    now: DateTime<Utc>,
    namespace_uuid: Uuid,
    namespace_name: &str,
    source_uuid: Uuid,
    source_name: &str,
    name: &str,
    physical_name: &str,
    description: Option<&str>,
    is_deleted: bool,
) -> Result<DatasetRow, sqlx::Error> {
    sqlx::query_as::<_, DatasetRow>(
        "INSERT INTO datasets \
         (uuid, type, created_at, updated_at, namespace_uuid, namespace_name, \
          source_uuid, source_name, name, physical_name, description, is_deleted, is_hidden) \
         VALUES ($1, $2, $3, $3, $4, $5, $6, $7, $8, $9, $10, $11, false) \
         ON CONFLICT (namespace_uuid, name) DO UPDATE SET \
         type = EXCLUDED.type, \
         updated_at = EXCLUDED.updated_at, \
         physical_name = EXCLUDED.physical_name, \
         description = EXCLUDED.description, \
         is_deleted = EXCLUDED.is_deleted, \
         is_hidden = EXCLUDED.is_hidden \
         RETURNING uuid, type, created_at, updated_at, namespace_uuid, \
         namespace_name, source_uuid, source_name, name, physical_name, \
         last_modified_at::timestamptz AS last_modified_at, \
         description, current_version_uuid, is_deleted, is_hidden",
    )
    .bind(uuid)
    .bind(type_)
    .bind(now)
    .bind(namespace_uuid)
    .bind(namespace_name)
    .bind(source_uuid)
    .bind(source_name)
    .bind(name)
    .bind(physical_name)
    .bind(description)
    .bind(is_deleted)
    .fetch_one(exec)
    .await
}

/// Returns `true` if a non-hidden dataset with the given namespace and name
/// exists in `datasets_view`.
pub async fn exists(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (\
         SELECT 1 FROM datasets_view \
         WHERE namespace_name = $1 AND name = $2\
         )",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .fetch_one(pool)
    .await?;
    Ok(result)
}

/// Find a dataset by namespace and name from `datasets_view`.
///
/// Returns a `DatasetRow` with `is_hidden` always `None` because
/// `datasets_view` does not include that column (it filters hidden rows).
pub async fn find_dataset_as_row(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
) -> Result<Option<DatasetRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetRow>(
        "SELECT uuid, type, created_at, updated_at, namespace_uuid, \
         namespace_name, source_uuid, source_name, name, physical_name, \
         last_modified_at::timestamptz AS last_modified_at, \
         description, current_version_uuid, is_deleted, \
         NULL::boolean AS is_hidden \
         FROM datasets_view \
         WHERE namespace_name = $1 AND name = $2",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .fetch_optional(pool)
    .await
}

/// Find a dataset by namespace and name from `datasets_view`.
///
/// Simplified version -- the full query with tags/facets joins will be added
/// when the service layer needs it.
pub async fn find_by_name(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
) -> Result<Option<DatasetRow>, sqlx::Error> {
    find_dataset_as_row(pool, namespace_name, dataset_name).await
}

/// List datasets in a namespace ordered by name, with limit/offset pagination.
pub async fn find_all(
    pool: &PgPool,
    namespace_name: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<DatasetRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetRow>(
        "SELECT uuid, type, created_at, updated_at, namespace_uuid, \
         namespace_name, source_uuid, source_name, name, physical_name, \
         last_modified_at::timestamptz AS last_modified_at, \
         description, current_version_uuid, is_deleted, \
         NULL::boolean AS is_hidden \
         FROM datasets_view \
         WHERE namespace_name = $1 \
         ORDER BY name \
         LIMIT $2 OFFSET $3",
    )
    .bind(namespace_name)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Count datasets in a namespace.
pub async fn count(pool: &PgPool, namespace_name: &str) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM datasets_view WHERE namespace_name = $1")
            .bind(namespace_name)
            .fetch_one(pool)
            .await?;
    Ok(n)
}

/// Update the current version UUID for a dataset.
pub async fn update_version(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    version_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE datasets SET current_version_uuid = $1, updated_at = $2 \
         WHERE uuid = $3",
    )
    .bind(version_uuid)
    .bind(now)
    .bind(uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Batch-update `updated_at` and `last_modified_at` for a set of dataset UUIDs.
pub async fn update_last_modified_at(
    exec: impl Executor<'_, Database = Postgres>,
    uuids: &[Uuid],
    last_modified_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE datasets SET updated_at = $1, last_modified_at = $1 \
         WHERE uuid = ANY($2)",
    )
    .bind(last_modified_at)
    .bind(uuids)
    .execute(exec)
    .await?;
    Ok(())
}

/// Find output dataset UUIDs for a run.
///
/// Queries `dataset_versions` where `run_uuid` matches (output datasets have
/// their producing run stored on the version row).
pub async fn find_output_dataset_uuids_by_run(
    pool: impl Executor<'_, Database = Postgres>,
    run_uuid: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT DISTINCT dataset_uuid FROM dataset_versions WHERE run_uuid = $1")
            .bind(run_uuid)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

/// Insert a tag mapping for a dataset (ON CONFLICT DO NOTHING).
pub async fn update_tag_mapping(
    exec: impl Executor<'_, Database = Postgres>,
    dataset_uuid: Uuid,
    tag_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO datasets_tag_mapping (dataset_uuid, tag_uuid, tagged_at) \
         VALUES ($1, $2, $3) \
         ON CONFLICT DO NOTHING",
    )
    .bind(dataset_uuid)
    .bind(tag_uuid)
    .bind(now)
    .execute(exec)
    .await?;
    Ok(())
}

/// Delete a tag mapping from a dataset by looking up via namespace, dataset
/// name, and tag name.
pub async fn delete_dataset_tag(
    exec: impl Executor<'_, Database = Postgres>,
    namespace_name: &str,
    dataset_name: &str,
    tag_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM datasets_tag_mapping dtm \
         WHERE EXISTS ( \
             SELECT 1 FROM datasets d \
             JOIN tags t ON d.uuid = dtm.dataset_uuid AND t.uuid = dtm.tag_uuid \
             JOIN namespaces n ON d.namespace_uuid = n.uuid \
             WHERE d.name = $1 AND t.name = $2 AND n.name = $3 \
         )",
    )
    .bind(dataset_name)
    .bind(tag_name)
    .bind(namespace_name)
    .execute(exec)
    .await?;
    Ok(())
}

/// Soft-delete a dataset by setting `is_hidden = true`.
///
/// Looks up the dataset by joining `datasets` with `namespaces` on
/// `namespace_uuid`. Returns the updated row, or `None` if no match.
pub async fn delete(
    exec: impl Executor<'_, Database = Postgres>,
    namespace_name: &str,
    name: &str,
) -> Result<Option<DatasetRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetRow>(
        "UPDATE datasets d SET is_hidden = true \
         FROM namespaces n \
         WHERE n.uuid = d.namespace_uuid \
         AND n.name = $1 AND d.name = $2 \
         RETURNING d.uuid, d.type, d.created_at, d.updated_at, d.namespace_uuid, \
         d.namespace_name, d.source_uuid, d.source_name, d.name, d.physical_name, \
         d.last_modified_at::timestamptz AS last_modified_at, \
         d.description, d.current_version_uuid, d.is_deleted, d.is_hidden",
    )
    .bind(namespace_name)
    .bind(name)
    .fetch_optional(exec)
    .await
}

/// Count all non-hidden datasets (no namespace filter).
///
/// Matches Java `DatasetDao.count()` — `SELECT count(*) FROM datasets_view`.
pub async fn count_all(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM datasets_view")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Soft-delete all datasets in a namespace by setting `is_hidden = true`.
///
/// Joins `datasets` with `namespaces` on `namespace_uuid` to find all
/// datasets belonging to the given namespace name.
pub async fn delete_by_namespace_name(
    exec: impl Executor<'_, Database = Postgres>,
    namespace_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE datasets d \
         SET is_hidden = true \
         FROM namespaces n \
         WHERE n.uuid = d.namespace_uuid \
         AND n.name = $1",
    )
    .bind(namespace_name)
    .execute(exec)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Batch enrichment functions for list() optimization
// ---------------------------------------------------------------------------

/// Batch-fetch lifecycle states for multiple dataset version UUIDs.
pub async fn find_lifecycle_states_batch(
    pool: &PgPool,
    version_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Option<String>>, sqlx::Error> {
    let rows: Vec<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT uuid, lifecycle_state FROM dataset_versions WHERE uuid = ANY($1)")
            .bind(version_uuids)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().collect())
}

/// Batch-fetch source names for multiple source UUIDs.
pub async fn find_source_names_batch(
    pool: &PgPool,
    source_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, String>, sqlx::Error> {
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT uuid, name FROM sources WHERE uuid = ANY($1)")
            .bind(source_uuids)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().collect())
}

/// Batch-fetch facets for multiple dataset version UUIDs.
///
/// Returns a map from dataset_version_uuid → merged facets JSON object.
pub async fn find_facets_batch(
    pool: &PgPool,
    version_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, serde_json::Value>, sqlx::Error> {
    let rows: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT df.dataset_version_uuid, \
                JSONB_AGG(df.facet ORDER BY df.lineage_event_time ASC) AS facets \
         FROM dataset_facets df \
         WHERE df.dataset_version_uuid = ANY($1) \
           AND LOWER(df.type) IN ('dataset', 'unknown', 'input') \
           AND df.facet IS NOT NULL \
         GROUP BY df.dataset_version_uuid",
    )
    .bind(version_uuids)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Batch-fetch tags for multiple dataset UUIDs.
///
/// Returns a map from dataset_uuid → tag names.
pub async fn find_tags_batch(
    pool: &PgPool,
    dataset_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<String>>, sqlx::Error> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT dtm.dataset_uuid, t.name \
         FROM tags t \
         INNER JOIN datasets_tag_mapping dtm ON dtm.tag_uuid = t.uuid \
         WHERE dtm.dataset_uuid = ANY($1) \
         ORDER BY t.name",
    )
    .bind(dataset_uuids)
    .fetch_all(pool)
    .await?;

    let mut map: std::collections::HashMap<Uuid, Vec<String>> = std::collections::HashMap::new();
    for (uuid, name) in rows {
        map.entry(uuid).or_default().push(name);
    }
    Ok(map)
}

/// Find a dataset by namespace and name with facets, tags, fields, and lifecycle state.
///
/// Joins `datasets_view` with `dataset_versions`, `stream_versions`, aggregated
/// tags, and aggregated dataset facets. Matches Java `DatasetDao.findDatasetByName()`.
pub async fn find_dataset_by_name_with_facets(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
) -> Result<Option<DatasetWithFacetsRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetWithFacetsRow>(
        "SELECT d.uuid, d.type, d.created_at, d.updated_at, d.namespace_uuid, d.namespace_name, \
               d.source_uuid, d.source_name, d.name, d.physical_name, \
               d.last_modified_at::timestamptz AS last_modified_at, \
               d.description, d.current_version_uuid, d.is_deleted, \
               dv.fields, dv.lifecycle_state, sv.schema_location, t.tags, f.facets \
        FROM datasets_view d \
        LEFT JOIN dataset_versions dv ON d.current_version_uuid = dv.uuid \
        LEFT JOIN stream_versions AS sv ON sv.dataset_version_uuid = dv.uuid \
        LEFT JOIN ( \
            SELECT ARRAY_AGG(t.name) AS tags, m.dataset_uuid \
            FROM tags AS t \
            INNER JOIN datasets_tag_mapping AS m ON m.tag_uuid = t.uuid \
            GROUP BY m.dataset_uuid \
        ) t ON t.dataset_uuid = d.uuid \
        LEFT JOIN ( \
            SELECT df.dataset_version_uuid, \
                   JSONB_AGG(df.facet ORDER BY df.lineage_event_time ASC) AS facets \
            FROM dataset_facets AS df \
            WHERE df.facet IS NOT NULL AND \
             (LOWER(df.type) IN ('dataset', 'unknown', 'input')) AND \
              df.dataset_uuid = (SELECT uuid FROM datasets WHERE name = $2 AND namespace_name = $1) \
            GROUP BY df.dataset_version_uuid \
        ) f ON f.dataset_version_uuid = d.current_version_uuid \
        WHERE CAST(($1, $2) AS DATASET_NAME) = ANY(d.dataset_symlinks)",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .fetch_optional(pool)
    .await
}
