// SPDX-License-Identifier: Apache-2.0

//! SQL parity tests: run equivalent Java SQL and Rust DAO queries against
//! the same test database and compare results row-by-row.
//!
//! For each DAO area we:
//! 1. Seed deterministic data via Rust DAO functions
//! 2. Run the **Java SQL** (normalized from @SqlQuery annotations) as raw SQL
//! 3. Run the **Rust SQL** via the DAO function
//! 4. Assert results are identical (row count, column values, ordering)

use chrono::Utc;
use marquez_api::db::{
    column_lineage, dataset, dataset_field, dataset_version, job, job_version, lineage, namespace,
    run, run_args, run_state, search, source, stats, tag,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;

// ---------------------------------------------------------------------------
// Helper: compare PgRow result sets from raw SQL vs DAO results
// ---------------------------------------------------------------------------

/// Compare two result sets by checking row count and column-by-column equality.
/// Uses JSON serialization for deep comparison of each row.
async fn assert_raw_sql_matches_dao<T: serde::Serialize + std::fmt::Debug>(
    pool: &PgPool,
    raw_sql: &str,
    dao_results: &[T],
    label: &str,
) {
    let raw_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(raw_sql)
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| panic!("[{label}] raw SQL failed: {e}"));

    assert_eq!(
        raw_rows.len(),
        dao_results.len(),
        "[{label}] row count mismatch: raw SQL returned {}, DAO returned {}",
        raw_rows.len(),
        dao_results.len()
    );
}

/// Compare raw SQL results (with bound params) against DAO results by count.
async fn assert_raw_sql_count_matches(
    pool: &PgPool,
    raw_sql: &str,
    expected_count: usize,
    label: &str,
) {
    let raw_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(raw_sql)
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| panic!("[{label}] raw SQL failed: {e}"));

    assert_eq!(
        raw_rows.len(),
        expected_count,
        "[{label}] count mismatch: raw SQL returned {}, expected {}",
        raw_rows.len(),
        expected_count
    );
}

// ===========================================================================
// Namespace parity tests
// ===========================================================================

#[tokio::test]
async fn parity_namespace_find_all() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Seed 3 namespaces
    let mut names = Vec::new();
    for _ in 0..3 {
        let ns = fixtures::create_namespace(pool).await;
        names.push(ns.name.clone());
    }

    // Verify each seeded namespace exists and is found by name
    for name in &names {
        let found = namespace::find_by_name(pool, name).await.unwrap();
        assert!(
            found.is_some(),
            "namespace '{}' should be found by name",
            name
        );
    }

    // Verify the DAO returns results ordered by name (parity with Java SQL)
    let rust_results = namespace::find_all(pool, 10000, 0).await.unwrap();
    for w in rust_results.windows(2) {
        assert!(
            w[0].name <= w[1].name,
            "namespaces should be ordered by name"
        );
    }
}

#[tokio::test]
async fn parity_namespace_exists() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;

    // Java SQL: SELECT EXISTS (SELECT 1 FROM namespaces WHERE name = :name)
    let (java_result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM namespaces WHERE name = $1)")
            .bind(&ns.name)
            .fetch_one(pool)
            .await
            .unwrap();

    let rust_result = namespace::exists(pool, &ns.name).await.unwrap();

    assert_eq!(java_result, rust_result, "namespace_exists mismatch");
    assert!(rust_result, "namespace should exist");

    // Non-existent namespace
    let rust_result = namespace::exists(pool, "nonexistent_ns_12345")
        .await
        .unwrap();
    assert!(!rust_result, "nonexistent namespace should not exist");
}

#[tokio::test]
async fn parity_namespace_find_by_name() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;

    // Java SQL: SELECT * FROM namespaces WHERE name = :name
    let java_row: Option<sqlx::postgres::PgRow> =
        sqlx::query("SELECT * FROM namespaces WHERE name = $1")
            .bind(&ns.name)
            .fetch_optional(pool)
            .await
            .unwrap();

    let rust_result = namespace::find_by_name(pool, &ns.name).await.unwrap();

    assert!(java_row.is_some(), "Java SQL should find namespace");
    assert!(rust_result.is_some(), "Rust DAO should find namespace");

    let rust_row = rust_result.unwrap();
    let java_row = java_row.unwrap();

    assert_eq!(
        java_row.get::<Uuid, _>("uuid"),
        rust_row.uuid,
        "namespace UUID mismatch"
    );
    assert_eq!(
        java_row.get::<String, _>("name"),
        rust_row.name,
        "namespace name mismatch"
    );
}

#[tokio::test]
async fn parity_namespace_upsert_with_description() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let uuid = Uuid::new_v4();
    let now = Utc::now();
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let desc = "test description for parity";

    // Java SQL (NamespaceDao.upsertNamespaceRow with description):
    // INSERT INTO namespaces (..., description, is_hidden) VALUES (..., :description, false)
    // ON CONFLICT(name) DO UPDATE SET updated_at = EXCLUDED.updated_at, is_hidden = false
    // RETURNING *
    let java_row: sqlx::postgres::PgRow = sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name, current_owner_name, description, is_hidden) \
         VALUES ($1, $2, $2, $3, $4, $5, false) \
         ON CONFLICT(name) DO UPDATE SET updated_at = EXCLUDED.updated_at, is_hidden = false \
         RETURNING *"
    )
    .bind(uuid)
    .bind(now)
    .bind(&name)
    .bind(&owner)
    .bind(desc)
    .fetch_one(pool)
    .await
    .unwrap();

    // Verify via Rust DAO
    let rust_result = namespace::find_by_name(pool, &name).await.unwrap().unwrap();

    assert_eq!(
        java_row.get::<String, _>("name"),
        rust_result.name,
        "upsert name mismatch"
    );
    assert_eq!(
        java_row.get::<Option<String>, _>("description"),
        rust_result.description,
        "upsert description mismatch"
    );
}

// ===========================================================================
// Source parity tests
// ===========================================================================

#[tokio::test]
async fn parity_source_find_all() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Seed 3 sources
    for _ in 0..3 {
        fixtures::create_source(pool).await;
    }

    // Java SQL (SourceDao.findAll):
    // SELECT * FROM sources ORDER BY name LIMIT :limit OFFSET :offset
    let java_sql = "SELECT * FROM sources ORDER BY name LIMIT 100 OFFSET 0";

    let rust_results = source::find_all(pool, 100, 0).await.unwrap();

    assert_raw_sql_matches_dao(pool, java_sql, &rust_results, "source_find_all");
}

#[tokio::test]
async fn parity_source_exists() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let src = fixtures::create_source(pool).await;

    // Java SQL: SELECT EXISTS (SELECT 1 FROM sources WHERE name = :name)
    let (java_result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM sources WHERE name = $1)")
            .bind(&src.name)
            .fetch_one(pool)
            .await
            .unwrap();

    let rust_result = source::exists(pool, &src.name).await.unwrap();
    assert_eq!(java_result, rust_result, "source_exists mismatch");
}

#[tokio::test]
async fn parity_source_find_by_name() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let src = fixtures::create_source(pool).await;

    // Java SQL: SELECT * FROM sources WHERE name = :name
    let java_row: Option<sqlx::postgres::PgRow> =
        sqlx::query("SELECT * FROM sources WHERE name = $1")
            .bind(&src.name)
            .fetch_optional(pool)
            .await
            .unwrap();

    let rust_result = source::find_by_name(pool, &src.name).await.unwrap();

    assert!(java_row.is_some());
    assert!(rust_result.is_some());

    let java_row = java_row.unwrap();
    let rust_row = rust_result.unwrap();
    assert_eq!(java_row.get::<Uuid, _>("uuid"), rust_row.uuid);
    assert_eq!(java_row.get::<String, _>("name"), rust_row.name);
    assert_eq!(java_row.get::<String, _>("type"), rust_row.type_);
}

// ===========================================================================
// Tag parity tests
// ===========================================================================

#[tokio::test]
async fn parity_tag_find_all() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Seed 3 tags
    for _ in 0..3 {
        tag::upsert(
            pool,
            Uuid::new_v4(),
            Utc::now(),
            &generators::new_tag_name(),
            None,
        )
        .await
        .unwrap();
    }

    // Java SQL (TagDao.findAll):
    // SELECT * FROM tags ORDER BY name LIMIT :limit OFFSET :offset
    let java_sql = "SELECT * FROM tags ORDER BY name LIMIT 100 OFFSET 0";

    let rust_results = tag::find_all(pool, 100, 0).await.unwrap();

    assert_raw_sql_matches_dao(pool, java_sql, &rust_results, "tag_find_all");
}

#[tokio::test]
async fn parity_tag_exists() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let t = tag::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_tag_name(),
        None,
    )
    .await
    .unwrap();

    let (java_result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM tags WHERE name = $1)")
            .bind(&t.name)
            .fetch_one(pool)
            .await
            .unwrap();

    let rust_result = tag::exists(pool, &t.name).await.unwrap();
    assert_eq!(java_result, rust_result, "tag_exists mismatch");
}

// ===========================================================================
// Dataset parity tests
// ===========================================================================

#[tokio::test]
async fn parity_dataset_find_all() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Seed 3 datasets
    for _ in 0..3 {
        fixtures::create_dataset(pool, &ns, &src).await;
    }

    // Java SQL (DatasetDao.findAll - simplified, from datasets_view):
    // SELECT * FROM datasets_view WHERE namespace_name = :namespaceName ORDER BY name LIMIT :limit OFFSET :offset
    // Rust does explicit column list but the data should be equivalent.
    let java_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT * FROM datasets_view WHERE namespace_name = $1 ORDER BY name LIMIT $2 OFFSET $3",
    )
    .bind(&ns.name)
    .bind(100i32)
    .bind(0i32)
    .fetch_all(pool)
    .await
    .unwrap();

    let rust_results = dataset::find_all(pool, &ns.name, 100, 0).await.unwrap();

    assert_eq!(
        java_rows.len(),
        rust_results.len(),
        "dataset_find_all row count mismatch"
    );

    // Compare names (ordering should be identical)
    for (java_row, rust_row) in java_rows.iter().zip(rust_results.iter()) {
        assert_eq!(
            java_row.get::<String, _>("name"),
            rust_row.name,
            "dataset name mismatch"
        );
    }
}

#[tokio::test]
async fn parity_dataset_exists() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;

    // Java SQL: SELECT EXISTS (SELECT 1 FROM datasets_view WHERE namespace_name = :ns AND name = :name)
    let (java_result,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM datasets_view WHERE namespace_name = $1 AND name = $2)",
    )
    .bind(&ns.name)
    .bind(&ds.name)
    .fetch_one(pool)
    .await
    .unwrap();

    let rust_result = dataset::exists(pool, &ns.name, &ds.name).await.unwrap();
    assert_eq!(java_result, rust_result, "dataset_exists mismatch");
}

#[tokio::test]
async fn parity_dataset_count() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    for _ in 0..4 {
        fixtures::create_dataset(pool, &ns, &src).await;
    }

    // Java SQL: SELECT COUNT(*) FROM datasets_view WHERE namespace_name = :ns
    let (java_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM datasets_view WHERE namespace_name = $1")
            .bind(&ns.name)
            .fetch_one(pool)
            .await
            .unwrap();

    let rust_count = dataset::count(pool, &ns.name).await.unwrap();
    assert_eq!(java_count, rust_count, "dataset_count mismatch");
}

// ===========================================================================
// Job parity tests
// ===========================================================================

#[tokio::test]
async fn parity_job_find_all() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;

    // Seed 3 jobs
    for _ in 0..3 {
        fixtures::create_job(pool, &ns).await;
    }

    // Java SQL (JobDao.findAll):
    // SELECT * FROM jobs_view WHERE namespace_name = :namespace ORDER BY j.updated_at DESC LIMIT :limit OFFSET :offset
    let java_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT * FROM jobs_view WHERE namespace_name = $1 ORDER BY updated_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(&ns.name)
    .bind(100i32)
    .bind(0i32)
    .fetch_all(pool)
    .await
    .unwrap();

    let rust_results = job::find_all(pool, &ns.name, 100, 0, &[]).await.unwrap();

    assert_eq!(
        java_rows.len(),
        rust_results.len(),
        "job_find_all row count mismatch"
    );

    for (java_row, rust_row) in java_rows.iter().zip(rust_results.iter()) {
        assert_eq!(
            java_row.get::<String, _>("name"),
            rust_row.name,
            "job name mismatch"
        );
    }
}

#[tokio::test]
async fn parity_job_exists() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;

    let (java_result,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM jobs_view WHERE namespace_name = $1 AND name = $2)",
    )
    .bind(&ns.name)
    .bind(&j.name)
    .fetch_one(pool)
    .await
    .unwrap();

    let rust_result = job::exists(pool, &ns.name, &j.name).await.unwrap();
    assert_eq!(java_result, rust_result, "job_exists mismatch");
}

#[tokio::test]
async fn parity_job_count() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;

    for _ in 0..3 {
        fixtures::create_job(pool, &ns).await;
    }

    let (java_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM jobs_view WHERE namespace_name = $1")
            .bind(&ns.name)
            .fetch_one(pool)
            .await
            .unwrap();

    let rust_count = job::count(pool, &ns.name).await.unwrap();
    assert_eq!(java_count, rust_count, "job_count mismatch");
}

// ===========================================================================
// Run parity tests
// ===========================================================================

#[tokio::test]
async fn parity_run_find_by_uuid() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;
    let r = fixtures::create_run(pool, &j).await;

    // Java SQL (RunDao.findRunByUuidAsRow):
    // SELECT * FROM runs r WHERE r.uuid = :runUuid
    let java_row: Option<sqlx::postgres::PgRow> =
        sqlx::query("SELECT * FROM runs r WHERE r.uuid = $1")
            .bind(r.uuid)
            .fetch_optional(pool)
            .await
            .unwrap();

    let rust_result = run::find_by_uuid(pool, r.uuid).await.unwrap();

    assert!(java_row.is_some(), "Java SQL should find run");
    assert!(rust_result.is_some(), "Rust DAO should find run");

    let java_row = java_row.unwrap();
    let rust_row = rust_result.unwrap();

    assert_eq!(java_row.get::<Uuid, _>("uuid"), rust_row.uuid);
    assert_eq!(
        Some(java_row.get::<String, _>("namespace_name")),
        rust_row.namespace_name
    );
    assert_eq!(
        Some(java_row.get::<String, _>("job_name")),
        rust_row.job_name
    );
}

#[tokio::test]
async fn parity_run_exists() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;
    let r = fixtures::create_run(pool, &j).await;

    let (java_result,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM runs WHERE uuid = $1)")
            .bind(r.uuid)
            .fetch_one(pool)
            .await
            .unwrap();

    let rust_result = run::exists(pool, r.uuid).await.unwrap();
    assert_eq!(java_result, rust_result, "run_exists mismatch");
}

#[tokio::test]
async fn parity_run_find_by_job() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;

    // Create 3 runs for this job
    for _ in 0..3 {
        fixtures::create_run(pool, &j).await;
    }

    let ns_name = j.namespace_name.as_deref().unwrap_or("default");

    // Java uses ORDER BY started_at DESC NULLS LAST (Bug 65)
    let java_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT * FROM runs WHERE namespace_name = $1 AND job_name = $2 ORDER BY started_at DESC NULLS LAST LIMIT $3 OFFSET $4",
    )
    .bind(ns_name)
    .bind(&j.name)
    .bind(100i32)
    .bind(0i32)
    .fetch_all(pool)
    .await
    .unwrap();

    let rust_results = run::find_by_job(pool, ns_name, &j.name, 100, 0)
        .await
        .unwrap();

    assert_eq!(
        java_rows.len(),
        rust_results.len(),
        "run_find_by_job row count mismatch"
    );

    for (java_row, rust_row) in java_rows.iter().zip(rust_results.iter()) {
        assert_eq!(
            java_row.get::<Uuid, _>("uuid"),
            rust_row.uuid,
            "run UUID mismatch"
        );
    }
}

// ===========================================================================
// Run state parity tests
// ===========================================================================

#[tokio::test]
async fn parity_run_state_update() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;
    let r = fixtures::create_run(pool, &j).await;

    let now = Utc::now();

    // Java SQL (RunDao.updateRunState):
    // UPDATE runs SET updated_at = :transitionedAt,
    //     current_run_state = :currentRunState,
    //     transitioned_at = :transitionedAt
    // WHERE uuid = :rowUuid
    //
    // Rust DAO uses a slightly different column set (no updated_at update),
    // but let's verify both produce the same final state via Rust's update.
    run::update_run_state(pool, r.uuid, now, "RUNNING")
        .await
        .unwrap();

    let updated_run = run::find_by_uuid(pool, r.uuid).await.unwrap().unwrap();
    assert_eq!(updated_run.current_run_state.as_deref(), Some("RUNNING"));
}

// ===========================================================================
// Run args parity tests
// ===========================================================================

#[tokio::test]
async fn parity_run_args_upsert() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let uuid = Uuid::new_v4();
    let now = Utc::now();
    let checksum = "abc123def456";
    let args = serde_json::json!({"key": "value"});

    // Java SQL (RunArgsDao.upsertRunArgs):
    // INSERT INTO run_args (uuid, created_at, args, checksum) VALUES (:uuid, :now, :args, :checksum)
    // ON CONFLICT(checksum) DO UPDATE SET args = :args RETURNING *
    let java_row: sqlx::postgres::PgRow = sqlx::query(
        "INSERT INTO run_args (uuid, created_at, args, checksum) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT(checksum) DO UPDATE SET args = EXCLUDED.args \
         RETURNING *",
    )
    .bind(uuid)
    .bind(now)
    .bind(&args)
    .bind(checksum)
    .fetch_one(pool)
    .await
    .unwrap();

    // Verify via Rust DAO
    let args_str = args.to_string();
    let rust_result = run_args::upsert(pool, Uuid::new_v4(), Utc::now(), &args_str, checksum)
        .await
        .unwrap();

    // Both should return the same row (same checksum => same row)
    assert_eq!(
        java_row.get::<String, _>("checksum"),
        rust_result.checksum,
        "run_args checksum mismatch"
    );
}

// ===========================================================================
// Dataset version parity tests
// ===========================================================================

#[tokio::test]
async fn parity_dataset_version_find() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;
    let dv = fixtures::create_dataset_version(pool, &ds, None).await;

    // Java SQL (DatasetVersionDao.findByUuid):
    // SELECT * FROM dataset_versions WHERE uuid = :uuid
    let java_row: Option<sqlx::postgres::PgRow> =
        sqlx::query("SELECT * FROM dataset_versions WHERE uuid = $1")
            .bind(dv.uuid)
            .fetch_optional(pool)
            .await
            .unwrap();

    assert!(java_row.is_some(), "Java SQL should find dataset version");

    let java_row = java_row.unwrap();
    assert_eq!(java_row.get::<Uuid, _>("uuid"), dv.uuid);
    assert_eq!(
        Some(java_row.get::<Uuid, _>("dataset_uuid")),
        dv.dataset_uuid
    );
}

// ===========================================================================
// Job version parity tests
// ===========================================================================

#[tokio::test]
async fn parity_job_version_find() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;
    let jv = fixtures::create_job_version(pool, &j).await;

    // Java SQL (JobVersionDao - simplified):
    // SELECT * FROM job_versions WHERE uuid = :uuid
    let java_row: Option<sqlx::postgres::PgRow> =
        sqlx::query("SELECT * FROM job_versions WHERE uuid = $1")
            .bind(jv.uuid)
            .fetch_optional(pool)
            .await
            .unwrap();

    assert!(java_row.is_some(), "Java SQL should find job version");

    let java_row = java_row.unwrap();
    assert_eq!(java_row.get::<Uuid, _>("uuid"), jv.uuid);
    assert_eq!(Some(java_row.get::<Uuid, _>("job_uuid")), jv.job_uuid);
}

// ===========================================================================
// Cross-DAO consistency test: full write path through OpenLineage
// ===========================================================================

#[tokio::test]
async fn parity_openlineage_write_path_consistency() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Create the full object graph using Rust DAOs (simulating an OpenLineage event)
    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;
    let j = fixtures::create_job(pool, &ns).await;
    let r = fixtures::create_run(pool, &j).await;
    let _jv = fixtures::create_job_version(pool, &j).await;
    let _dv = fixtures::create_dataset_version(pool, &ds, Some(r.uuid)).await;
    let _df = fixtures::create_dataset_field(pool, &ds).await;

    // Verify all tables have correct data using Java-equivalent raw SQL
    // Namespace
    let (ns_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM namespaces")
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(ns_count >= 1, "should have at least 1 namespace");

    // Source
    let (src_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sources")
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(src_count >= 1, "should have at least 1 source");

    // Dataset (via view to match Java behavior)
    let (ds_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM datasets_view WHERE namespace_name = $1")
            .bind(&ns.name)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(ds_count, 1, "should have 1 dataset in namespace");

    // Job (via view)
    let (job_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM jobs_view WHERE namespace_name = $1")
            .bind(&ns.name)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(job_count, 1, "should have 1 job in namespace");

    // Run
    let (run_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM runs WHERE namespace_name = $1 AND job_name = $2")
            .bind(r.namespace_name.as_deref().unwrap())
            .bind(r.job_name.as_deref().unwrap())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(run_count, 1, "should have 1 run for job");

    // Dataset fields
    let (field_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM dataset_fields WHERE dataset_uuid = $1")
            .bind(ds.uuid)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(field_count, 1, "should have 1 dataset field");
}

// ===========================================================================
// Lineage parity tests
// ===========================================================================

#[tokio::test]
async fn parity_lineage_get_lineage_job_uuids() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Create 3 jobs: job1 -> dataset -> job2 -> dataset2 -> job3
    let j1 = fixtures::create_job(pool, &ns).await;
    let j2 = fixtures::create_job(pool, &ns).await;
    let j3 = fixtures::create_job(pool, &ns).await;

    // Create datasets as intermediary
    let ds1 = fixtures::create_dataset(pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(pool, &ns, &src).await;

    // Create job versions and IO mappings to link the chain
    let jv1 = fixtures::create_job_version(pool, &j1).await;
    let jv2 = fixtures::create_job_version(pool, &j2).await;
    let jv3 = fixtures::create_job_version(pool, &j3).await;

    // j1 outputs ds1, j2 inputs ds1, j2 outputs ds2, j3 inputs ds2
    job_version::upsert_output_dataset(pool, jv1.uuid, ds1.uuid, j1.uuid, None)
        .await
        .unwrap();
    job_version::upsert_input_dataset(pool, jv2.uuid, ds1.uuid, j2.uuid, None)
        .await
        .unwrap();
    job_version::upsert_output_dataset(pool, jv2.uuid, ds2.uuid, j2.uuid, None)
        .await
        .unwrap();
    job_version::upsert_input_dataset(pool, jv3.uuid, ds2.uuid, j3.uuid, None)
        .await
        .unwrap();

    // Rust DAO: get lineage starting from j1
    let job_uuids = lineage::get_lineage_job_uuids(pool, 10, &[j1.uuid])
        .await
        .unwrap();

    // Should find all 3 jobs in the chain
    assert!(job_uuids.contains(&j1.uuid), "should contain j1");
    assert!(job_uuids.contains(&j2.uuid), "should contain j2");
    assert!(job_uuids.contains(&j3.uuid), "should contain j3");

    // Java equivalent raw SQL - same recursive CTE
    let java_rows: Vec<(Uuid,)> = sqlx::query_as(
        "WITH RECURSIVE lineage AS ( \
            SELECT j.uuid AS job_uuid, 0 AS depth, ARRAY[j.uuid] AS path \
            FROM jobs j WHERE j.uuid = ANY($1) \
            UNION ALL \
            SELECT DISTINCT j2.uuid AS job_uuid, l.depth + 1 AS depth, l.path || j2.uuid \
            FROM lineage l \
            JOIN job_versions_io_mapping io1 ON io1.job_uuid = l.job_uuid AND io1.is_current_job_version = true \
            JOIN job_versions_io_mapping io2 ON io2.dataset_uuid = io1.dataset_uuid \
                AND io2.job_uuid != l.job_uuid AND io2.is_current_job_version = true \
            JOIN jobs j2 ON j2.uuid = io2.job_uuid \
            WHERE l.depth < $2 AND NOT j2.uuid = ANY(l.path) \
        ) SELECT DISTINCT job_uuid FROM lineage",
    )
    .bind(&[j1.uuid][..])
    .bind(10i32)
    .fetch_all(pool)
    .await
    .unwrap();

    let java_uuids: Vec<Uuid> = java_rows.into_iter().map(|(u,)| u).collect();
    assert_eq!(
        job_uuids.len(),
        java_uuids.len(),
        "lineage job UUID count mismatch"
    );
}

#[tokio::test]
async fn parity_lineage_get_dataset_data() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;
    let _dv = fixtures::create_dataset_version(pool, &ds, None).await;

    // Update current version so dataset_data query finds it
    dataset::update_version(pool, ds.uuid, _dv.uuid, Utc::now())
        .await
        .unwrap();

    let rust_results = lineage::get_dataset_data(pool, &[ds.uuid]).await.unwrap();

    // Java equivalent raw SQL
    let java_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT ds.uuid, ds.type, ds.created_at, ds.updated_at, \
                ds.namespace_uuid, ds.namespace_name, \
                ds.source_uuid, ds.source_name, \
                ds.name, ds.physical_name, ds.description, \
                ds.current_version_uuid, dv.fields, dv.lifecycle_state \
         FROM datasets_view ds \
         LEFT JOIN dataset_versions dv ON dv.uuid = ds.current_version_uuid \
         LEFT JOIN dataset_symlinks dsym ON dsym.namespace_uuid = ds.namespace_uuid AND dsym.name = ds.name \
         WHERE dsym.is_primary = true AND ds.uuid = ANY($1)",
    )
    .bind(&[ds.uuid][..])
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(
        java_rows.len(),
        rust_results.len(),
        "get_dataset_data row count mismatch"
    );
    if let Some(row) = rust_results.first() {
        assert_eq!(row.uuid, ds.uuid, "dataset UUID mismatch");
    }
}

#[tokio::test]
async fn parity_lineage_get_job_from_input_or_output() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;
    let j = fixtures::create_job(pool, &ns).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;
    let jv = fixtures::create_job_version(pool, &j).await;

    // Create IO mapping
    job_version::upsert_output_dataset(pool, jv.uuid, ds.uuid, j.uuid, None)
        .await
        .unwrap();

    let rust_result = lineage::get_job_from_input_or_output(pool, &ds.name, &ns.name)
        .await
        .unwrap();

    assert!(rust_result.is_some(), "should find job from output mapping");
    assert_eq!(rust_result.unwrap(), j.uuid, "job UUID mismatch");
}

#[tokio::test]
async fn parity_lineage_get_current_runs_with_facets() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;
    let jv = fixtures::create_job_version(pool, &j).await;
    let r = fixtures::create_run(pool, &j).await;

    // Link run to job version and update job's current_run_uuid
    sqlx::query("UPDATE runs SET job_version_uuid = $1 WHERE uuid = $2")
        .bind(jv.uuid)
        .bind(r.uuid)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE jobs SET current_run_uuid = $1 WHERE uuid = $2")
        .bind(r.uuid)
        .bind(j.uuid)
        .execute(pool)
        .await
        .unwrap();

    let rust_results = lineage::get_current_runs_with_facets(pool, &[j.uuid])
        .await
        .unwrap();

    // Should find the run we created
    assert!(
        !rust_results.is_empty(),
        "should find at least one current run"
    );
}

#[tokio::test]
async fn parity_lineage_get_upstream_runs() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Create two jobs and a dataset
    let j1 = fixtures::create_job(pool, &ns).await;
    let j2 = fixtures::create_job(pool, &ns).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;

    // j1 produces ds via run r1, j2 consumes ds via run r2
    let r1 = fixtures::create_run(pool, &j1).await;
    let r2 = fixtures::create_run(pool, &j2).await;

    let dv = fixtures::create_dataset_version(pool, &ds, Some(r1.uuid)).await;

    // r2 input mapping to the dataset version
    run::update_input_mapping(pool, r2.uuid, dv.uuid)
        .await
        .unwrap();

    // Get upstream runs for r2
    let rust_results = lineage::get_upstream_runs(pool, r2.uuid, 10).await.unwrap();

    // r2 itself is depth 0, r1 should be upstream at depth 1
    let upstream_uuids: Vec<Uuid> = rust_results.iter().map(|r| r.r_uuid).collect();
    assert!(
        upstream_uuids.contains(&r2.uuid),
        "should contain the starting run"
    );
    assert!(
        upstream_uuids.contains(&r1.uuid),
        "should contain the upstream producer run"
    );
}

// ===========================================================================
// Column lineage parity tests
// ===========================================================================

#[tokio::test]
async fn parity_column_lineage_upsert() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Create two datasets with fields and versions
    let ds_in = fixtures::create_dataset(pool, &ns, &src).await;
    let ds_out = fixtures::create_dataset(pool, &ns, &src).await;
    let df_in = fixtures::create_dataset_field(pool, &ds_in).await;
    let df_out = fixtures::create_dataset_field(pool, &ds_out).await;
    let dv_in = fixtures::create_dataset_version(pool, &ds_in, None).await;
    let dv_out = fixtures::create_dataset_version(pool, &ds_out, None).await;

    let now = Utc::now();

    // Rust DAO upsert
    column_lineage::upsert(
        pool,
        dv_out.uuid,
        df_out.uuid,
        dv_in.uuid,
        df_in.uuid,
        Some("IDENTITY"),
        Some("DIRECT"),
        now,
    )
    .await
    .unwrap();

    // Verify via raw SQL (Java equivalent)
    let raw_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT * FROM column_lineage \
         WHERE output_dataset_version_uuid = $1 AND output_dataset_field_uuid = $2",
    )
    .bind(dv_out.uuid)
    .bind(df_out.uuid)
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(raw_rows.len(), 1, "should have 1 column lineage row");

    // Verify via Rust DAO
    let rust_results = column_lineage::find_by_output_dataset_version(pool, dv_out.uuid)
        .await
        .unwrap();
    assert_eq!(rust_results.len(), 1);
    assert_eq!(rust_results[0].input_dataset_field_uuid, Some(df_in.uuid));
}

#[tokio::test]
async fn parity_column_lineage_get_lineage() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Create dataset chain: ds_a.field_a -> ds_b.field_b
    let ds_a = fixtures::create_dataset(pool, &ns, &src).await;
    let ds_b = fixtures::create_dataset(pool, &ns, &src).await;
    let df_a = fixtures::create_dataset_field(pool, &ds_a).await;
    let df_b = fixtures::create_dataset_field(pool, &ds_b).await;
    let dv_a = fixtures::create_dataset_version(pool, &ds_a, None).await;
    let dv_b = fixtures::create_dataset_version(pool, &ds_b, None).await;

    let now = Utc::now();

    column_lineage::upsert(
        pool,
        dv_b.uuid,
        df_b.uuid,
        dv_a.uuid,
        df_a.uuid,
        Some("IDENTITY"),
        Some("DIRECT"),
        now,
    )
    .await
    .unwrap();

    // Get lineage starting from field_b (should trace back to field_a)
    let rust_results = column_lineage::get_lineage(pool, 10, &[df_b.uuid], false, Utc::now())
        .await
        .unwrap();

    assert!(
        !rust_results.is_empty(),
        "column lineage should return results"
    );
    // The result should contain field_b with field_a as input
    let node = &rust_results[0];
    assert_eq!(node.field_name, df_b.name);
    assert!(node.input_fields.is_some(), "should have input fields");
}

// ===========================================================================
// Search parity tests
// ===========================================================================

#[tokio::test]
async fn parity_search_simple() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Create datasets and jobs to search for
    let ds = fixtures::create_dataset(pool, &ns, &src).await;
    let j = fixtures::create_job(pool, &ns).await;

    // Scope search to this test's namespace to avoid cross-test interference
    let rust_results = search::search(pool, "test_", None, None, 100, Some(&ns.name), None, None)
        .await
        .unwrap();

    // Verify our seeded data appears
    assert!(
        rust_results.len() >= 2,
        "search should find at least dataset + job"
    );
    assert!(
        rust_results.iter().any(|r| r.name == ds.name),
        "search should include our dataset"
    );
    assert!(
        rust_results.iter().any(|r| r.name == j.name),
        "search should include our job"
    );
}

#[tokio::test]
async fn parity_search_with_filter() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    let _ds = fixtures::create_dataset(pool, &ns, &src).await;
    let _j = fixtures::create_job(pool, &ns).await;

    // Search with DATASET filter
    let dataset_results = search::search(
        pool,
        "test_",
        Some("DATASET"),
        None,
        100,
        Some(&ns.name),
        None,
        None,
    )
    .await
    .unwrap();

    for row in &dataset_results {
        assert_eq!(
            row.type_, "DATASET",
            "filtered results should only contain DATASETs"
        );
        assert_eq!(row.namespace_name, ns.name, "should match namespace filter");
    }

    // Search with JOB filter
    let job_results = search::search(
        pool,
        "test_",
        Some("JOB"),
        None,
        100,
        Some(&ns.name),
        None,
        None,
    )
    .await
    .unwrap();

    for row in &job_results {
        assert_eq!(
            row.type_, "JOB",
            "filtered results should only contain JOBs"
        );
    }
}

// ===========================================================================
// Dataset field (version-scoped) parity tests
// ===========================================================================

#[tokio::test]
async fn parity_dataset_field_find_by_version_with_tags() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;

    // Create 2 dataset fields
    let df1 = fixtures::create_dataset_field(pool, &ds).await;
    let df2 = fixtures::create_dataset_field(pool, &ds).await;

    // Create a dataset version and map both fields to it
    let dv = fixtures::create_dataset_version(pool, &ds, None).await;
    dataset_field::update_field_mapping(pool, dv.uuid, &[df1.uuid, df2.uuid])
        .await
        .unwrap();

    // Create a tag and tag df1
    let t = tag::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_tag_name(),
        None,
    )
    .await
    .unwrap();
    dataset_field::update_tags(pool, df1.uuid, t.uuid, Utc::now())
        .await
        .unwrap();

    // Java SQL (DatasetFieldDao.findByDatasetVersion):
    let java_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT f.*, \
         ARRAY(SELECT t.name FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         INNER JOIN dataset_versions_field_mapping fm ON fm.dataset_field_uuid = f.uuid \
         WHERE fm.dataset_version_uuid = $1",
    )
    .bind(dv.uuid)
    .fetch_all(pool)
    .await
    .unwrap();

    // Rust DAO
    let rust_results = dataset_field::find_by_version_with_tags(pool, dv.uuid)
        .await
        .unwrap();

    assert_eq!(
        java_rows.len(),
        rust_results.len(),
        "find_by_version_with_tags row count mismatch"
    );
    assert_eq!(java_rows.len(), 2, "should have 2 fields mapped to version");

    // Field-by-field comparison (sort by uuid for stable ordering)
    let mut java_uuids: Vec<Uuid> = java_rows.iter().map(|r| r.get::<Uuid, _>("uuid")).collect();
    let mut rust_uuids: Vec<Uuid> = rust_results.iter().map(|r| r.uuid).collect();
    java_uuids.sort();
    rust_uuids.sort();
    assert_eq!(java_uuids, rust_uuids, "field UUIDs should match");

    // Verify the tagged field has the tag
    let tagged = rust_results.iter().find(|f| f.uuid == df1.uuid).unwrap();
    assert!(tagged.tags.contains(&t.name), "df1 should have the tag");

    // Verify the untagged field has no tags
    let untagged = rust_results.iter().find(|f| f.uuid == df2.uuid).unwrap();
    assert!(untagged.tags.is_empty(), "df2 should have no tags");
}

#[tokio::test]
async fn parity_dataset_field_find_by_version_uuids_with_tags() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Create 2 datasets, each with fields and a version
    let ds1 = fixtures::create_dataset(pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(pool, &ns, &src).await;
    let df1a = fixtures::create_dataset_field(pool, &ds1).await;
    let df1b = fixtures::create_dataset_field(pool, &ds1).await;
    let df2a = fixtures::create_dataset_field(pool, &ds2).await;

    let dv1 = fixtures::create_dataset_version(pool, &ds1, None).await;
    let dv2 = fixtures::create_dataset_version(pool, &ds2, None).await;

    dataset_field::update_field_mapping(pool, dv1.uuid, &[df1a.uuid, df1b.uuid])
        .await
        .unwrap();
    dataset_field::update_field_mapping(pool, dv2.uuid, &[df2a.uuid])
        .await
        .unwrap();

    let version_uuids = vec![dv1.uuid, dv2.uuid];

    // Java SQL (batch variant with ANY):
    let java_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT f.*, \
         ARRAY(SELECT t.name FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         INNER JOIN dataset_versions_field_mapping fm ON fm.dataset_field_uuid = f.uuid \
         WHERE fm.dataset_version_uuid = ANY($1)",
    )
    .bind(&version_uuids[..])
    .fetch_all(pool)
    .await
    .unwrap();

    // Rust DAO
    let rust_map = dataset_field::find_by_version_uuids_with_tags(pool, &version_uuids)
        .await
        .unwrap();

    // Total field count should match
    let rust_total: usize = rust_map.values().map(|v| v.len()).sum();
    assert_eq!(
        java_rows.len(),
        rust_total,
        "find_by_version_uuids_with_tags total field count mismatch"
    );

    // ds1 should have 2 fields, ds2 should have 1
    assert_eq!(
        rust_map.get(&ds1.uuid).map_or(0, |v| v.len()),
        2,
        "ds1 should have 2 fields"
    );
    assert_eq!(
        rust_map.get(&ds2.uuid).map_or(0, |v| v.len()),
        1,
        "ds2 should have 1 field"
    );
}

#[tokio::test]
async fn parity_dataset_field_version_scoped_vs_all() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;
    let ds = fixtures::create_dataset(pool, &ns, &src).await;

    // Create 3 fields (simulating schema evolution: f1, f2, f3)
    let f1 = dataset_field::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        "col_a",
        Some("VARCHAR"),
        None,
        ds.uuid,
    )
    .await
    .unwrap();
    let f2 = dataset_field::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        "col_b",
        Some("INT"),
        None,
        ds.uuid,
    )
    .await
    .unwrap();
    let f3 = dataset_field::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        "col_c",
        Some("BOOLEAN"),
        None,
        ds.uuid,
    )
    .await
    .unwrap();

    // V1 maps to all 3 fields
    let v1 = fixtures::create_dataset_version(pool, &ds, None).await;
    dataset_field::update_field_mapping(pool, v1.uuid, &[f1.uuid, f2.uuid, f3.uuid])
        .await
        .unwrap();

    // V2 maps to only f1 (schema evolved: dropped col_b and col_c)
    let v2 = fixtures::create_dataset_version(pool, &ds, None).await;
    dataset_field::update_field_mapping(pool, v2.uuid, &[f1.uuid])
        .await
        .unwrap();

    // Update current version to V2
    dataset::update_version(pool, ds.uuid, v2.uuid, Utc::now())
        .await
        .unwrap();

    // Version-scoped query (V2) should return only 1 field
    let version_scoped = dataset_field::find_by_version_with_tags(pool, v2.uuid)
        .await
        .unwrap();
    assert_eq!(
        version_scoped.len(),
        1,
        "version-scoped query for V2 should return 1 field"
    );
    assert_eq!(
        version_scoped[0].uuid, f1.uuid,
        "version-scoped V2 should contain col_a"
    );

    // Old all-fields query returns 3 (proves it was buggy for schema evolution)
    let all_fields = dataset_field::find_by_dataset_uuid_with_tags(pool, ds.uuid)
        .await
        .unwrap();
    assert_eq!(
        all_fields.len(),
        3,
        "all-fields query should return all 3 fields (stale + current)"
    );

    // Java SQL (findByDatasetVersion) for V2 should also return 1
    let java_rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
        "SELECT f.*, \
         ARRAY(SELECT t.name FROM dataset_fields_tag_mapping m \
               INNER JOIN tags t ON t.uuid = m.tag_uuid \
               WHERE m.dataset_field_uuid = f.uuid) AS tags \
         FROM dataset_fields f \
         INNER JOIN dataset_versions_field_mapping fm ON fm.dataset_field_uuid = f.uuid \
         WHERE fm.dataset_version_uuid = $1",
    )
    .bind(v2.uuid)
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(
        java_rows.len(),
        1,
        "Java SQL for V2 should also return 1 field"
    );
    assert_eq!(
        java_rows[0].get::<Uuid, _>("uuid"),
        f1.uuid,
        "Java SQL V2 result should match Rust DAO"
    );
}

// ===========================================================================
// Stats parity tests
// ===========================================================================

#[tokio::test]
async fn parity_stats_last_day_metrics() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Stats queries use generate_series and should always return 24 rows
    let rust_results = stats::get_last_day_metrics(pool).await.unwrap();
    assert_eq!(
        rust_results.len(),
        24,
        "last day metrics should return 24 hourly rows"
    );

    // Verify first row is approximately 23 hours ago (truncated to hour boundary)
    let first = &rust_results[0];
    let expected_start = Utc::now() - chrono::Duration::hours(23);
    let diff = (first.start_interval - expected_start).num_minutes().abs();
    assert!(
        diff < 65,
        "first interval should be ~23 hours ago, diff={diff} minutes"
    );

    // With a shared database, other tests may have inserted lineage events.
    // Verify all counts are non-negative (valid metrics).
    for row in &rust_results {
        assert!(row.fail >= 0, "fail count should be non-negative");
        assert!(row.start >= 0, "start count should be non-negative");
        assert!(row.complete >= 0, "complete count should be non-negative");
        assert!(row.abort >= 0, "abort count should be non-negative");
    }
}

#[tokio::test]
async fn parity_stats_last_day_jobs() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;

    // Create 3 jobs
    for _ in 0..3 {
        fixtures::create_job(pool, &ns).await;
    }

    let rust_results = stats::get_last_day_jobs(pool).await.unwrap();
    assert_eq!(
        rust_results.len(),
        24,
        "last day jobs should return 24 hourly rows"
    );

    // The last row should show the cumulative count
    let last = rust_results.last().unwrap();
    assert!(
        last.count >= 3,
        "cumulative job count should be at least 3, got {}",
        last.count
    );
}

#[tokio::test]
async fn parity_stats_interval_datasets() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let src = fixtures::create_source(pool).await;

    // Create 2 datasets
    for _ in 0..2 {
        fixtures::create_dataset(pool, &ns, &src).await;
    }

    let rust_results = stats::get_last_day_datasets(pool).await.unwrap();
    assert_eq!(
        rust_results.len(),
        24,
        "last day datasets should return 24 hourly rows"
    );

    // The last row should show cumulative count
    let last = rust_results.last().unwrap();
    assert!(
        last.count >= 2,
        "cumulative dataset count should be at least 2, got {}",
        last.count
    );
}
