// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `namespaces` and `owners` tables.

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{NamespaceRow, OwnerRow};

/// Upsert a namespace row.
///
/// When `description` is `Some`, performs INSERT ... ON CONFLICT DO UPDATE
/// (sets updated_at, is_hidden=false) and returns the row via RETURNING.
///
/// When `description` is `None`, performs INSERT ... ON CONFLICT DO NOTHING
/// to avoid unnecessary lock contention on the write path, then SELECTs the
/// row by name. If the returned row is hidden, it is automatically un-hidden.
pub async fn upsert(
    pool: &PgPool,
    uuid: Uuid,
    now: DateTime<Utc>,
    name: &str,
    owner_name: &str,
    description: Option<&str>,
) -> Result<NamespaceRow, sqlx::Error> {
    let row = match description {
        Some(desc) => {
            sqlx::query_as::<_, NamespaceRow>(
                "INSERT INTO namespaces (uuid, created_at, updated_at, name, current_owner_name, description, is_hidden) \
                 VALUES ($1, $2, $2, $3, $4, $5, false) \
                 ON CONFLICT(name) DO UPDATE SET updated_at = EXCLUDED.updated_at, current_owner_name = EXCLUDED.current_owner_name, is_hidden = false \
                 RETURNING *",
            )
            .bind(uuid)
            .bind(now)
            .bind(name)
            .bind(owner_name)
            .bind(desc)
            .fetch_one(pool)
            .await?
        }
        None => {
            // INSERT ... ON CONFLICT DO NOTHING — avoids lock contention.
            sqlx::query(
                "INSERT INTO namespaces (uuid, created_at, updated_at, name, current_owner_name) \
                 VALUES ($1, $2, $2, $3, $4) \
                 ON CONFLICT(name) DO NOTHING",
            )
            .bind(uuid)
            .bind(now)
            .bind(name)
            .bind(owner_name)
            .execute(pool)
            .await?;

            sqlx::query_as::<_, NamespaceRow>(
                "SELECT * FROM namespaces WHERE name = $1",
            )
            .bind(name)
            .fetch_one(pool)
            .await?
        }
    };

    // If the row was soft-deleted, un-hide it.
    if row.is_hidden.unwrap_or(false) {
        if let Some(restored) = undelete(pool, name).await? {
            return Ok(restored);
        }
    }

    Ok(row)
}

/// Returns `true` if a namespace with the given name exists.
pub async fn exists(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
    let (result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM namespaces WHERE name = $1)")
            .bind(name)
            .fetch_one(pool)
            .await?;
    Ok(result)
}

/// Find a namespace by exact name. Returns `None` if not found.
pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<NamespaceRow>, sqlx::Error> {
    sqlx::query_as::<_, NamespaceRow>("SELECT * FROM namespaces WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
}

/// List namespaces ordered by name with limit/offset pagination.
pub async fn find_all(
    pool: &PgPool,
    limit: i32,
    offset: i32,
) -> Result<Vec<NamespaceRow>, sqlx::Error> {
    sqlx::query_as::<_, NamespaceRow>("SELECT * FROM namespaces ORDER BY name LIMIT $1 OFFSET $2")
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

/// List namespaces excluding those matching a regex pattern.
///
/// Matches Java's `NamespaceDao.findAllWithExclusion()`:
/// `SELECT * FROM namespaces WHERE name !~ :excluded ORDER BY name LIMIT :limit OFFSET :offset`
pub async fn find_all_with_exclusion(
    pool: &PgPool,
    excluded_pattern: &str,
    limit: i32,
    offset: i32,
) -> Result<Vec<NamespaceRow>, sqlx::Error> {
    sqlx::query_as::<_, NamespaceRow>(
        "SELECT * FROM namespaces WHERE name !~ $1 ORDER BY name LIMIT $2 OFFSET $3",
    )
    .bind(excluded_pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Soft-delete a namespace by setting `is_hidden = true`.
pub async fn delete(
    exec: impl Executor<'_, Database = Postgres>,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE namespaces SET is_hidden = true WHERE name = $1")
        .bind(name)
        .execute(exec)
        .await?;
    Ok(())
}

/// Restore a soft-deleted namespace by setting `is_hidden = false`.
pub async fn undelete(
    exec: impl Executor<'_, Database = Postgres>,
    name: &str,
) -> Result<Option<NamespaceRow>, sqlx::Error> {
    sqlx::query_as::<_, NamespaceRow>(
        "UPDATE namespaces SET is_hidden = false WHERE name = $1 RETURNING *",
    )
    .bind(name)
    .fetch_optional(exec)
    .await
}

/// Update the current owner name for a namespace.
///
/// Matches Java `NamespaceDao.setCurrentOwner(rowUuid, updatedAt, currentOwnerName)`.
pub async fn set_current_owner(
    exec: impl Executor<'_, Database = Postgres>,
    row_uuid: Uuid,
    updated_at: DateTime<Utc>,
    current_owner_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE namespaces SET updated_at = $1, current_owner_name = $2 \
         WHERE uuid = $3",
    )
    .bind(updated_at)
    .bind(current_owner_name)
    .bind(row_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// Upsert an owner row (INSERT ... ON CONFLICT DO UPDATE).
pub async fn upsert_owner(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    name: &str,
) -> Result<OwnerRow, sqlx::Error> {
    sqlx::query_as::<_, OwnerRow>(
        "INSERT INTO owners (uuid, created_at, name) \
         VALUES ($1, $2, $3) \
         ON CONFLICT(name) DO UPDATE SET created_at = EXCLUDED.created_at \
         RETURNING *",
    )
    .bind(uuid)
    .bind(now.naive_utc())
    .bind(name)
    .fetch_one(exec)
    .await
}

/// Find an owner by exact name. Returns `None` if not found.
pub async fn find_owner(pool: &PgPool, name: &str) -> Result<Option<OwnerRow>, sqlx::Error> {
    sqlx::query_as::<_, OwnerRow>("SELECT * FROM owners WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
}

/// Insert a namespace ownership record.
pub async fn start_ownership(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    started_at: NaiveDateTime,
    namespace_uuid: Uuid,
    owner_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO namespace_ownerships (uuid, started_at, namespace_uuid, owner_uuid) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(uuid)
    .bind(started_at)
    .bind(namespace_uuid)
    .bind(owner_uuid)
    .execute(exec)
    .await?;
    Ok(())
}

/// End a namespace ownership by setting `ended_at`.
pub async fn end_ownership(
    exec: impl Executor<'_, Database = Postgres>,
    ended_at: NaiveDateTime,
    namespace_uuid: Uuid,
    owner_uuid: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE namespace_ownerships \
         SET ended_at = $1 \
         WHERE namespace_uuid = $2 AND owner_uuid = $3",
    )
    .bind(ended_at)
    .bind(namespace_uuid)
    .bind(owner_uuid)
    .execute(exec)
    .await?;
    Ok(())
}
