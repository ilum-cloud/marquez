// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use marquez_api::db::search;

#[tokio::test]
async fn search_datasets() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    // Search for the dataset by name substring
    let results = search::search(&db.pool, &ds.name, None, None, 10, None, None, None)
        .await
        .unwrap();
    assert!(
        results
            .iter()
            .any(|r| r.name == ds.name && r.type_ == "DATASET"),
        "Should find the dataset by name"
    );
}

#[tokio::test]
async fn search_jobs() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let results = search::search(&db.pool, &job.name, None, None, 10, None, None, None)
        .await
        .unwrap();
    assert!(
        results
            .iter()
            .any(|r| r.name == job.name && r.type_ == "JOB"),
        "Should find the job by name"
    );
}

#[tokio::test]
async fn search_with_filter_dataset() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let _job = fixtures::create_job(&db.pool, &ns).await;

    // Filter by DATASET only
    let results = search::search(
        &db.pool,
        "test_",
        Some("DATASET"),
        None,
        100,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    for r in &results {
        assert_eq!(r.type_, "DATASET", "All results should be DATASET");
    }
    assert!(
        results.iter().any(|r| r.name == ds.name),
        "Should find the dataset"
    );
}

#[tokio::test]
async fn search_with_filter_job() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let _src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    // Filter by JOB only, search by the specific job name for isolation
    let results = search::search(
        &db.pool,
        &job.name,
        Some("JOB"),
        None,
        100,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    for r in &results {
        assert_eq!(r.type_, "JOB", "All results should be JOB");
    }
    assert!(
        results.iter().any(|r| r.name == job.name),
        "Should find the job"
    );
}

#[tokio::test]
async fn search_no_results() {
    let db = TestDb::new().await;

    let results = search::search(
        &db.pool,
        "nonexistent_xyz_12345",
        None,
        None,
        10,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(
        results.is_empty(),
        "Should return no results for nonexistent query"
    );
}

#[tokio::test]
async fn search_case_insensitive() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    // Search with uppercase version of the job name
    let upper_query = job.name.to_uppercase();
    let results = search::search(&db.pool, &upper_query, None, None, 10, None, None, None)
        .await
        .unwrap();
    assert!(
        results.iter().any(|r| r.name == job.name),
        "Search should be case-insensitive"
    );
}
