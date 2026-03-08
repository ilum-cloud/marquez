// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `sources` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::SourceRow;

/// Upsert a source row.
///
/// INSERT ... ON CONFLICT(name) DO UPDATE SET type, updated_at,
/// connection_url, and optionally description. Returns the resulting row.
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    type_: &str,
    now: DateTime<Utc>,
    name: &str,
    connection_url: &str,
    description: Option<&str>,
) -> Result<SourceRow, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>(
        "INSERT INTO sources (uuid, type, created_at, updated_at, name, connection_url, description) \
         VALUES ($1, $2, $3, $3, $4, $5, $6) \
         ON CONFLICT(name) DO UPDATE SET \
         type = EXCLUDED.type, \
         updated_at = EXCLUDED.updated_at, \
         connection_url = EXCLUDED.connection_url, \
         description = EXCLUDED.description \
         RETURNING *",
    )
    .bind(uuid)
    .bind(type_)
    .bind(now)
    .bind(name)
    .bind(connection_url)
    .bind(description)
    .fetch_one(exec)
    .await
}

/// Upsert a source with default semantics — on conflict, only update `updated_at`.
///
/// Does NOT overwrite `type` or `connection_url` if the source already exists.
/// Matches Java's `SourceDao.upsertOrDefault()`.
pub async fn upsert_or_default(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    type_: &str,
    now: DateTime<Utc>,
    name: &str,
    connection_url: &str,
) -> Result<SourceRow, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>(
        "INSERT INTO sources (uuid, type, created_at, updated_at, name, connection_url) \
         VALUES ($1, $2, $3, $3, $4, $5) \
         ON CONFLICT(name) DO UPDATE SET updated_at = EXCLUDED.updated_at \
         RETURNING *",
    )
    .bind(uuid)
    .bind(type_)
    .bind(now)
    .bind(name)
    .bind(connection_url)
    .fetch_one(exec)
    .await
}

/// Returns `true` if a source with the given name exists.
pub async fn exists(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM sources WHERE name = $1)")
            .bind(name)
            .fetch_one(pool)
            .await?;
    Ok(result)
}

/// Find a source by exact name. Returns `None` if not found.
pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<SourceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>("SELECT * FROM sources WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
}

/// List sources ordered by name with limit/offset pagination.
pub async fn find_all(
    pool: &PgPool,
    limit: i32,
    offset: i32,
) -> Result<Vec<SourceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>("SELECT * FROM sources ORDER BY name LIMIT $1 OFFSET $2")
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}
