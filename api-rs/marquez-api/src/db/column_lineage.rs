// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `column_lineage` table.
//!
//! Tracks field-level lineage: which output dataset fields are derived from
//! which input dataset fields, including transformation metadata.
//!
//! Note: `column_lineage` timestamps are `TIMESTAMP` (not `TIMESTAMPTZ`), so
//! we use `NaiveDateTime` / `now.naive_utc()`.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{ColumnLineageNodeRow, ColumnLineageRow};

/// Upsert a column lineage row.
///
/// INSERT ON CONFLICT on the unique constraint
/// (output_dataset_version_uuid, output_dataset_field_uuid,
///  input_dataset_version_uuid, input_dataset_field_uuid)
/// DO UPDATE SET transformation_description, transformation_type, updated_at.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    output_dv_uuid: Uuid,
    output_field_uuid: Uuid,
    input_dv_uuid: Uuid,
    input_field_uuid: Uuid,
    transformation_description: Option<&str>,
    transformation_type: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO column_lineage (\
            output_dataset_version_uuid, output_dataset_field_uuid, \
            input_dataset_version_uuid, input_dataset_field_uuid, \
            transformation_description, transformation_type, \
            created_at, updated_at\
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $7) \
        ON CONFLICT (output_dataset_version_uuid, output_dataset_field_uuid, \
                     input_dataset_version_uuid, input_dataset_field_uuid) \
        DO UPDATE SET \
            transformation_description = EXCLUDED.transformation_description, \
            transformation_type = EXCLUDED.transformation_type, \
            updated_at = EXCLUDED.updated_at",
    )
    .bind(output_dv_uuid)
    .bind(output_field_uuid)
    .bind(input_dv_uuid)
    .bind(input_field_uuid)
    .bind(transformation_description)
    .bind(transformation_type)
    .bind(now.naive_utc())
    .execute(exec)
    .await?;
    Ok(())
}

/// Batch row for column lineage upserts.
pub struct ColumnLineageInput {
    pub output_dv_uuid: Uuid,
    pub output_field_uuid: Uuid,
    pub input_dv_uuid: Uuid,
    pub input_field_uuid: Uuid,
    pub transformation_description: Option<String>,
    pub transformation_type: Option<String>,
}

/// Batch upsert multiple column lineage rows in a single INSERT statement.
///
/// Reduces N+1 query overhead by inserting all rows for an output column at once.
pub async fn upsert_batch(
    exec: impl Executor<'_, Database = Postgres>,
    rows: &[ColumnLineageInput],
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }

    // Build a multi-row INSERT using UNNEST arrays
    let output_dv_uuids: Vec<Uuid> = rows.iter().map(|r| r.output_dv_uuid).collect();
    let output_field_uuids: Vec<Uuid> = rows.iter().map(|r| r.output_field_uuid).collect();
    let input_dv_uuids: Vec<Uuid> = rows.iter().map(|r| r.input_dv_uuid).collect();
    let input_field_uuids: Vec<Uuid> = rows.iter().map(|r| r.input_field_uuid).collect();
    let descriptions: Vec<Option<String>> = rows
        .iter()
        .map(|r| r.transformation_description.clone())
        .collect();
    let types: Vec<Option<String>> = rows.iter().map(|r| r.transformation_type.clone()).collect();

    let naive_now = now.naive_utc();

    sqlx::query(
        "INSERT INTO column_lineage (\
            output_dataset_version_uuid, output_dataset_field_uuid, \
            input_dataset_version_uuid, input_dataset_field_uuid, \
            transformation_description, transformation_type, \
            created_at, updated_at\
        ) SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::uuid[], $4::uuid[], \
            $5::text[], $6::text[]) AS t(\
            output_dataset_version_uuid, output_dataset_field_uuid, \
            input_dataset_version_uuid, input_dataset_field_uuid, \
            transformation_description, transformation_type), \
            (SELECT $7::timestamp AS created_at, $7::timestamp AS updated_at) AS ts \
        ON CONFLICT (output_dataset_version_uuid, output_dataset_field_uuid, \
                     input_dataset_version_uuid, input_dataset_field_uuid) \
        DO UPDATE SET \
            transformation_description = EXCLUDED.transformation_description, \
            transformation_type = EXCLUDED.transformation_type, \
            updated_at = EXCLUDED.updated_at",
    )
    .bind(&output_dv_uuids)
    .bind(&output_field_uuids)
    .bind(&input_dv_uuids)
    .bind(&input_field_uuids)
    .bind(&descriptions)
    .bind(&types)
    .bind(naive_now)
    .execute(exec)
    .await?;
    Ok(())
}

/// Find column lineage rows by output dataset version UUID.
pub async fn find_by_output_dataset_version(
    pool: &PgPool,
    output_dv_uuid: Uuid,
) -> Result<Vec<ColumnLineageRow>, sqlx::Error> {
    sqlx::query_as::<_, ColumnLineageRow>(
        "SELECT * FROM column_lineage \
         WHERE output_dataset_version_uuid = $1",
    )
    .bind(output_dv_uuid)
    .fetch_all(pool)
    .await
}

/// Find column lineage rows by input dataset version UUID.
pub async fn find_by_input_dataset_version(
    pool: &PgPool,
    input_dv_uuid: Uuid,
) -> Result<Vec<ColumnLineageRow>, sqlx::Error> {
    sqlx::query_as::<_, ColumnLineageRow>(
        "SELECT * FROM column_lineage \
         WHERE input_dataset_version_uuid = $1",
    )
    .bind(input_dv_uuid)
    .fetch_all(pool)
    .await
}

/// Get column lineage graph using a recursive CTE with cycle detection.
///
/// Starting from the given dataset field UUIDs, traverse upstream
/// (input direction) and optionally downstream (output direction).
/// Uses `ARRAY_AGG` to return aggregated results, eliminating N+1 queries.
///
/// Matches Java's `ColumnLineageDao.getLineage()` CTE.
pub async fn get_lineage(
    pool: &PgPool,
    depth: i32,
    dataset_field_uuids: &[Uuid],
    with_downstream: bool,
    created_at_until: DateTime<Utc>,
) -> Result<Vec<ColumnLineageNodeRow>, sqlx::Error> {
    sqlx::query_as::<_, ColumnLineageNodeRow>(
        "WITH RECURSIVE \
           column_lineage_latest AS ( \
             SELECT DISTINCT ON (output_dataset_field_uuid, input_dataset_field_uuid) * \
             FROM column_lineage \
             WHERE created_at <= $3 \
             ORDER BY output_dataset_field_uuid, input_dataset_field_uuid, updated_at DESC \
           ), \
           dataset_fields_view AS ( \
             SELECT d.namespace_name, d.name AS dataset_name, \
                    df.name AS field_name, df.type AS field_type, df.uuid, d.namespace_uuid \
             FROM dataset_fields df \
             INNER JOIN datasets_view d ON d.uuid = df.dataset_uuid \
           ), \
           column_lineage_recursive AS ( \
             SELECT *, 0 AS depth, false AS is_cycle, \
                    ARRAY[ROW(output_dataset_field_uuid, input_dataset_field_uuid)::record] AS path \
             FROM column_lineage_latest \
             WHERE output_dataset_field_uuid = ANY($1) \
             UNION ALL \
             SELECT \
               adj.output_dataset_version_uuid, \
               adj.output_dataset_field_uuid, \
               adj.input_dataset_version_uuid, \
               adj.input_dataset_field_uuid, \
               adj.transformation_description, \
               adj.transformation_type, \
               adj.created_at, \
               adj.updated_at, \
               node.depth + 1, \
               ROW(adj.input_dataset_field_uuid, adj.output_dataset_field_uuid)::record = ANY(path), \
               path || ROW(adj.input_dataset_field_uuid, adj.output_dataset_field_uuid)::record \
             FROM column_lineage_latest adj, column_lineage_recursive node \
             WHERE ( \
               node.input_dataset_field_uuid = adj.output_dataset_field_uuid \
               OR ($4 AND adj.input_dataset_field_uuid = node.output_dataset_field_uuid) \
             ) \
             AND node.depth < $2 - 1 \
             AND NOT is_cycle \
           ) \
         SELECT \
           output_fields.namespace_name, \
           output_fields.dataset_name, \
           output_fields.field_name, \
           output_fields.field_type, \
           JSONB_AGG(DISTINCT JSONB_BUILD_ARRAY( \
             input_fields.namespace_name, \
             input_fields.dataset_name, \
             CAST(clr.input_dataset_version_uuid AS VARCHAR), \
             input_fields.field_name, \
             clr.transformation_description, \
             clr.transformation_type \
           )) AS input_fields, \
           clr.output_dataset_version_uuid AS dataset_version_uuid \
         FROM column_lineage_recursive clr \
         INNER JOIN dataset_fields_view output_fields \
           ON clr.output_dataset_field_uuid = output_fields.uuid \
         INNER JOIN dataset_symlinks ds_output \
           ON ds_output.namespace_uuid = output_fields.namespace_uuid \
           AND ds_output.name = output_fields.dataset_name \
         LEFT JOIN dataset_fields_view input_fields \
           ON clr.input_dataset_field_uuid = input_fields.uuid \
         INNER JOIN dataset_symlinks ds_input \
           ON ds_input.namespace_uuid = input_fields.namespace_uuid \
           AND ds_input.name = input_fields.dataset_name \
         WHERE NOT clr.is_cycle \
           AND ds_output.is_primary IS TRUE \
           AND ds_input.is_primary IS TRUE \
         GROUP BY \
           output_fields.namespace_name, \
           output_fields.dataset_name, \
           output_fields.field_name, \
           output_fields.field_type, \
           clr.output_dataset_version_uuid",
    )
    .bind(dataset_field_uuids)
    .bind(depth)
    .bind(created_at_until.naive_utc())
    .bind(with_downstream)
    .fetch_all(pool)
    .await
}
