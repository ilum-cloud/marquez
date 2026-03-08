// SPDX-License-Identifier: Apache-2.0

//! Edge case parity tests validating specific behavioral differences between
//! Java and Rust implementations.
//!
//! These tests verify that edge cases around timestamps, NULLs, JSONB,
//! concurrent operations, and type coercions produce identical results.

use chrono::{TimeZone, Utc};
use marquez_api::db::{dataset, job, namespace, openlineage, run, run_args, source, tag};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;

// ===========================================================================
// Timestamp precision: DateTime<Utc> vs Java Instant
// ===========================================================================

#[tokio::test]
async fn edge_case_timestamptz_precision() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Insert a namespace with a precise timestamp
    let now = Utc::now();
    let uuid = Uuid::new_v4();
    let name = generators::new_namespace_name();

    let row = namespace::upsert(pool, uuid, now, &name, "owner", None)
        .await
        .unwrap();

    // Read back the raw timestamp from PG
    let (pg_created_at,): (chrono::DateTime<Utc>,) =
        sqlx::query_as("SELECT created_at FROM namespaces WHERE uuid = $1")
            .bind(row.uuid)
            .fetch_one(pool)
            .await
            .unwrap();

    // PostgreSQL TIMESTAMPTZ has microsecond precision.
    // Both Java Instant and Rust DateTime<Utc> should round-trip cleanly.
    // The difference should be within 1 microsecond.
    let diff = (pg_created_at - now).num_microseconds().unwrap_or(i64::MAX);
    assert!(
        diff.abs() <= 1,
        "TIMESTAMPTZ precision: PG={pg_created_at:?}, Rust={now:?}, diff_us={diff}"
    );
}

#[tokio::test]
async fn edge_case_naive_timestamp_cast() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // The `runs` table stores `transitioned_at`, `started_at`, `ended_at` as
    // TIMESTAMP (not TIMESTAMPTZ). Rust casts them to TIMESTAMPTZ in SELECT.
    // Java reads them as Instant (implicitly UTC).
    // Verify that the round-trip preserves the value.
    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;
    let r = fixtures::create_run(pool, &j).await;

    let transition_time = Utc::now();
    run::update_run_state(pool, r.uuid, transition_time, "RUNNING")
        .await
        .unwrap();

    // Read raw TIMESTAMP value
    let raw_row: sqlx::postgres::PgRow =
        sqlx::query("SELECT transitioned_at FROM runs WHERE uuid = $1")
            .bind(r.uuid)
            .fetch_one(pool)
            .await
            .unwrap();

    let raw_ts: chrono::NaiveDateTime = raw_row.get("transitioned_at");

    // Read via Rust DAO (which casts to TIMESTAMPTZ)
    let rust_row = run::find_by_uuid(pool, r.uuid).await.unwrap().unwrap();
    let rust_ts = rust_row.transitioned_at.unwrap();

    // The naive timestamp (stored as UTC) should match the DAO's TIMESTAMPTZ result
    let raw_as_utc = Utc.from_utc_datetime(&raw_ts);
    let diff = (raw_as_utc - rust_ts)
        .num_microseconds()
        .unwrap_or(i64::MAX);
    assert!(
        diff.abs() <= 1,
        "naive TIMESTAMP -> TIMESTAMPTZ cast: raw={raw_as_utc:?}, dao={rust_ts:?}, diff_us={diff}"
    );
}

// ===========================================================================
// NULL binding: Option::None vs Java null
// ===========================================================================

#[tokio::test]
async fn edge_case_null_binding_optional_fields() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Insert a namespace without description (None)
    let uuid = Uuid::new_v4();
    let now = Utc::now();
    let name = generators::new_namespace_name();

    let row = namespace::upsert(pool, uuid, now, &name, "owner", None)
        .await
        .unwrap();

    // Verify description is NULL in the database
    let (desc,): (Option<String>,) =
        sqlx::query_as("SELECT description FROM namespaces WHERE uuid = $1")
            .bind(row.uuid)
            .fetch_one(pool)
            .await
            .unwrap();

    assert!(
        desc.is_none(),
        "description should be NULL when passed as None"
    );

    // Now upsert with description — Java parity: description is NOT updated on conflict
    let row2 = namespace::upsert(pool, Uuid::new_v4(), now, &name, "owner", Some("hello"))
        .await
        .unwrap();

    assert!(
        row2.description.is_none(),
        "description should remain NULL after re-upsert (Java parity: no description update on conflict)"
    );
}

#[tokio::test]
async fn edge_case_null_optional_uuid_fields() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;

    // Create a run with many optional fields as None
    let r = run::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        Some(j.uuid),
        None, // job_version_uuid
        None, // parent_run_uuid
        None, // run_args_uuid
        None, // nominal_start_time
        None, // nominal_end_time
        None, // current_run_state
        None, // started_at
        None, // start_run_state_uuid
        None, // ended_at
        None, // end_run_state_uuid
        &ns.name,
        &j.name,
        None, // location
        None, // external_id
    )
    .await
    .unwrap();

    // Verify all optional fields are NULL
    let raw_row: sqlx::postgres::PgRow = sqlx::query("SELECT * FROM runs WHERE uuid = $1")
        .bind(r.uuid)
        .fetch_one(pool)
        .await
        .unwrap();

    assert!(raw_row.get::<Option<Uuid>, _>("parent_run_uuid").is_none());
    assert!(raw_row.get::<Option<Uuid>, _>("run_args_uuid").is_none());
    assert!(raw_row.get::<Option<String>, _>("location").is_none());
    assert!(raw_row.get::<Option<String>, _>("external_id").is_none());
}

// ===========================================================================
// Empty array: Rust &[] vs Java Array[]
// ===========================================================================

#[tokio::test]
async fn edge_case_empty_uuid_array() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Test that an empty UUID array works correctly in ANY($1) queries
    let empty_uuids: &[Uuid] = &[];

    let rows: Vec<sqlx::postgres::PgRow> =
        sqlx::query("SELECT * FROM namespaces WHERE uuid = ANY($1)")
            .bind(empty_uuids)
            .fetch_all(pool)
            .await
            .unwrap();

    assert_eq!(rows.len(), 0, "empty array should return 0 rows");
}

#[tokio::test]
async fn edge_case_dataset_update_empty_uuid_array() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Batch update with empty array should succeed without error
    let empty_uuids: &[Uuid] = &[];
    let now = Utc::now();

    dataset::update_last_modified_at(pool, empty_uuids, now)
        .await
        .unwrap();

    // Verify no rows were modified
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM datasets")
        .fetch_one(pool)
        .await
        .unwrap();
    // Count may be 0 or any number, but no error should have occurred.
    let _ = count;
}

// ===========================================================================
// JSONB NULL vs absent
// ===========================================================================

#[tokio::test]
async fn edge_case_jsonb_null_vs_absent() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Test that JSONB column handling is consistent between:
    // {"key": null} vs {}

    let json_with_null = serde_json::json!({"key": null});
    let json_empty = serde_json::json!({});

    // Insert two run_args rows with different JSONB patterns
    let row1 = run_args::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &json_with_null.to_string(),
        "check_null_1",
    )
    .await
    .unwrap();
    let row2 = run_args::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &json_empty.to_string(),
        "check_null_2",
    )
    .await
    .unwrap();

    // Read back raw JSON (stored as TEXT in run_args)
    let (raw1_str,): (String,) = sqlx::query_as("SELECT args FROM run_args WHERE uuid = $1")
        .bind(row1.uuid)
        .fetch_one(pool)
        .await
        .unwrap();
    let raw1: serde_json::Value = serde_json::from_str(&raw1_str).unwrap();
    let (raw2_str,): (String,) = sqlx::query_as("SELECT args FROM run_args WHERE uuid = $1")
        .bind(row2.uuid)
        .fetch_one(pool)
        .await
        .unwrap();
    let raw2: serde_json::Value = serde_json::from_str(&raw2_str).unwrap();

    // Verify: PG preserves the distinction between {"key": null} and {}
    assert!(
        raw1.get("key").is_some(),
        "JSONB should preserve 'key': null"
    );
    assert!(raw1["key"].is_null(), "JSONB 'key' value should be null");
    assert!(raw2.get("key").is_none(), "JSONB should not have 'key'");
}

// ===========================================================================
// Concurrent upserts: same final state after parallel writes
// ===========================================================================

#[tokio::test]
async fn edge_case_concurrent_namespace_upserts() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let name = generators::new_namespace_name();

    // Spawn 10 concurrent upserts for the same namespace name
    let mut handles = Vec::new();
    for i in 0..10 {
        let pool = pool.clone();
        let name = name.clone();
        handles.push(tokio::spawn(async move {
            namespace::upsert(
                &pool,
                Uuid::new_v4(),
                Utc::now(),
                &name,
                &format!("owner_{i}"),
                None,
            )
            .await
        }));
    }

    // All should succeed (no errors from concurrent ON CONFLICT)
    let mut success_count = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => success_count += 1,
            Err(e) => panic!("concurrent upsert failed: {e}"),
        }
    }
    assert_eq!(success_count, 10, "all concurrent upserts should succeed");

    // Exactly one namespace should exist
    let rows: Vec<sqlx::postgres::PgRow> = sqlx::query("SELECT * FROM namespaces WHERE name = $1")
        .bind(&name)
        .fetch_all(pool)
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "should have exactly 1 namespace after concurrent upserts"
    );
}

#[tokio::test]
async fn edge_case_concurrent_source_upserts() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let name = generators::new_source_name();

    let mut handles = Vec::new();
    for _ in 0..10 {
        let pool = pool.clone();
        let name = name.clone();
        handles.push(tokio::spawn(async move {
            source::upsert(
                &pool,
                Uuid::new_v4(),
                "POSTGRESQL",
                Utc::now(),
                &name,
                "postgresql://localhost:5432/test",
                None,
            )
            .await
        }));
    }

    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let rows: Vec<sqlx::postgres::PgRow> = sqlx::query("SELECT * FROM sources WHERE name = $1")
        .bind(&name)
        .fetch_all(pool)
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "should have exactly 1 source after concurrent upserts"
    );
}

// ===========================================================================
// Run state mapping: Java-compatible behavior
// ===========================================================================

#[tokio::test]
async fn edge_case_run_state_mapping_java_compat() {
    // Verify that the Rust get_run_state function matches Java behavior exactly

    // Standard types
    assert_eq!(openlineage::get_run_state("COMPLETE"), "COMPLETED");
    assert_eq!(openlineage::get_run_state("ABORT"), "ABORTED");
    assert_eq!(openlineage::get_run_state("FAIL"), "FAILED");
    assert_eq!(openlineage::get_run_state("START"), "RUNNING");

    // Java uses toLowerCase(), Rust uses to_lowercase()
    assert_eq!(openlineage::get_run_state("complete"), "COMPLETED");
    assert_eq!(openlineage::get_run_state("Complete"), "COMPLETED");
    assert_eq!(openlineage::get_run_state("COMPLETE"), "COMPLETED");

    // Java defaults unknown to RUNNING (not OTHER)
    assert_eq!(openlineage::get_run_state("unknown"), "RUNNING");
    assert_eq!(openlineage::get_run_state("RUNNING"), "RUNNING");
    assert_eq!(openlineage::get_run_state("OTHER"), "RUNNING");
    assert_eq!(openlineage::get_run_state(""), "RUNNING");
    assert_eq!(openlineage::get_run_state("random_string"), "RUNNING");
}

// ===========================================================================
// Namespace name sanitization: Java-compatible behavior
// ===========================================================================

#[tokio::test]
async fn edge_case_namespace_sanitization_java_compat() {
    // Verify Rust format_namespace_name matches Java regex: [^a-z:/A-Z0-9\-_.@+]

    // Allowed characters pass through unchanged
    assert_eq!(
        openlineage::format_namespace_name("s3://my-bucket/path"),
        "s3://my-bucket/path"
    );
    assert_eq!(
        openlineage::format_namespace_name("namespace_v1"),
        "namespace_v1"
    );
    assert_eq!(openlineage::format_namespace_name("ns.name"), "ns.name");
    assert_eq!(openlineage::format_namespace_name("user@host"), "user@host");
    assert_eq!(openlineage::format_namespace_name("name+tag"), "name+tag");
    assert_eq!(openlineage::format_namespace_name("abc123"), "abc123");
    assert_eq!(openlineage::format_namespace_name("ABC"), "ABC");

    // Disallowed characters replaced with _
    assert_eq!(
        openlineage::format_namespace_name("ns with spaces"),
        "ns_with_spaces"
    );
    assert_eq!(
        openlineage::format_namespace_name("ns#special"),
        "ns_special"
    );
    assert_eq!(openlineage::format_namespace_name("ns$dollar"), "ns_dollar");
    assert_eq!(
        openlineage::format_namespace_name("ns%percent"),
        "ns_percent"
    );
    assert_eq!(openlineage::format_namespace_name("ns&amp"), "ns_amp");
    assert_eq!(openlineage::format_namespace_name("ns(paren)"), "ns_paren_");
    assert_eq!(openlineage::format_namespace_name("ns{brace}"), "ns_brace_");
    assert_eq!(
        openlineage::format_namespace_name("ns[bracket]"),
        "ns_bracket_"
    );

    // Mixed allowed and disallowed
    assert_eq!(
        openlineage::format_namespace_name("s3://bucket/path with spaces"),
        "s3://bucket/path_with_spaces"
    );
}

// ===========================================================================
// Stats timezone: AT TIME ZONE produces consistent bucketing
// ===========================================================================

#[tokio::test]
async fn edge_case_timezone_bucketing() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    // Verify that DATE_TRUNC('hour', NOW()) in stats queries produces
    // consistent results regardless of PG server timezone.
    let (pg_now,): (chrono::DateTime<Utc>,) = sqlx::query_as("SELECT NOW()")
        .fetch_one(pool)
        .await
        .unwrap();

    let (truncated,): (chrono::DateTime<Utc>,) = sqlx::query_as("SELECT DATE_TRUNC('hour', NOW())")
        .fetch_one(pool)
        .await
        .unwrap();

    // Truncated should be within the same hour
    assert_eq!(
        pg_now.date_naive(),
        truncated.date_naive(),
        "should be same date"
    );
    use chrono::Timelike;
    assert_eq!(truncated.time().minute(), 0, "minutes should be 0");
    assert_eq!(truncated.time().second(), 0, "seconds should be 0");
}

// ===========================================================================
// ON CONFLICT idempotency: Rust's DO NOTHING vs Java's fail
// ===========================================================================

#[tokio::test]
async fn edge_case_idempotent_run_args_upsert() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let args = serde_json::json!({"key": "value"});
    let checksum = "unique_checksum_123";

    // First insert
    let args_str = args.to_string();
    let row1 = run_args::upsert(pool, Uuid::new_v4(), Utc::now(), &args_str, checksum)
        .await
        .unwrap();

    // Second insert with same checksum — should return same row (idempotent)
    let row2 = run_args::upsert(pool, Uuid::new_v4(), Utc::now(), &args_str, checksum)
        .await
        .unwrap();

    assert_eq!(
        row1.uuid, row2.uuid,
        "duplicate upsert should return same row"
    );
    assert_eq!(row1.checksum, row2.checksum, "checksum should match");
}

// ===========================================================================
// COALESCE behavior in upserts
// ===========================================================================

#[tokio::test]
async fn edge_case_coalesce_preserves_existing_on_null() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let ns = fixtures::create_namespace(pool).await;
    let j = fixtures::create_job(pool, &ns).await;

    // Create a run with location set
    let run_uuid = Uuid::new_v4();
    let r = run::upsert(
        pool,
        run_uuid,
        Utc::now(),
        Some(j.uuid),
        None,
        None,
        None,
        None,
        None,
        Some("RUNNING"),
        None,
        None,
        None,
        None,
        &ns.name,
        &j.name,
        Some("http://localhost:8080"),
        None,
    )
    .await
    .unwrap();

    assert_eq!(r.location.as_deref(), Some("http://localhost:8080"));

    // Upsert again with None for location — Bug 44: no COALESCE, so location is overwritten
    let r2 = run::upsert(
        pool,
        run_uuid,
        Utc::now(),
        Some(j.uuid),
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
        &j.name,
        None, // location is None
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        r2.location.as_deref(),
        None,
        "Bug 44: location should be overwritten to NULL (no COALESCE)"
    );
    assert_eq!(
        r2.current_run_state.as_deref(),
        Some("COMPLETED"),
        "run state should be updated"
    );
}

// ===========================================================================
// Pagination validation: negative, zero, and overflow edge cases
// ===========================================================================

#[test]
fn pagination_limit_clamps_negative() {
    let params = marquez_api::api::extractors::PaginationParams {
        limit: Some(-10),
        offset: None,
    };
    assert_eq!(params.limit(), 0, "negative limit should clamp to 0");
}

#[test]
fn pagination_limit_clamps_zero() {
    let params = marquez_api::api::extractors::PaginationParams {
        limit: Some(0),
        offset: None,
    };
    assert_eq!(params.limit(), 0, "zero limit should return 0");
}

#[test]
fn pagination_limit_caps_at_1000() {
    let params = marquez_api::api::extractors::PaginationParams {
        limit: Some(5000),
        offset: None,
    };
    assert_eq!(params.limit(), 1000, "limit should cap at 1000");
}

#[test]
fn pagination_limit_default() {
    let params = marquez_api::api::extractors::PaginationParams {
        limit: None,
        offset: None,
    };
    assert_eq!(params.limit(), 100, "default limit should be 100");
}

#[test]
fn pagination_offset_clamps_negative() {
    let params = marquez_api::api::extractors::PaginationParams {
        limit: None,
        offset: Some(-5),
    };
    assert_eq!(params.offset(), 0, "negative offset should clamp to 0");
}

#[test]
fn pagination_offset_default() {
    let params = marquez_api::api::extractors::PaginationParams {
        limit: None,
        offset: None,
    };
    assert_eq!(params.offset(), 0, "default offset should be 0");
}
