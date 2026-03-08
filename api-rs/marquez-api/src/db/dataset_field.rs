// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `dataset_fields` table and related mappings.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{
    DatasetFieldRow, DatasetFieldWithTagsRow, FieldUuidTimestampRow, InputFieldDataRow,
};

/// Upsert a dataset field row.
///
/// INSERT ON CONFLICT(dataset_uuid, name, type) DO UPDATE SET updated_at,
/// description. Type defaults to `'UNKNOWN'` when `None`.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    name: &str,
    type_: Option<&str>,
    description: Option<&str>,
    dataset_uuid: Uuid,
) -> Result<DatasetFieldRow, sqlx::Error> {
    sqlx::query_as::<_, DatasetFieldRow>(
        "INSERT INTO dataset_fields \
         (uuid, type, created_at, updated_at, dataset_uuid, name, description) \
         VALUES ($1, COALESCE($2, 'UNKNOWN'), $3, $3, $4, $5, $6) \
         ON CONFLICT(dataset_uuid, name, type) DO UPDATE SET \
         updated_at = EXCLUDED.updated_at, \
         description = EXCLUDED.description \
         RETURNING *",
    )
    .bind(uuid)
    .bind(type_)
    .bind(now)
    .bind(dataset_uuid)
    .bind(name)
    .bind(description)
    .fetch_one(exec)
    .await
}

/// Returns `true` if a field with the given dataset UUID and name exists.
pub async fn exists(pool: &PgPool, dataset_uuid: Uuid, name: &str) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (\
         SELECT 1 FROM dataset_fields \
         WHERE dataset_uuid = $1 AND name = $2\
         )",
    )
    .bind(dataset_uuid)
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(result)
}

/// Find all fields for a dataset by its UUID, including tags.
///
/// Queries `dataset_fields` directly by `dataset_uuid` and collects tags
/// from `dataset_fields_tag_mapping`. This avoids going through any version
/// mapping table, which may be empty for the current version.
///
/// Used by the dataset GET enrichment path to always return the correct
/// fields regardless of version field mapping state.
pub async fn find_by_dataset_uuid_with_tags(
    pool: &PgPool,
    dataset_uuid: Uuid,
) -> Result<Vec<DatasetFieldWithTagsRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetFieldWithTagsRow>(
        "SELECT f.*, \
         ARRAY(SELECT t.name \
               FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         WHERE f.dataset_uuid = $1",
    )
    .bind(dataset_uuid)
    .fetch_all(pool)
    .await
}

/// Batch-fetch fields with tags for multiple dataset UUIDs.
///
/// Returns a map from dataset_uuid → fields. Each field includes its tags.
/// Used by `DatasetService::list()` to avoid per-dataset field queries.
pub async fn find_by_dataset_uuids_with_tags(
    pool: &PgPool,
    dataset_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<DatasetFieldWithTagsRow>>, sqlx::Error> {
    let rows = sqlx::query_as::<_, DatasetFieldWithTagsRow>(
        "SELECT f.*, \
         ARRAY(SELECT t.name \
               FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         WHERE f.dataset_uuid = ANY($1)",
    )
    .bind(dataset_uuids)
    .fetch_all(pool)
    .await?;

    let mut map: std::collections::HashMap<Uuid, Vec<DatasetFieldWithTagsRow>> =
        std::collections::HashMap::new();
    for row in rows {
        if let Some(ds_uuid) = row.dataset_uuid {
            map.entry(ds_uuid).or_default().push(row);
        }
    }
    Ok(map)
}

/// Find fields for a single dataset version via `dataset_versions_field_mapping`,
/// including tags. Used by `enrich_dataset()` (single-get path).
///
/// PK lookup on `dataset_versions_field_mapping(dataset_version_uuid, dataset_field_uuid)`.
pub async fn find_by_version_with_tags(
    pool: &PgPool,
    dataset_version_uuid: Uuid,
) -> Result<Vec<DatasetFieldWithTagsRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetFieldWithTagsRow>(
        "SELECT f.*, \
         ARRAY(SELECT t.name \
               FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         INNER JOIN dataset_versions_field_mapping fm ON fm.dataset_field_uuid = f.uuid \
         WHERE fm.dataset_version_uuid = $1",
    )
    .bind(dataset_version_uuid)
    .fetch_all(pool)
    .await
}

/// Batch-fetch fields with tags for multiple dataset versions via
/// `dataset_versions_field_mapping`. Returns a map from `dataset_uuid` → fields.
///
/// Used by `DatasetService::list()` to show only current-version fields.
/// PK scan on `dataset_versions_field_mapping(dataset_version_uuid)`.
pub async fn find_by_version_uuids_with_tags(
    pool: &PgPool,
    version_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<DatasetFieldWithTagsRow>>, sqlx::Error> {
    if version_uuids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let rows = sqlx::query_as::<_, DatasetFieldWithTagsRow>(
        "SELECT f.*, \
         ARRAY(SELECT t.name \
               FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         INNER JOIN dataset_versions_field_mapping fm ON fm.dataset_field_uuid = f.uuid \
         WHERE fm.dataset_version_uuid = ANY($1)",
    )
    .bind(version_uuids)
    .fetch_all(pool)
    .await?;

    let mut map: std::collections::HashMap<Uuid, Vec<DatasetFieldWithTagsRow>> =
        std::collections::HashMap::new();
    for row in rows {
        if let Some(ds_uuid) = row.dataset_uuid {
            map.entry(ds_uuid).or_default().push(row);
        }
    }
    Ok(map)
}

/// Find field UUIDs for a dataset across ALL versions, using dataset symlinks.
///
/// Matches Java's `DatasetFieldDao.findDatasetFieldsUuids(namespace, name)`:
/// ```sql
/// SELECT df.uuid FROM dataset_fields df
/// JOIN datasets_view AS d ON d.uuid = df.dataset_uuid
/// JOIN dataset_versions_field_mapping AS fm ON fm.dataset_field_uuid = df.uuid
/// JOIN dataset_versions AS dv ON dv.uuid = fm.dataset_version_uuid
/// WHERE CAST((:namespaceName, :datasetName) AS DATASET_NAME) = ANY(d.dataset_symlinks)
/// GROUP BY df.uuid
/// ```
pub async fn find_dataset_fields_uuids(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT df.uuid \
         FROM dataset_fields df \
         JOIN datasets_view AS d ON d.uuid = df.dataset_uuid \
         JOIN dataset_versions_field_mapping AS fm ON fm.dataset_field_uuid = df.uuid \
         JOIN dataset_versions AS dv ON dv.uuid = fm.dataset_version_uuid \
         WHERE CAST(($1, $2) AS DATASET_NAME) = ANY(d.dataset_symlinks) \
         GROUP BY df.uuid",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

/// Find fields associated with a dataset version via the
/// `dataset_versions_field_mapping` join table.
pub async fn find_by_dataset_version(
    pool: &PgPool,
    dataset_version_uuid: Uuid,
) -> Result<Vec<DatasetFieldRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetFieldRow>(
        "SELECT f.* FROM dataset_fields f \
         INNER JOIN dataset_versions_field_mapping fm \
         ON fm.dataset_field_uuid = f.uuid \
         WHERE fm.dataset_version_uuid = $1",
    )
    .bind(dataset_version_uuid)
    .fetch_all(pool)
    .await
}

/// Find the UUID of a dataset field by dataset UUID and field name.
pub async fn find_uuid(
    pool: &PgPool,
    dataset_uuid: Uuid,
    field_name: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT uuid FROM dataset_fields \
         WHERE dataset_uuid = $1 AND name = $2",
    )
    .bind(dataset_uuid)
    .bind(field_name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(u,)| u))
}

/// Batch insert field-to-version mappings.
///
/// Each mapping is inserted individually with ON CONFLICT DO NOTHING.
pub async fn update_field_mapping(
    pool: &PgPool,
    dataset_version_uuid: Uuid,
    field_uuids: &[Uuid],
) -> Result<(), sqlx::Error> {
    for field_uuid in field_uuids {
        sqlx::query(
            "INSERT INTO dataset_versions_field_mapping \
             (dataset_version_uuid, dataset_field_uuid) \
             VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(dataset_version_uuid)
        .bind(field_uuid)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Remove a tag from a dataset field.
pub async fn delete_tag(
    exec: impl Executor<'_, Database = Postgres>,
    field_uuid: Uuid,
    tag_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM dataset_fields_tag_mapping \
         WHERE dataset_field_uuid = $1 AND tag_uuid = $2",
    )
    .bind(field_uuid)
    .bind(tag_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Tag a dataset field (ON CONFLICT DO NOTHING).
pub async fn update_tags(
    exec: impl Executor<'_, Database = Postgres>,
    field_uuid: Uuid,
    tag_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO dataset_fields_tag_mapping \
         (dataset_field_uuid, tag_uuid, tagged_at) \
         VALUES ($1, $2, $3) \
         ON CONFLICT DO NOTHING",
    )
    .bind(field_uuid)
    .bind(tag_uuid)
    .bind(now)
    .execute(exec)
    .await?;
    Ok(())
}

/// Find field UUIDs associated with the latest run of a job.
///
/// Uses a CTE to find the latest run for the given job (by namespace + name),
/// then joins through `dataset_versions` to find all dataset fields connected
/// to that run's output datasets.
pub async fn find_fields_uuids_by_job(
    pool: &PgPool,
    namespace_name: &str,
    job_name: &str,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "WITH latest_run AS ( \
             SELECT DISTINCT r.uuid AS uuid, r.created_at \
             FROM runs_view r \
             WHERE r.namespace_name = $1 AND r.job_name = $2 \
             ORDER BY r.created_at DESC, r.uuid DESC \
             LIMIT 1 \
         ) \
         SELECT dataset_fields.uuid \
         FROM dataset_fields \
         JOIN dataset_versions ON dataset_versions.dataset_uuid = dataset_fields.dataset_uuid \
         JOIN latest_run ON dataset_versions.run_uuid = latest_run.uuid",
    )
    .bind(namespace_name)
    .bind(job_name)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

/// Find field UUIDs with timestamps for a given job version.
///
/// Joins `dataset_fields` through `dataset_versions` and `runs` to find all
/// fields associated with runs that belong to the specified job version.
pub async fn find_fields_uuids_by_job_version(
    pool: &PgPool,
    job_version: Uuid,
) -> Result<Vec<FieldUuidTimestampRow>, sqlx::Error> {
    sqlx::query_as::<_, FieldUuidTimestampRow>(
        "SELECT dataset_fields.uuid, r.created_at \
         FROM dataset_fields \
         JOIN dataset_versions ON dataset_versions.dataset_uuid = dataset_fields.dataset_uuid \
         JOIN runs_view r ON r.job_version_uuid = $1",
    )
    .bind(job_version)
    .fetch_all(pool)
    .await
}

/// Find field UUIDs with timestamps for a given dataset version, optionally
/// filtered by field name.
///
/// Covers both Java overloads of `DatasetFieldDao.findDatasetVersionFieldsUuids`:
/// - `findDatasetVersionFieldsUuids(UUID datasetVersion)` — all fields
/// - `findDatasetVersionFieldsUuids(String fieldName, UUID datasetVersion)` — filtered
pub async fn find_dataset_version_fields_uuids(
    pool: &PgPool,
    dataset_version: Uuid,
    field_name: Option<&str>,
) -> Result<Vec<FieldUuidTimestampRow>, sqlx::Error> {
    match field_name {
        Some(name) => {
            sqlx::query_as::<_, FieldUuidTimestampRow>(
                "SELECT df.uuid, dv.created_at \
                 FROM dataset_fields df \
                 JOIN datasets_view AS d ON d.uuid = df.dataset_uuid \
                 JOIN dataset_versions AS dv ON dv.uuid = $1 \
                 JOIN dataset_versions_field_mapping AS fm ON fm.dataset_field_uuid = df.uuid \
                 WHERE fm.dataset_version_uuid = $1 AND df.name = $2",
            )
            .bind(dataset_version)
            .bind(name)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, FieldUuidTimestampRow>(
                "SELECT df.uuid, dv.created_at \
                 FROM dataset_fields df \
                 JOIN dataset_versions_field_mapping AS fm ON fm.dataset_field_uuid = df.uuid \
                 JOIN dataset_versions AS dv ON dv.uuid = $1 \
                 WHERE fm.dataset_version_uuid = $1",
            )
            .bind(dataset_version)
            .fetch_all(pool)
            .await
        }
    }
}

/// Batch-fetch fields with tags for multiple dataset schema versions.
///
/// Returns a map from schema_version_uuid → fields with tags.
/// Used by `DatasetService::list_versions()` to avoid per-version field queries.
///
/// Two-step approach: first fetch the (schema_version_uuid, field_uuid) mappings,
/// then batch-fetch all fields with tags by their UUIDs.
pub async fn find_by_dataset_schema_versions_batch(
    pool: &PgPool,
    schema_version_uuids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<DatasetFieldWithTagsRow>>, sqlx::Error> {
    if schema_version_uuids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    // Step 1: Get all (schema_version_uuid, field_uuid) pairs.
    let mappings: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT dataset_schema_version_uuid, dataset_field_uuid \
         FROM dataset_schema_versions_field_mapping \
         WHERE dataset_schema_version_uuid = ANY($1)",
    )
    .bind(schema_version_uuids)
    .fetch_all(pool)
    .await?;

    if mappings.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    // Step 2: Collect unique field UUIDs and batch-fetch fields with tags.
    let field_uuids: Vec<Uuid> = mappings
        .iter()
        .map(|(_, fu)| *fu)
        .collect::<std::collections::HashSet<Uuid>>()
        .into_iter()
        .collect();

    let field_rows = sqlx::query_as::<_, DatasetFieldWithTagsRow>(
        "SELECT f.*, \
         ARRAY(SELECT t.name \
               FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         WHERE f.uuid = ANY($1)",
    )
    .bind(&field_uuids)
    .fetch_all(pool)
    .await?;

    // Step 3: Build field lookup and group by schema_version_uuid.
    let field_by_uuid: std::collections::HashMap<Uuid, DatasetFieldWithTagsRow> =
        field_rows.into_iter().map(|f| (f.uuid, f)).collect();

    let mut map: std::collections::HashMap<Uuid, Vec<DatasetFieldWithTagsRow>> =
        std::collections::HashMap::new();
    for (schema_uuid, field_uuid) in mappings {
        if let Some(field) = field_by_uuid.get(&field_uuid) {
            map.entry(schema_uuid).or_default().push(field.clone());
        }
    }
    Ok(map)
}

/// Find fields associated with a dataset schema version, including tags.
///
/// Matches Java `DatasetFieldDao.findByDatasetSchemaVersion(UUID)`.
pub async fn find_by_dataset_schema_version(
    pool: &PgPool,
    schema_version_uuid: Uuid,
) -> Result<Vec<DatasetFieldWithTagsRow>, sqlx::Error> {
    sqlx::query_as::<_, DatasetFieldWithTagsRow>(
        "SELECT f.*, \
         ARRAY(SELECT t.name \
               FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         INNER JOIN dataset_schema_versions_field_mapping fm \
         ON fm.dataset_field_uuid = f.uuid \
         WHERE fm.dataset_schema_version_uuid = $1",
    )
    .bind(schema_version_uuid)
    .fetch_all(pool)
    .await
}

/// Delete a tag from a dataset version's fields JSONB column.
///
/// Updates the `fields` JSONB array in `dataset_versions` to remove the
/// specified tag from the matching field entry. Only modifies rows where the
/// field name and tag both exist.
pub async fn delete_dataset_version_field_tag(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
    field_name: &str,
    tag_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE dataset_versions \
         SET fields = ( \
             SELECT jsonb_agg( \
                 CASE \
                     WHEN elem->>'name' = $3 AND elem->'tags' @> jsonb_build_array($4) \
                     THEN jsonb_set(elem, '{tags}', (elem->'tags') - $4) \
                     ELSE elem \
                 END \
             ) \
             FROM jsonb_array_elements(fields) AS elem \
         ) \
         WHERE dataset_name = $2 AND namespace_name = $1 \
         AND EXISTS ( \
             SELECT 1 \
             FROM jsonb_array_elements(fields) AS elem \
             WHERE elem->>'name' = $3 \
             AND elem->'tags' @> jsonb_build_array($4) \
         )",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .bind(field_name)
    .bind(tag_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Add a tag to a dataset version's fields JSONB column.
///
/// Updates the `fields` JSONB array in `dataset_versions` to add the
/// specified tag to the matching field entry. Only modifies rows where the
/// field name exists and the tag is not already present.
pub async fn update_dataset_version_field_tag(
    pool: &PgPool,
    namespace_name: &str,
    dataset_name: &str,
    field_name: &str,
    tag_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE dataset_versions \
         SET fields = ( \
             SELECT jsonb_agg( \
                 CASE \
                     WHEN elem->>'name' = $3 AND NOT (COALESCE(elem->'tags', '[]'::jsonb) @> jsonb_build_array($4)) \
                     THEN jsonb_set(elem, '{tags}', COALESCE(elem->'tags', '[]'::jsonb) || jsonb_build_array($4)) \
                     ELSE elem \
                 END \
             ) \
             FROM jsonb_array_elements(fields) AS elem \
         ) \
         WHERE dataset_name = $2 AND namespace_name = $1 \
         AND EXISTS ( \
             SELECT 1 \
             FROM jsonb_array_elements(fields) AS elem \
             WHERE elem->>'name' = $3 \
         )",
    )
    .bind(namespace_name)
    .bind(dataset_name)
    .bind(field_name)
    .bind(tag_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Find input fields data associated with a run.
///
/// Joins `dataset_fields` through `dataset_versions_field_mapping`,
/// `dataset_versions`, `datasets_view`, and `runs_input_mapping` to find
/// all input field data for a given run UUID.
/// Matches Java `DatasetFieldDao.findInputFieldsDataAssociatedWithRun()`.
pub async fn find_input_fields_data_associated_with_run(
    exec: impl Executor<'_, Database = Postgres>,
    run_uuid: Uuid,
) -> Result<Vec<InputFieldDataRow>, sqlx::Error> {
    sqlx::query_as::<_, InputFieldDataRow>(
        "SELECT datasets_view.namespace_name AS namespace_name, \
               datasets_view.name AS dataset_name, \
               dataset_fields.name AS field_name, \
               datasets_view.uuid AS dataset_uuid, \
               dataset_versions.uuid AS dataset_version_uuid, \
               dataset_fields.uuid AS dataset_field_uuid \
        FROM dataset_fields \
        JOIN dataset_versions_field_mapping fm ON fm.dataset_field_uuid = dataset_fields.uuid \
        JOIN dataset_versions ON dataset_versions.uuid = fm.dataset_version_uuid \
        JOIN datasets_view ON datasets_view.uuid = dataset_versions.dataset_uuid \
        JOIN runs_input_mapping ON runs_input_mapping.dataset_version_uuid = dataset_versions.uuid \
        WHERE runs_input_mapping.run_uuid = $1",
    )
    .bind(run_uuid)
    .fetch_all(exec)
    .await
}
