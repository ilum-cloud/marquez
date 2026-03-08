// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use marquez_api::db::stats;

/// Note: The materialized view `lineage_events_by_type_hourly_view` is
/// populated on creation but is NOT automatically refreshed. So event
/// metrics tests may return empty results unless we explicitly refresh
/// the view. These tests verify that the queries execute without error
/// and return the expected shape.

#[tokio::test]
async fn get_last_day_metrics() {
    let db = TestDb::new().await;

    // Should not error, may return empty (materialized view may not have been refreshed)
    let _result = stats::get_last_day_metrics(&db.pool).await.unwrap();
}

#[tokio::test]
async fn get_last_week_metrics() {
    let db = TestDb::new().await;

    let _result = stats::get_last_week_metrics(&db.pool, "UTC").await.unwrap();
}

#[tokio::test]
async fn get_last_day_jobs() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let _job = fixtures::create_job(&db.pool, &ns).await;

    let result = stats::get_last_day_jobs(&db.pool).await.unwrap();
    // Should have at least 1 interval with the job we just created
    assert!(
        !result.is_empty(),
        "Should have at least one hourly interval"
    );
    let total: i64 = result.iter().map(|r| r.count).sum();
    assert!(total >= 1, "Should count at least the job we created");
}

#[tokio::test]
async fn get_last_week_jobs() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let _job = fixtures::create_job(&db.pool, &ns).await;

    let result = stats::get_last_week_jobs(&db.pool, "UTC").await.unwrap();
    assert!(!result.is_empty());
    let total: i64 = result.iter().map(|r| r.count).sum();
    assert!(total >= 1);
}

#[tokio::test]
async fn get_last_day_datasets() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let _ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let result = stats::get_last_day_datasets(&db.pool).await.unwrap();
    assert!(!result.is_empty());
    let total: i64 = result.iter().map(|r| r.count).sum();
    assert!(total >= 1);
}

#[tokio::test]
async fn get_last_week_datasets() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let _ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let result = stats::get_last_week_datasets(&db.pool, "UTC")
        .await
        .unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn get_last_day_sources() {
    let db = TestDb::new().await;
    let _src = fixtures::create_source(&db.pool).await;

    let result = stats::get_last_day_sources(&db.pool).await.unwrap();
    assert!(!result.is_empty());
    let total: i64 = result.iter().map(|r| r.count).sum();
    assert!(total >= 1);
}

#[tokio::test]
async fn get_last_week_sources() {
    let db = TestDb::new().await;
    let _src = fixtures::create_source(&db.pool).await;

    let result = stats::get_last_week_sources(&db.pool, "UTC").await.unwrap();
    assert!(!result.is_empty());
}
