// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `run_args` table.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::db::RunArgsRow;

/// Upsert a run_args row.
///
/// Performs INSERT ... ON CONFLICT(checksum) DO NOTHING, then SELECTs by
/// checksum.  Uses `&PgPool` because two queries are needed.
pub async fn upsert(
    pool: &PgPool,
    uuid: Uuid,
    now: DateTime<Utc>,
    args: &str,
    checksum: &str,
) -> Result<RunArgsRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO run_args (uuid, created_at, args, checksum) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT(checksum) DO NOTHING",
    )
    .bind(uuid)
    .bind(now.naive_utc())
    .bind(args)
    .bind(checksum)
    .execute(pool)
    .await?;

    // The row now exists (either just inserted or already present).
    find_by_checksum(pool, checksum)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

/// Find run_args by checksum. Returns `None` if not found.
pub async fn find_by_checksum(
    pool: &PgPool,
    checksum: &str,
) -> Result<Option<RunArgsRow>, sqlx::Error> {
    sqlx::query_as::<_, RunArgsRow>("SELECT * FROM run_args WHERE checksum = $1")
        .bind(checksum)
        .fetch_optional(pool)
        .await
}
