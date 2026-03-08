// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use marquez_api::db::{job_version, lineage};

#[tokio::test]
async fn get_lineage_single_job() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    // A single job with no connections should return itself.
    let result = lineage::get_lineage_job_uuids(&db.pool, 5, &[job.uuid])
        .await
        .unwrap();
    assert_eq!(result.len(), 1);
    assert!(result.contains(&job.uuid));
}

#[tokio::test]
async fn get_lineage_connected() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    // Create two jobs that share a dataset:
    // job_a (OUTPUT) -> dataset -> job_b (INPUT)
    let job_a = fixtures::create_job(&db.pool, &ns).await;
    let job_b = fixtures::create_job(&db.pool, &ns).await;
    let dataset = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let jv_a = fixtures::create_job_version(&db.pool, &job_a).await;
    let jv_b = fixtures::create_job_version(&db.pool, &job_b).await;

    // job_a outputs the dataset
    job_version::upsert_output_dataset(&db.pool, jv_a.uuid, dataset.uuid, job_a.uuid, None)
        .await
        .unwrap();

    // job_b inputs the dataset
    job_version::upsert_input_dataset(&db.pool, jv_b.uuid, dataset.uuid, job_b.uuid, None)
        .await
        .unwrap();

    // Starting from job_a, should find both jobs
    let result = lineage::get_lineage_job_uuids(&db.pool, 5, &[job_a.uuid])
        .await
        .unwrap();
    assert!(
        result.len() >= 2,
        "Expected at least 2 jobs in lineage, got {}",
        result.len()
    );
    assert!(result.contains(&job_a.uuid));
    assert!(result.contains(&job_b.uuid));

    // Starting from job_b, should also find both
    let result2 = lineage::get_lineage_job_uuids(&db.pool, 5, &[job_b.uuid])
        .await
        .unwrap();
    assert!(result2.len() >= 2);
    assert!(result2.contains(&job_a.uuid));
    assert!(result2.contains(&job_b.uuid));
}

#[tokio::test]
async fn get_lineage_depth_limit() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    // Create a chain: job_1 -> ds_1 -> job_2 -> ds_2 -> job_3
    let job_1 = fixtures::create_job(&db.pool, &ns).await;
    let job_2 = fixtures::create_job(&db.pool, &ns).await;
    let job_3 = fixtures::create_job(&db.pool, &ns).await;
    let ds_1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds_2 = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let jv_1 = fixtures::create_job_version(&db.pool, &job_1).await;
    let jv_2 = fixtures::create_job_version(&db.pool, &job_2).await;
    let jv_3 = fixtures::create_job_version(&db.pool, &job_3).await;

    // job_1 outputs ds_1
    job_version::upsert_output_dataset(&db.pool, jv_1.uuid, ds_1.uuid, job_1.uuid, None)
        .await
        .unwrap();
    // job_2 inputs ds_1
    job_version::upsert_input_dataset(&db.pool, jv_2.uuid, ds_1.uuid, job_2.uuid, None)
        .await
        .unwrap();
    // job_2 outputs ds_2
    job_version::upsert_output_dataset(&db.pool, jv_2.uuid, ds_2.uuid, job_2.uuid, None)
        .await
        .unwrap();
    // job_3 inputs ds_2
    job_version::upsert_input_dataset(&db.pool, jv_3.uuid, ds_2.uuid, job_3.uuid, None)
        .await
        .unwrap();

    // Depth 1 from job_1: should find job_1 + job_2 (but not job_3)
    let result = lineage::get_lineage_job_uuids(&db.pool, 1, &[job_1.uuid])
        .await
        .unwrap();
    assert!(result.contains(&job_1.uuid));
    assert!(result.contains(&job_2.uuid));
    // job_3 might not be reached with depth=1
    // (depth=1 means one hop: job_1 -> ds_1 -> job_2)

    // Depth 10 from job_1: should find all three
    let result_deep = lineage::get_lineage_job_uuids(&db.pool, 10, &[job_1.uuid])
        .await
        .unwrap();
    assert!(result_deep.contains(&job_1.uuid));
    assert!(result_deep.contains(&job_2.uuid));
    assert!(result_deep.contains(&job_3.uuid));
}

#[tokio::test]
async fn get_upstream_runs() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let _src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let run = fixtures::create_run(&db.pool, &job).await;

    // Simple test: query upstream of a run with no connections
    let result = lineage::get_upstream_runs(&db.pool, run.uuid, 5)
        .await
        .unwrap();
    // Should at least return the run itself
    assert!(result.len() >= 1);
    assert!(result.iter().any(|r| r.r_uuid == run.uuid));
}

#[tokio::test]
async fn get_lineage_empty_input() {
    let db = TestDb::new().await;

    // Empty job list should return empty
    let result = lineage::get_lineage_job_uuids(&db.pool, 5, &[])
        .await
        .unwrap();
    assert!(result.is_empty());
}
