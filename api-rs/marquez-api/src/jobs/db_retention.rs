use sqlx::PgPool;
use tokio::time::{interval, sleep, Duration};
use tokio_util::sync::CancellationToken;

/// Configuration for the DB retention job.
pub struct RetentionConfig {
    pub frequency_mins: u64,
    pub batch_size: i64,
    pub retention_days: i32,
}

/// Run the DB retention job on a periodic schedule.
///
/// Applies retention policy every `frequency_mins` minutes.
/// Uses a `CancellationToken` for clean shutdown.
pub async fn run(pool: PgPool, config: RetentionConfig, token: CancellationToken) {
    let mut tick = interval(Duration::from_secs(config.frequency_mins * 60));
    tracing::info!(
        "DB retention job started (every {} mins, retention: {} days, batch: {})",
        config.frequency_mins,
        config.retention_days,
        config.batch_size
    );

    loop {
        tokio::select! {
            _ = tick.tick() => {
                if let Err(e) = run_retention(&pool, &config).await {
                    tracing::error!("DB retention job failed: {}", e);
                }
            }
            _ = token.cancelled() => {
                tracing::info!("DB retention job shutting down");
                return;
            }
        }
    }
}

/// Run one iteration of the retention policy.
///
/// Deletes old data in the same order as Java's `DbRetention`:
/// 1. jobs
/// 2. job_versions (protects current versions)
/// 3. runs
/// 4. datasets (protects datasets used as I/O in recent job versions)
/// 5. dataset_versions (protects current versions and versions used as run inputs)
/// 6. lineage_events
pub async fn run_retention(pool: &PgPool, config: &RetentionConfig) -> Result<(), sqlx::Error> {
    let days = config.retention_days;
    let batch = config.batch_size;

    // 1. Jobs
    let deleted = delete_batch_loop(
        pool,
        r#"
        WITH deleted_rows AS (
            DELETE FROM jobs
            WHERE uuid IN (
                SELECT uuid FROM jobs
                WHERE updated_at < CURRENT_TIMESTAMP - make_interval(days => $1)
                FOR UPDATE SKIP LOCKED
                LIMIT $2
            ) RETURNING uuid
        )
        SELECT COUNT(*) FROM deleted_rows
        "#,
        days,
        batch,
    )
    .await?;
    tracing::info!("Retention: deleted {} old jobs", deleted);

    // 2. Job versions (protect current versions of recent jobs)
    let deleted = delete_job_versions(pool, days, batch).await?;
    tracing::info!("Retention: deleted {} old job versions", deleted);

    // 3. Runs
    let deleted = delete_batch_loop(
        pool,
        r#"
        WITH deleted_rows AS (
            DELETE FROM runs
            WHERE uuid IN (
                SELECT uuid FROM runs
                WHERE updated_at < CURRENT_TIMESTAMP - make_interval(days => $1)
                FOR UPDATE SKIP LOCKED
                LIMIT $2
            ) RETURNING uuid
        )
        SELECT COUNT(*) FROM deleted_rows
        "#,
        days,
        batch,
    )
    .await?;
    tracing::info!("Retention: deleted {} old runs", deleted);

    // 4. Datasets (protect datasets used as I/O in recent job versions)
    let deleted = delete_datasets(pool, days, batch).await?;
    tracing::info!("Retention: deleted {} old datasets", deleted);

    // 5. Dataset versions (protect current versions and run inputs)
    let deleted = delete_dataset_versions(pool, days, batch).await?;
    tracing::info!("Retention: deleted {} old dataset versions", deleted);

    // 6. Lineage events (use ctid since run_uuid column was dropped in V36)
    let deleted = delete_batch_loop(
        pool,
        r#"
        WITH deleted_rows AS (
            DELETE FROM lineage_events
            WHERE ctid IN (
                SELECT ctid FROM lineage_events
                WHERE event_time < CURRENT_TIMESTAMP - make_interval(days => $1)
                FOR UPDATE SKIP LOCKED
                LIMIT $2
            ) RETURNING created_at
        )
        SELECT COUNT(*) FROM deleted_rows
        "#,
        days,
        batch,
    )
    .await?;
    tracing::info!("Retention: deleted {} old lineage events", deleted);

    Ok(())
}

/// Delete old job versions, protecting current versions of recent jobs.
async fn delete_job_versions(pool: &PgPool, days: i32, batch: i64) -> Result<i64, sqlx::Error> {
    let mut total = 0i64;
    loop {
        let deleted: (i64,) = sqlx::query_as(
            r#"
            WITH protected AS (
                SELECT current_version_uuid
                FROM jobs
                WHERE updated_at >= CURRENT_TIMESTAMP - make_interval(days => $1)
            ),
            deleted_rows AS (
                DELETE FROM job_versions AS jv
                WHERE jv.uuid IN (
                    SELECT uuid FROM job_versions
                    WHERE created_at < CURRENT_TIMESTAMP - make_interval(days => $1)
                    FOR UPDATE SKIP LOCKED
                    LIMIT $2
                ) AND NOT EXISTS (
                    SELECT 1 FROM protected p
                    WHERE jv.uuid = p.current_version_uuid
                ) RETURNING uuid
            )
            SELECT COUNT(*) FROM deleted_rows
            "#,
        )
        .bind(days)
        .bind(batch)
        .fetch_one(pool)
        .await?;

        total += deleted.0;
        if deleted.0 == 0 {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    Ok(total)
}

/// Delete old datasets, protecting datasets used as I/O in recent job versions.
async fn delete_datasets(pool: &PgPool, days: i32, batch: i64) -> Result<i64, sqlx::Error> {
    let mut total = 0i64;
    loop {
        let deleted: (i64,) = sqlx::query_as(
            r#"
            WITH protected AS (
                SELECT dataset_uuid
                FROM job_versions_io_mapping AS jvio
                INNER JOIN job_versions AS jv ON jvio.job_version_uuid = jv.uuid
                WHERE jv.created_at >= CURRENT_TIMESTAMP - make_interval(days => $1)
            ),
            deleted_rows AS (
                DELETE FROM datasets AS d
                WHERE d.uuid IN (
                    SELECT uuid FROM datasets
                    WHERE updated_at < CURRENT_TIMESTAMP - make_interval(days => $1)
                    FOR UPDATE SKIP LOCKED
                    LIMIT $2
                ) AND NOT EXISTS (
                    SELECT 1 FROM protected p
                    WHERE d.uuid = p.dataset_uuid
                ) RETURNING uuid
            )
            SELECT COUNT(*) FROM deleted_rows
            "#,
        )
        .bind(days)
        .bind(batch)
        .fetch_one(pool)
        .await?;

        total += deleted.0;
        if deleted.0 == 0 {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    Ok(total)
}

/// Delete old dataset versions, protecting current versions and versions used as run inputs.
async fn delete_dataset_versions(pool: &PgPool, days: i32, batch: i64) -> Result<i64, sqlx::Error> {
    let mut total = 0i64;
    loop {
        let deleted: (i64,) = sqlx::query_as(
            r#"
            WITH protected_as_input AS (
                SELECT dataset_version_uuid
                FROM runs_input_mapping AS ri
                INNER JOIN runs AS r ON ri.run_uuid = r.uuid
                WHERE r.created_at >= CURRENT_TIMESTAMP - make_interval(days => $1)
            ),
            protected_as_current AS (
                SELECT current_version_uuid
                FROM datasets
                WHERE updated_at >= CURRENT_TIMESTAMP - make_interval(days => $1)
            ),
            deleted_rows AS (
                DELETE FROM dataset_versions AS dv
                WHERE dv.uuid IN (
                    SELECT uuid FROM dataset_versions
                    WHERE created_at < CURRENT_TIMESTAMP - make_interval(days => $1)
                    FOR UPDATE SKIP LOCKED
                    LIMIT $2
                ) AND NOT EXISTS (
                    SELECT 1 FROM protected_as_input pi
                    WHERE dv.uuid = pi.dataset_version_uuid
                ) AND NOT EXISTS (
                    SELECT 1 FROM protected_as_current pc
                    WHERE dv.uuid = pc.current_version_uuid
                ) RETURNING uuid
            )
            SELECT COUNT(*) FROM deleted_rows
            "#,
        )
        .bind(days)
        .bind(batch)
        .fetch_one(pool)
        .await?;

        total += deleted.0;
        if deleted.0 == 0 {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    Ok(total)
}

/// Generic batch-delete loop for simple tables (no protection logic).
async fn delete_batch_loop(
    pool: &PgPool,
    sql: &str,
    retention_days: i32,
    batch_size: i64,
) -> Result<i64, sqlx::Error> {
    let mut total = 0i64;
    loop {
        let deleted: (i64,) = sqlx::query_as(sql)
            .bind(retention_days)
            .bind(batch_size)
            .fetch_one(pool)
            .await?;

        total += deleted.0;
        if deleted.0 == 0 {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    Ok(total)
}
