// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `dataset_versions` table, plus symlink and schema
//! version helpers.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{
    DatasetSchemaVersionRow, DatasetSymlinkRow, DatasetVersionRow, EnrichedDatasetVersionRow,
};

// ---------------------------------------------------------------------------
// dataset_versions core
// ---------------------------------------------------------------------------

/// Upsert a dataset version row.
///
/// INSERT ON CONFLICT(version) DO UPDATE SET run_uuid. Returns the row.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    dataset_uuid: Uuid,
    version: Uuid,
    schema_version_uuid: Option<Uuid>,
    run_uuid: Option<Uuid>,
    fields: Option<serde_json::Value>,
    namespace_name: &str,
    dataset_name: &str,
    lifecycle_state: Option<&str>,
) -> Result<DatasetVersionRow, sqlx::Error> {
    sqlx::query_as::<_, DatasetVersionRow>(
        "INSERT INTO dataset_versions \
         (uuid, created_at, dataset_uuid, version, dataset_schema_version_uuid, \
          run_uuid, fields, namespace_name, dataset_name, lifecycle_state) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (version) DO UPDATE SET \
         run_uuid = EXCLUDED.run_uuid \
         RETURNING *",
    )
    .bind(uuid)
    .bind(now)
    .bind(dataset_uuid)
    .bind(version)
    .bind(schema_version_uuid)
    .bind(run_uuid)
    .bind(fields)
    .bind(namespace_name)
    .bind(dataset_name)
    .bind(lifecycle_state)
    .fetch_one(exec)
    .await
}

/// Find a dataset version by its UUID.
pub async fn find_by_uuid(
    pool: &PgPool,
    uuid: Uuid,
) -> Result<Option<DatasetVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetVersionRow>("SELECT * FROM dataset_versions WHERE uuid = $1")
        .bind(uuid)
        .fetch_optional(pool)
        .await
}

/// Find a dataset version by its UUID (executor variant for use inside transactions).
pub async fn find_by_uuid_exec(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
) -> Result<Option<DatasetVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetVersionRow>("SELECT * FROM dataset_versions WHERE uuid = $1")
        .bind(uuid)
        .fetch_optional(exec)
        .await
}

/// List all enriched versions for a dataset, ordered by created_at descending.
///
/// Uses CTEs to gather version data, tags, and facets. Matches Java's
/// `DatasetVersionDao.findAll()` which returns enriched `DatasetVersion` objects.
pub async fn find_all(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<EnrichedDatasetVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, EnrichedDatasetVersionRow>(
        "WITH dataset_symlinks_names AS ( \
             SELECT name FROM dataset_symlinks WHERE NOT is_primary \
         ), selected_dataset_versions AS ( \
             SELECT dv.* FROM dataset_versions dv \
             WHERE dv.namespace_name = $1 AND dv.dataset_name = $2 \
             AND dv.dataset_name NOT IN (SELECT name FROM dataset_symlinks_names) \
             ORDER BY dv.created_at DESC \
             LIMIT $3 OFFSET $4 \
         ), selected_dataset_version_facets AS ( \
             SELECT dv.uuid, dv.dataset_name, dv.namespace_name, \
                    df.run_uuid, df.lineage_event_time, df.facet \
             FROM selected_dataset_versions dv \
             LEFT JOIN dataset_facets df ON df.dataset_version_uuid = dv.uuid \
                 AND (LOWER(df.type) IN ('dataset', 'unknown', 'input')) \
         ) \
         SELECT d.type, d.name, d.physical_name, d.namespace_name, d.source_name, \
                d.description, dv.lifecycle_state, dv.created_at, \
                dv.uuid AS current_version_uuid, dv.version, \
                dv.dataset_schema_version_uuid, dv.fields, \
                dv.run_uuid AS createdByRunUuid, \
                sv.schema_location, t.tags, f.facets \
         FROM selected_dataset_versions dv \
         LEFT JOIN datasets_view d ON d.uuid = dv.dataset_uuid \
         LEFT JOIN stream_versions AS sv ON sv.dataset_version_uuid = dv.uuid \
         LEFT JOIN ( \
             SELECT ARRAY_AGG(t.name) AS tags, m.dataset_uuid \
             FROM tags AS t \
             INNER JOIN datasets_tag_mapping AS m ON m.tag_uuid = t.uuid \
             GROUP BY m.dataset_uuid \
         ) t ON t.dataset_uuid = dv.dataset_uuid \
         LEFT JOIN ( \
             SELECT dvf.uuid AS dataset_uuid, \
                    JSONB_AGG(dvf.facet ORDER BY dvf.lineage_event_time ASC) AS facets \
             FROM selected_dataset_version_facets dvf \
             WHERE dvf.facet IS NOT NULL \
             GROUP BY dvf.uuid \
         ) f ON f.dataset_uuid = dv.uuid \
         ORDER BY dv.created_at DESC",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Find dataset versions that are inputs for a given run, via
/// `runs_input_mapping`.
pub async fn find_input_versions_for(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<Vec<DatasetVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetVersionRow>(
        "SELECT dv.* FROM dataset_versions dv \
         INNER JOIN runs_input_mapping rim \
         ON rim.dataset_version_uuid = dv.uuid \
         WHERE rim.run_uuid = $1",
    )
    .bind(run_uuid)
    .fetch_all(pool)
    .await
}

/// Find dataset versions that are outputs for a given run.
pub async fn find_output_versions_for(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<Vec<DatasetVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetVersionRow>("SELECT * FROM dataset_versions WHERE run_uuid = $1")
        .bind(run_uuid)
        .fetch_all(pool)
        .await
}

/// Count versions for a dataset.
pub async fn count(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
) -> Result<i64, sqlx::Error> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM dataset_versions \
         WHERE namespace_name = $1 AND dataset_name = $2",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Find an enriched dataset version by UUID.
///
/// Uses CTEs to gather version data, tags, and facets. Matches Java's
/// `DatasetVersionDao.findBy(UUID version)` — the complex CTE variant.
pub async fn find_enriched(
    pool: &PgPool,
    version_uuid: Uuid,
) -> Result<Option<EnrichedDatasetVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, EnrichedDatasetVersionRow>(
        "WITH selected_dataset_versions AS ( \
             SELECT dv.* FROM dataset_versions dv WHERE dv.uuid = $1 \
         ), selected_dataset_version_facets AS ( \
             SELECT dv.uuid, dv.dataset_name, dv.namespace_name, \
                    df.run_uuid, df.lineage_event_time, df.facet \
             FROM selected_dataset_versions dv \
             LEFT JOIN dataset_facets df ON df.dataset_version_uuid = dv.uuid \
                 AND (LOWER(df.type) IN ('dataset', 'unknown', 'input')) \
         ) \
         SELECT d.type, d.name, d.physical_name, d.namespace_name, d.source_name, \
                d.description, dv.lifecycle_state, dv.created_at, \
                dv.uuid AS current_version_uuid, dv.version, \
                dv.dataset_schema_version_uuid, dv.fields, \
                dv.run_uuid AS createdByRunUuid, \
                sv.schema_location, t.tags, f.facets \
         FROM selected_dataset_versions dv \
         LEFT JOIN datasets_view d ON d.uuid = dv.dataset_uuid \
         LEFT JOIN stream_versions AS sv ON sv.dataset_version_uuid = dv.uuid \
         LEFT JOIN ( \
             SELECT ARRAY_AGG(t.name) AS tags, m.dataset_uuid \
             FROM tags AS t \
             INNER JOIN datasets_tag_mapping AS m ON m.tag_uuid = t.uuid \
             GROUP BY m.dataset_uuid \
         ) t ON t.dataset_uuid = dv.dataset_uuid \
         LEFT JOIN ( \
             SELECT dvf.uuid AS dataset_uuid, \
                    JSONB_AGG(dvf.facet ORDER BY dvf.lineage_event_time ASC) AS facets \
             FROM selected_dataset_version_facets dvf \
             WHERE dvf.facet IS NOT NULL \
             GROUP BY dvf.uuid \
         ) f ON f.dataset_uuid = dv.uuid",
    )
    .bind(version_uuid)
    .fetch_optional(pool)
    .await
}

/// Update the `fields` JSONB column on a dataset version.
pub async fn update_fields(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    fields: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE dataset_versions SET fields = $2 WHERE uuid = $1")
        .bind(uuid)
        .bind(fields)
        .execute(exec)
        .await?;
    Ok(())
}

/// Insert a stream version row (ON CONFLICT DO NOTHING).
///
/// Links a dataset version to a schema location in the `stream_versions` table.
pub async fn insert_stream_version(
    exec: impl Executor<'_, Database = Postgres>,
    dataset_version_uuid: Uuid,
    schema_location: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO stream_versions (dataset_version_uuid, schema_location) \
         VALUES ($1, $2) \
         ON CONFLICT DO NOTHING",
    )
    .bind(dataset_version_uuid)
    .bind(schema_location)
    .execute(exec)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// dataset_symlinks helpers
// ---------------------------------------------------------------------------

/// Upsert a dataset symlink row.
///
/// INSERT ON CONFLICT(name, namespace_uuid) DO NOTHING, then SELECT the row.
pub async fn upsert_symlink(
    pool: &PgPool,
    uuid: Uuid,
    name: &str,
    namespace_uuid: Uuid,
    type_: Option<&str>,
    is_primary: bool,
    now: DateTime<Utc>,
) -> Result<DatasetSymlinkRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO dataset_symlinks \
         (dataset_uuid, name, namespace_uuid, is_primary, type, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $6) \
         ON CONFLICT (name, namespace_uuid) DO NOTHING",
    )
    .bind(uuid)
    .bind(name)
    .bind(namespace_uuid)
    .bind(is_primary)
    .bind(type_)
    .bind(now.naive_utc())
    .execute(pool)
    .await?;

    find_symlink_by_namespace_and_name(pool, namespace_uuid, name)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Find a symlink by namespace UUID and name.
pub async fn find_symlink_by_namespace_and_name(
    pool: &PgPool,
    namespace_uuid: Uuid,
    name: &str,
) -> Result<Option<DatasetSymlinkRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetSymlinkRow>(
        "SELECT * FROM dataset_symlinks \
         WHERE namespace_uuid = $1 AND name = $2",
    )
    .bind(namespace_uuid)
    .bind(name)
    .fetch_optional(pool)
    .await
}

// ---------------------------------------------------------------------------
// dataset_schema_versions helpers
// ---------------------------------------------------------------------------

/// Upsert a schema version row.
///
/// INSERT ON CONFLICT DO NOTHING, then returns the row (or None if
/// something unexpected happens).
pub async fn upsert_schema_version(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    dataset_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<DatasetSchemaVersionRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetSchemaVersionRow>(
        "INSERT INTO dataset_schema_versions (uuid, dataset_uuid, created_at) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (uuid) DO NOTHING \
         RETURNING *",
    )
    .bind(uuid)
    .bind(dataset_uuid)
    .bind(now)
    .fetch_optional(exec)
    .await
}

/// Batch insert field-to-schema-version mappings.
pub async fn upsert_schema_field_mappings(
    pool: &PgPool,
    schema_version_uuid: Uuid,
    field_uuids: &[Uuid],
) -> Result<(), sqlx::Error> {
    for field_uuid in field_uuids {
        sqlx::query(
            "INSERT INTO dataset_schema_versions_field_mapping \
             (dataset_schema_version_uuid, dataset_field_uuid) \
             VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(schema_version_uuid)
        .bind(field_uuid)
        .execute(pool)
        .await?;
    }
    Ok(())
}
