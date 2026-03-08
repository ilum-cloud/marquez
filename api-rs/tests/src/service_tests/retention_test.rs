#[cfg(test)]
mod tests {
    use crate::common::TestDb;
    use crate::fixtures;
    use marquez_api::jobs::db_retention::{run_retention, RetentionConfig};

    #[tokio::test]
    async fn retention_deletes_old_jobs() {
        let db = TestDb::new().await;
        let pool = &db.pool;

        // Create a namespace first (required by FK constraint)
        let ns = fixtures::create_namespace(pool).await;

        // Insert a job with updated_at set to 30 days ago
        let job_uuid = uuid::Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO jobs (uuid, type, name, simple_name, namespace_uuid, namespace_name,
                              created_at, updated_at, current_version_uuid)
            VALUES ($1, 'BATCH', 'old_job', 'old_job', $2, $3,
                    CURRENT_TIMESTAMP - INTERVAL '30 days',
                    CURRENT_TIMESTAMP - INTERVAL '30 days',
                    NULL)
            "#,
        )
        .bind(job_uuid)
        .bind(ns.uuid)
        .bind(&ns.name)
        .execute(pool)
        .await
        .expect("insert old job");

        // Verify job exists
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE uuid = $1")
            .bind(job_uuid)
            .fetch_one(pool)
            .await
            .expect("count jobs");
        assert_eq!(count.0, 1);

        // Run retention with 7-day policy
        let config = RetentionConfig {
            frequency_mins: 0,
            batch_size: 1000,
            retention_days: 7,
        };
        run_retention(pool, &config).await.expect("run retention");

        // Verify job was deleted
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE uuid = $1")
            .bind(job_uuid)
            .fetch_one(pool)
            .await
            .expect("count jobs after retention");
        assert_eq!(count.0, 0, "Old job should have been deleted by retention");
    }

    #[tokio::test]
    async fn retention_preserves_recent_jobs() {
        let db = TestDb::new().await;
        let pool = &db.pool;

        // Create a namespace
        let ns = fixtures::create_namespace(pool).await;

        // Insert a recent job (created today)
        let job_uuid = uuid::Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO jobs (uuid, type, name, simple_name, namespace_uuid, namespace_name,
                              created_at, updated_at, current_version_uuid)
            VALUES ($1, 'BATCH', 'recent_job', 'recent_job', $2, $3,
                    CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, NULL)
            "#,
        )
        .bind(job_uuid)
        .bind(ns.uuid)
        .bind(&ns.name)
        .execute(pool)
        .await
        .expect("insert recent job");

        // Run retention
        let config = RetentionConfig {
            frequency_mins: 0,
            batch_size: 1000,
            retention_days: 7,
        };
        run_retention(pool, &config).await.expect("run retention");

        // Verify job was preserved
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE uuid = $1")
            .bind(job_uuid)
            .fetch_one(pool)
            .await
            .expect("count jobs after retention");
        assert_eq!(
            count.0, 1,
            "Recent job should have been preserved by retention"
        );
    }

    #[tokio::test]
    async fn retention_batch_loop_deletes_all() {
        let db = TestDb::new().await;
        let pool = &db.pool;

        // Create a namespace
        let ns = fixtures::create_namespace(pool).await;

        // Insert 5 old jobs (more than batch size of 2)
        for i in 0..5 {
            let job_uuid = uuid::Uuid::new_v4();
            let name = format!("batch_old_job_{}", i);
            sqlx::query(
                r#"
                INSERT INTO jobs (uuid, type, name, simple_name, namespace_uuid, namespace_name,
                                  created_at, updated_at, current_version_uuid)
                VALUES ($1, 'BATCH', $2, $3, $4, $5,
                        CURRENT_TIMESTAMP - INTERVAL '30 days',
                        CURRENT_TIMESTAMP - INTERVAL '30 days',
                        NULL)
                "#,
            )
            .bind(job_uuid)
            .bind(&name)
            .bind(&name)
            .bind(ns.uuid)
            .bind(&ns.name)
            .execute(pool)
            .await
            .expect("insert old job");
        }

        // Run retention with small batch size to test batch loop
        let config = RetentionConfig {
            frequency_mins: 0,
            batch_size: 2,
            retention_days: 7,
        };
        run_retention(pool, &config).await.expect("run retention");

        // Verify all old jobs were deleted (batch loop should have iterated multiple times)
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM jobs WHERE name LIKE 'batch_old_job_%' AND namespace_uuid = $1",
        )
        .bind(ns.uuid)
        .fetch_one(pool)
        .await
        .expect("count old jobs");
        assert_eq!(
            count.0, 0,
            "All old jobs should have been deleted across multiple batches"
        );
    }
}
