// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use chrono::Utc;
use marquez_api::db::run_state;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    let uuid = Uuid::new_v4();
    let now = Utc::now();
    let row = run_state::upsert(&db.pool, uuid, now, run.uuid, "RUNNING")
        .await
        .unwrap();

    assert_eq!(row.uuid, uuid);
    assert_eq!(row.run_uuid, Some(run.uuid));
    assert_eq!(row.state, "RUNNING");
}

#[tokio::test]
async fn upsert_multiple_states() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    let s1 = run_state::upsert(&db.pool, Uuid::new_v4(), Utc::now(), run.uuid, "NEW")
        .await
        .unwrap();
    let s2 = run_state::upsert(&db.pool, Uuid::new_v4(), Utc::now(), run.uuid, "RUNNING")
        .await
        .unwrap();
    let s3 = run_state::upsert(&db.pool, Uuid::new_v4(), Utc::now(), run.uuid, "COMPLETED")
        .await
        .unwrap();

    assert_eq!(s1.state, "NEW");
    assert_eq!(s2.state, "RUNNING");
    assert_eq!(s3.state, "COMPLETED");
}

#[tokio::test]
async fn find_by_run_uuid() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    // Insert states with increasing timestamps.
    run_state::upsert(&db.pool, Uuid::new_v4(), Utc::now(), run.uuid, "NEW")
        .await
        .unwrap();
    run_state::upsert(&db.pool, Uuid::new_v4(), Utc::now(), run.uuid, "RUNNING")
        .await
        .unwrap();
    run_state::upsert(&db.pool, Uuid::new_v4(), Utc::now(), run.uuid, "COMPLETED")
        .await
        .unwrap();

    let states = run_state::find_by_run_uuid(&db.pool, run.uuid)
        .await
        .unwrap();
    assert_eq!(states.len(), 3);
    // Most recent first (DESC order).
    assert_eq!(states[0].state, "COMPLETED");
    assert_eq!(states[1].state, "RUNNING");
    assert_eq!(states[2].state, "NEW");
}

#[tokio::test]
async fn find_by_run_uuid_empty() {
    let db = TestDb::new().await;

    let states = run_state::find_by_run_uuid(&db.pool, Uuid::new_v4())
        .await
        .unwrap();
    assert!(states.is_empty());
}
