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

/// V77 repairs runs whose denormalized job_name kept the raw event name while
/// the job was stored under its parent-qualified canonical name (issue #21).
#[tokio::test]
async fn test_v77_backfills_run_job_name_for_parented_jobs() {
    use chrono::Utc;
    use marquez_api::db::{job, namespace, run};
    use uuid::Uuid;

    let (pool, _container) = setup_test_db().await;
    let runner = MigrationRunner::new(migrations_dir());
    runner.run_all(&pool).await.expect("migrations apply");

    let now = Utc::now();
    let ns = namespace::upsert(&pool, Uuid::new_v4(), now, "v77_ns", "anonymous", None)
        .await
        .expect("namespace");

    let parent = job::upsert(
        &pool,
        Uuid::new_v4(),
        "BATCH",
        now,
        ns.uuid,
        &ns.name,
        "v77_parent",
        None,
        None,
        None,
        Some("v77_parent"),
        None,
        None,
    )
    .await
    .expect("parent job");

    // The jobs_view trigger stores this job under 'v77_parent.v77_child'.
    let child = job::upsert(
        &pool,
        Uuid::new_v4(),
        "BATCH",
        now,
        ns.uuid,
        &ns.name,
        "v77_child",
        None,
        None,
        None,
        Some("v77_child"),
        Some(parent.uuid),
        None,
    )
    .await
    .expect("child job");
    assert_eq!(child.name, "v77_parent.v77_child");

    // Recreate the pre-V77 divergence: the run kept the raw event name.
    let run_uuid = Uuid::new_v4();
    run::upsert(
        &pool,
        run_uuid,
        now,
        Some(child.uuid),
        None,
        None,
        None,
        None,
        None,
        Some("COMPLETED"),
        None,
        None,
        None,
        None,
        &ns.name,
        "v77_child",
        None,
        None,
    )
    .await
    .expect("run");

    let sql = std::fs::read_to_string(
        migrations_dir().join("V77__fix_runs_job_name_for_parented_jobs.sql"),
    )
    .expect("read V77 migration");
    sqlx::raw_sql(&sql).execute(&pool).await.expect("apply V77");

    let (job_name,): (String,) = sqlx::query_as("SELECT job_name FROM runs WHERE uuid = $1")
        .bind(run_uuid)
        .fetch_one(&pool)
        .await
        .expect("read run");
    assert_eq!(job_name, "v77_parent.v77_child");
}
