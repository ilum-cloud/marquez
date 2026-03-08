// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use chrono::Utc;
use marquez_api::db::facets;
use uuid::Uuid;

#[tokio::test]
async fn insert_dataset_facet() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let dv = fixtures::create_dataset_version(&db.pool, &ds, Some(run.uuid)).await;

    let facet = serde_json::json!({"schema": {"fields": []}});
    facets::insert_dataset_facet(
        &db.pool,
        Utc::now(),
        ds.uuid,
        dv.uuid,
        Some(run.uuid),
        Utc::now(),
        Some("START"),
        "DATASET",
        "schema",
        &facet,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn insert_job_facet() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    let facet = serde_json::json!({"sql": "SELECT 1"});
    facets::insert_job_facet(
        &db.pool,
        Utc::now(),
        job.uuid,
        run.uuid,
        Utc::now(),
        Some("COMPLETE"),
        "sql",
        &facet,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn insert_run_facet() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    let facet = serde_json::json!({"environment": "production"});
    facets::insert_run_facet(
        &db.pool,
        Utc::now(),
        run.uuid,
        Utc::now(),
        "START",
        "environment",
        &facet,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn run_facet_exists_true() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    let event_time = Utc::now();
    let facet = serde_json::json!({"key": "value"});
    facets::insert_run_facet(
        &db.pool,
        Utc::now(),
        run.uuid,
        event_time,
        "COMPLETE",
        "test_facet",
        &facet,
    )
    .await
    .unwrap();

    assert!(
        facets::run_facet_exists(&db.pool, run.uuid, event_time, "COMPLETE", "test_facet")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn run_facet_exists_false() {
    let db = TestDb::new().await;

    assert!(!facets::run_facet_exists(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        "START",
        "nonexistent"
    )
    .await
    .unwrap());
}

#[tokio::test]
async fn find_job_facets_by_run() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    let facet1 = serde_json::json!({"sql": "SELECT 1"});
    let facet2 = serde_json::json!({"documentation": "A job"});

    facets::insert_job_facet(
        &db.pool,
        Utc::now(),
        job.uuid,
        run.uuid,
        Utc::now(),
        Some("COMPLETE"),
        "sql",
        &facet1,
    )
    .await
    .unwrap();

    facets::insert_job_facet(
        &db.pool,
        Utc::now(),
        job.uuid,
        run.uuid,
        Utc::now(),
        Some("COMPLETE"),
        "documentation",
        &facet2,
    )
    .await
    .unwrap();

    let result = facets::find_job_facets_by_run(&db.pool, run.uuid)
        .await
        .unwrap();
    assert!(result.is_some());
    let obj = result.unwrap();
    assert!(obj.is_object(), "Expected object, got: {}", obj);
    assert_eq!(obj.as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn find_run_facets_by_run() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    let facet1 = serde_json::json!({"environment": "prod"});
    let facet2 = serde_json::json!({"spark_version": "3.5"});

    facets::insert_run_facet(
        &db.pool,
        Utc::now(),
        run.uuid,
        Utc::now(),
        "START",
        "environment",
        &facet1,
    )
    .await
    .unwrap();

    facets::insert_run_facet(
        &db.pool,
        Utc::now(),
        run.uuid,
        Utc::now(),
        "START",
        "spark_version",
        &facet2,
    )
    .await
    .unwrap();

    let result = facets::find_run_facets_by_run(&db.pool, run.uuid)
        .await
        .unwrap();
    assert!(result.is_some());
    let obj = result.unwrap();
    assert!(obj.is_object(), "Expected object, got: {}", obj);
    assert_eq!(obj.as_object().unwrap().len(), 2);
}
