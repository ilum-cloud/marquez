// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use chrono::{Duration, Utc};
use marquez_api::db::{run, run_state};
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let uuid = Uuid::new_v4();
    let now = Utc::now();

    let row = run::upsert(
        &db.pool,
        uuid,
        now,
        Some(job.uuid),
        None,
        None,
        None,
        None,
        None,
        Some("NEW"),
        None,
        None,
        None,
        None,
        &ns.name,
        &job.name,
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(row.uuid, uuid);
    assert_eq!(row.job_uuid, Some(job.uuid));
    assert_eq!(row.current_run_state.as_deref(), Some("NEW"));
    assert_eq!(row.namespace_name.as_deref(), Some(ns.name.as_str()));
    assert_eq!(row.job_name.as_deref(), Some(job.name.as_str()));
}

#[tokio::test]
async fn upsert_updates_on_conflict() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let uuid = Uuid::new_v4();
    let now = Utc::now();

    let row1 = run::upsert(
        &db.pool,
        uuid,
        now,
        Some(job.uuid),
        None,
        None,
        None,
        None,
        None,
        Some("NEW"),
        None,
        None,
        None,
        None,
        &ns.name,
        &job.name,
        None,
        None,
    )
    .await
    .unwrap();

    // Upsert again with same UUID but different state.
    let row2 = run::upsert(
        &db.pool,
        uuid,
        Utc::now(),
        Some(job.uuid),
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
        &job.name,
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row2.current_run_state.as_deref(), Some("RUNNING"));
    assert!(row2.updated_at >= row1.updated_at);
}

#[tokio::test]
async fn exists_true() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;

    assert!(run::exists(&db.pool, r.uuid).await.unwrap());
}

#[tokio::test]
async fn exists_false() {
    let db = TestDb::new().await;

    assert!(!run::exists(&db.pool, Uuid::new_v4()).await.unwrap());
}

#[tokio::test]
async fn find_by_uuid() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;

    let found = run::find_by_uuid(&db.pool, r.uuid).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().uuid, r.uuid);
}

#[tokio::test]
async fn find_by_uuid_not_found() {
    let db = TestDb::new().await;

    let found = run::find_by_uuid(&db.pool, Uuid::new_v4()).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn update_run_state() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;

    run::update_run_state(&db.pool, r.uuid, Utc::now(), "RUNNING")
        .await
        .unwrap();

    let found = run::find_by_uuid(&db.pool, r.uuid).await.unwrap().unwrap();
    assert_eq!(found.current_run_state.as_deref(), Some("RUNNING"));
}

#[tokio::test]
async fn update_start_state() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;

    let start_uuid = Uuid::new_v4();
    // Create the run_state row so FK is satisfied.
    run_state::upsert(&db.pool, start_uuid, Utc::now(), r.uuid, "RUNNING")
        .await
        .unwrap();

    let started_at = Utc::now();
    run::update_start_state(&db.pool, r.uuid, started_at, start_uuid)
        .await
        .unwrap();

    let found = run::find_by_uuid(&db.pool, r.uuid).await.unwrap().unwrap();
    assert!(found.started_at.is_some());
    assert_eq!(found.start_run_state_uuid, Some(start_uuid));
}

#[tokio::test]
async fn update_end_state() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;

    let end_uuid = Uuid::new_v4();
    // Create the run_state row so FK is satisfied.
    run_state::upsert(&db.pool, end_uuid, Utc::now(), r.uuid, "COMPLETED")
        .await
        .unwrap();

    let ended_at = Utc::now();
    run::update_end_state(&db.pool, r.uuid, ended_at, end_uuid)
        .await
        .unwrap();

    let found = run::find_by_uuid(&db.pool, r.uuid).await.unwrap().unwrap();
    assert!(found.ended_at.is_some());
    assert_eq!(found.end_run_state_uuid, Some(end_uuid));
}

#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    for _ in 0..3 {
        fixtures::create_run(&db.pool, &job).await;
    }

    let page1 = run::find_by_job(&db.pool, &ns.name, &job.name, 2, 0)
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = run::find_by_job(&db.pool, &ns.name, &job.name, 2, 2)
        .await
        .unwrap();
    assert_eq!(page2.len(), 1);
}

#[tokio::test]
async fn find_by_job() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job1 = fixtures::create_job(&db.pool, &ns).await;
    let job2 = fixtures::create_job(&db.pool, &ns).await;

    for _ in 0..2 {
        fixtures::create_run(&db.pool, &job1).await;
    }
    fixtures::create_run(&db.pool, &job2).await;

    let runs = run::find_by_job(&db.pool, &ns.name, &job1.name, 10, 0)
        .await
        .unwrap();
    assert_eq!(runs.len(), 2);
    for r in &runs {
        assert_eq!(r.job_name.as_deref(), Some(job1.name.as_str()));
    }
}

#[tokio::test]
async fn update_job_version() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;

    run::update_job_version(&db.pool, r.uuid, jv.uuid)
        .await
        .unwrap();

    let found = run::find_by_uuid(&db.pool, r.uuid).await.unwrap().unwrap();
    assert_eq!(found.job_version_uuid, Some(jv.uuid));
}

#[tokio::test]
async fn update_input_mapping() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let dv = fixtures::create_dataset_version(&db.pool, &ds, None).await;

    run::update_input_mapping(&db.pool, r.uuid, dv.uuid)
        .await
        .unwrap();

    // Idempotent -- calling again should not error.
    run::update_input_mapping(&db.pool, r.uuid, dv.uuid)
        .await
        .unwrap();

    // Verify via dataset_version query.
    let inputs = marquez_api::db::dataset_version::find_input_versions_for(&db.pool, r.uuid)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].uuid, dv.uuid);
}

/// Bug 13: Verify `updated_at` advances after state transitions.
///
/// When `update_run_state`, `update_start_state`, or `update_end_state` are
/// called, the run's `updated_at` field should advance to at least the
/// transition timestamp.
#[tokio::test]
async fn updated_at_advances_on_state_transitions() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let r = fixtures::create_run(&db.pool, &job).await;
    let original_updated_at = r.updated_at;

    // Transition 1: update_run_state to RUNNING
    let transition_time = Utc::now() + Duration::seconds(1);
    run::update_run_state(&db.pool, r.uuid, transition_time, "RUNNING")
        .await
        .unwrap();
    let after_state = run::find_by_uuid(&db.pool, r.uuid).await.unwrap().unwrap();
    assert!(
        after_state.updated_at > original_updated_at,
        "updated_at should advance after update_run_state"
    );

    // Transition 2: update_start_state
    let start_time = Utc::now() + Duration::seconds(2);
    let start_uuid = Uuid::new_v4();
    run_state::upsert(&db.pool, start_uuid, start_time, r.uuid, "RUNNING")
        .await
        .unwrap();
    run::update_start_state(&db.pool, r.uuid, start_time, start_uuid)
        .await
        .unwrap();
    let after_start = run::find_by_uuid(&db.pool, r.uuid).await.unwrap().unwrap();
    assert!(
        after_start.updated_at > after_state.updated_at,
        "updated_at should advance after update_start_state"
    );

    // Transition 3: update_end_state
    let end_time = Utc::now() + Duration::seconds(3);
    let end_uuid = Uuid::new_v4();
    run_state::upsert(&db.pool, end_uuid, end_time, r.uuid, "COMPLETED")
        .await
        .unwrap();
    run::update_end_state(&db.pool, r.uuid, end_time, end_uuid)
        .await
        .unwrap();
    let after_end = run::find_by_uuid(&db.pool, r.uuid).await.unwrap().unwrap();
    assert!(
        after_end.updated_at > after_start.updated_at,
        "updated_at should advance after update_end_state"
    );
}

// ---------------------------------------------------------------------------
// Bug 44: Run upsert overwrites current_run_state and location (no COALESCE)
// ---------------------------------------------------------------------------

/// Bug 44: Verify that run upsert directly sets current_run_state (not
/// COALESCE). A second upsert with a different state should overwrite.
#[tokio::test]
async fn upsert_overwrites_current_run_state() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let uuid = Uuid::new_v4();
    let now = Utc::now();

    // First upsert with RUNNING
    run::upsert(
        &db.pool,
        uuid,
        now,
        Some(job.uuid),
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
        &job.name,
        None,
        None,
    )
    .await
    .unwrap();

    // Second upsert with None state — should overwrite to None (no COALESCE)
    let row = run::upsert(
        &db.pool,
        uuid,
        Utc::now(),
        Some(job.uuid),
        None,
        None,
        None,
        None,
        None,
        None, // explicitly None
        None,
        None,
        None,
        None,
        &ns.name,
        &job.name,
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        row.current_run_state, None,
        "current_run_state should be overwritten to None (Bug 44)"
    );
}

// ---------------------------------------------------------------------------
// Bug 65: Run list sorted by started_at DESC NULLS LAST
// ---------------------------------------------------------------------------

/// Bug 65: Verify that find_all and find_by_job sort by started_at DESC
/// NULLS LAST (matching Java's RunDao), not by updated_at DESC.
#[tokio::test]
async fn find_by_job_ordered_by_started_at_desc() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    // Create 3 runs, then set different started_at times
    let r1 = fixtures::create_run(&db.pool, &job).await;
    let r2 = fixtures::create_run(&db.pool, &job).await;
    let r3 = fixtures::create_run(&db.pool, &job).await;

    // Set started_at: r2 most recent, then r1, then r3 oldest
    let now = Utc::now();
    let s2_uuid = Uuid::new_v4();
    run_state::upsert(&db.pool, s2_uuid, now, r2.uuid, "RUNNING")
        .await
        .unwrap();
    run::update_start_state(&db.pool, r2.uuid, now, s2_uuid)
        .await
        .unwrap();

    let early = now - chrono::Duration::hours(2);
    let s1_uuid = Uuid::new_v4();
    run_state::upsert(&db.pool, s1_uuid, early, r1.uuid, "RUNNING")
        .await
        .unwrap();
    run::update_start_state(&db.pool, r1.uuid, early, s1_uuid)
        .await
        .unwrap();

    let oldest = now - chrono::Duration::hours(4);
    let s3_uuid = Uuid::new_v4();
    run_state::upsert(&db.pool, s3_uuid, oldest, r3.uuid, "RUNNING")
        .await
        .unwrap();
    run::update_start_state(&db.pool, r3.uuid, oldest, s3_uuid)
        .await
        .unwrap();

    let runs = run::find_by_job(&db.pool, &ns.name, &job.name, 10, 0)
        .await
        .unwrap();
    assert_eq!(runs.len(), 3);
    // Order should be: r2 (most recent started_at), r1, r3
    assert_eq!(
        runs[0].uuid, r2.uuid,
        "First run should have most recent started_at (Bug 65)"
    );
    assert_eq!(runs[1].uuid, r1.uuid);
    assert_eq!(runs[2].uuid, r3.uuid);
}

/// Bug 44: Verify that run upsert directly sets location (not COALESCE).
#[tokio::test]
async fn upsert_overwrites_location() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let uuid = Uuid::new_v4();
    let now = Utc::now();

    // First upsert with a location
    run::upsert(
        &db.pool,
        uuid,
        now,
        Some(job.uuid),
        None,
        None,
        None,
        None,
        None,
        Some("NEW"),
        None,
        None,
        None,
        None,
        &ns.name,
        &job.name,
        Some("https://old-location"),
        None,
    )
    .await
    .unwrap();

    // Second upsert with different location
    let row = run::upsert(
        &db.pool,
        uuid,
        Utc::now(),
        Some(job.uuid),
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
        &job.name,
        Some("https://new-location"),
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        row.location.as_deref(),
        Some("https://new-location"),
        "location should be overwritten (Bug 44)"
    );
}
