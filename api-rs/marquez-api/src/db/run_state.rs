// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for the `run_states` table.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::RunStateRow;

/// Insert a new run state row. Returns the inserted row.
///
/// Note: `run_states` has no ON CONFLICT clause because there is no
/// unique constraint other than the primary key (`uuid`).
pub async fn upsert(
    exec: impl Executor<'_, Database = Postgres>,
    uuid: Uuid,
    now: DateTime<Utc>,
    run_uuid: Uuid,
    state: &str,
) -> Result<RunStateRow, sqlx::Error> {
    sqlx::query_as::<_, RunStateRow>(
        "INSERT INTO run_states (uuid, transitioned_at, run_uuid, state) \
         VALUES ($1, $2, $3, $4) \
         RETURNING *",
    )
    .bind(uuid)
    .bind(now)
    .bind(run_uuid)
    .bind(state)
    .fetch_one(exec)
    .await
}

/// Find all run states for a given run, ordered by `transitioned_at` descending
/// (most recent first).
pub async fn find_by_run_uuid(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<Vec<RunStateRow>, sqlx::Error> {
    sqlx::query_as::<_, RunStateRow>(
        "SELECT * FROM run_states \
         WHERE run_uuid = $1 \
         ORDER BY transitioned_at DESC",
    )
    .bind(run_uuid)
    .fetch_all(pool)
    .await
}
