use marquez_api::db::migration::MigrationRunner;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn setup_test_db() -> (PgPool, testcontainers::ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .start()
        .await
        .expect("Failed to start postgres container");

    let host = container.get_host().await.expect("get host");
    let port = container.get_host_port_ipv4(5432).await.expect("get port");

    let url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("Failed to connect to test db");

    (pool, container)
}

fn migrations_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("migrations")
}

#[tokio::test]
async fn test_all_migrations_apply_cleanly() {
    let (pool, _container) = setup_test_db().await;
    let runner = MigrationRunner::new(migrations_dir());

    runner
        .run_all(&pool)
        .await
        .expect("all migrations should apply without errors");
}

#[tokio::test]
async fn test_key_tables_exist() {
    let (pool, _container) = setup_test_db().await;
    let runner = MigrationRunner::new(migrations_dir());
    runner.run_all(&pool).await.expect("migrations failed");

    let expected_tables = [
        "namespaces",
        "sources",
        "datasets",
        "dataset_versions",
        "dataset_fields",
        "jobs",
        "job_versions",
        "runs",
        "run_states",
        "lineage_events",
        "column_lineage",
    ];

    for table in &expected_tables {
        let exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'public' AND table_name = $1
            )",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .expect("query failed");

        assert!(exists.0, "table '{}' should exist after migrations", table);
    }
}

#[tokio::test]
async fn test_custom_types_exist() {
    let (pool, _container) = setup_test_db().await;
    let runner = MigrationRunner::new(migrations_dir());
    runner.run_all(&pool).await.expect("migrations failed");

    let exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM pg_type WHERE typname = 'dataset_name'
        )",
    )
    .fetch_one(&pool)
    .await
    .expect("query failed");

    assert!(exists.0, "DATASET_NAME composite type should exist");
}

#[tokio::test]
async fn test_triggers_exist() {
    let (pool, _container) = setup_test_db().await;
    let runner = MigrationRunner::new(migrations_dir());
    runner.run_all(&pool).await.expect("migrations failed");

    // The rewrite_jobs_fqn_table trigger is on jobs_view (an INSTEAD OF trigger on a view),
    // so we check pg_trigger joined with pg_class.
    let exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM pg_trigger t
            JOIN pg_class c ON t.tgrelid = c.oid
            WHERE t.tgname = 'update_symlinks'
              AND c.relname = 'jobs_view'
        )",
    )
    .fetch_one(&pool)
    .await
    .expect("query failed");

    assert!(
        exists.0,
        "update_symlinks trigger on jobs_view should exist"
    );
}

#[tokio::test]
async fn test_materialized_views_exist() {
    let (pool, _container) = setup_test_db().await;
    let runner = MigrationRunner::new(migrations_dir());
    runner.run_all(&pool).await.expect("migrations failed");

    let exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM pg_matviews
            WHERE matviewname = 'lineage_events_by_type_hourly_view'
        )",
    )
    .fetch_one(&pool)
    .await
    .expect("query failed");

    assert!(
        exists.0,
        "lineage_events_by_type_hourly_view materialized view should exist"
    );
}

#[tokio::test]
async fn test_repeatable_migrations_idempotent() {
    let (pool, _container) = setup_test_db().await;
    let runner = MigrationRunner::new(migrations_dir());

    runner
        .run_all(&pool)
        .await
        .expect("first migration run should succeed");

    runner
        .run_all(&pool)
        .await
        .expect("second migration run should succeed (repeatable migrations are idempotent)");
}
