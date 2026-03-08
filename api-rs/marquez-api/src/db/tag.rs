// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `tags` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::TagRow;

/// Upsert a tag row.
///
/// INSERT ... ON CONFLICT(name) DO UPDATE SET updated_at and optionally
/// description. Returns the resulting row.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    name: &str,
    description: Option<&str>,
) -> Result<TagRow, sqlx::Error> {
    sqlx::query_as::<_, TagRow>(
        "INSERT INTO tags (uuid, created_at, updated_at, name, description) \
         VALUES ($1, $2, $2, $3, $4) \
         ON CONFLICT(name) DO UPDATE SET \
         updated_at = EXCLUDED.updated_at, \
         description = COALESCE(EXCLUDED.description, tags.description) \
         RETURNING *",
    )
    .bind(uuid)
    .bind(now)
    .bind(name)
    .bind(description)
    .fetch_one(exec)
    .await
}

/// Returns `true` if a tag with the given name exists.
pub async fn exists(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM tags WHERE name = $1)")
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(result)
}

/// Find a tag by exact name. Returns `None` if not found.
pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<TagRow>, sqlx::Error> {
    sqlx::query_as::<_, TagRow>("SELECT * FROM tags WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
}

/// List tags ordered by name with limit/offset pagination.
pub async fn find_all(pool: &PgPool, limit: i32, offset: i32) -> Result<Vec<TagRow>, sqlx::Error> {
    sqlx::query_as::<_, TagRow>("SELECT * FROM tags ORDER BY name LIMIT $1 OFFSET $2")
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}
