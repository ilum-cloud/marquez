// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;
use chrono::Utc;
use marquez_api::db::job_version;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let uuid = Uuid::new_v4();
    let version = generators::new_version();
    let now = Utc::now();

    let row = job_version::upsert(
        &db.pool,
        uuid,
        now,
        job.uuid,
        None,
        version,
        None,
        job.namespace_uuid,
        &ns.name,
        &job.name,
    )
    .await
    .unwrap();

    assert_eq!(row.uuid, uuid);
    assert_eq!(row.job_uuid, Some(job.uuid));
    assert_eq!(row.version, version);
    assert_eq!(row.namespace_name.as_deref(), Some(ns.name.as_str()));
    assert_eq!(row.job_name.as_deref(), Some(job.name.as_str()));
}

#[tokio::test]
async fn upsert_updates_on_conflict() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let version = generators::new_version();

    let row1 = job_version::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        job.uuid,
        None,
        version,
        None,
        job.namespace_uuid,
        &ns.name,
        &job.name,
    )
    .await
    .unwrap();

    // Same version => conflict => update updated_at only (location is immutable).
    let row2 = job_version::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        job.uuid,
        Some("https://new-location"),
        version,
        None,
        job.namespace_uuid,
        &ns.name,
        &job.name,
    )
    .await
    .unwrap();

    assert_eq!(row1.uuid, row2.uuid);
    // Location should NOT change on conflict (immutable after creation).
    assert_eq!(row2.location, row1.location);
    assert!(row2.updated_at >= row1.updated_at);
}

#[tokio::test]
async fn find_job_version() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;

    let found = job_version::find_job_version(&db.pool, &ns.name, &job.name, jv.version)
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().uuid, jv.uuid);
}

#[tokio::test]
async fn find_job_version_not_found() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let found = job_version::find_job_version(&db.pool, &ns.name, &job.name, Uuid::new_v4())
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    for _ in 0..3 {
        fixtures::create_job_version(&db.pool, &job).await;
    }

    let page1 = job_version::find_all(&db.pool, &ns.name, &job.name, 2, 0)
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = job_version::find_all(&db.pool, &ns.name, &job.name, 2, 2)
        .await
        .unwrap();
    assert_eq!(page2.len(), 1);
}

#[tokio::test]
async fn upsert_input_dataset() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    job_version::upsert_input_dataset(&db.pool, jv.uuid, ds.uuid, job.uuid, None)
        .await
        .unwrap();

    let inputs = job_version::find_input_datasets(&db.pool, jv.uuid)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].dataset_uuid, Some(ds.uuid));
    assert_eq!(inputs[0].io_type, "INPUT");
    assert_eq!(inputs[0].is_current_job_version, Some(true));
}

#[tokio::test]
async fn upsert_output_dataset() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    job_version::upsert_output_dataset(&db.pool, jv.uuid, ds.uuid, job.uuid, None)
        .await
        .unwrap();

    let outputs = job_version::find_output_datasets(&db.pool, jv.uuid)
        .await
        .unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].dataset_uuid, Some(ds.uuid));
    assert_eq!(outputs[0].io_type, "OUTPUT");
    assert_eq!(outputs[0].is_current_job_version, Some(true));
}

#[tokio::test]
async fn find_input_datasets() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;

    // Add two input datasets.
    let ds1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    job_version::upsert_input_dataset(&db.pool, jv.uuid, ds1.uuid, job.uuid, None)
        .await
        .unwrap();
    job_version::upsert_input_dataset(&db.pool, jv.uuid, ds2.uuid, job.uuid, None)
        .await
        .unwrap();

    let inputs = job_version::find_input_datasets(&db.pool, jv.uuid)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 2);
}

#[tokio::test]
async fn find_output_datasets() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;

    let ds1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    job_version::upsert_output_dataset(&db.pool, jv.uuid, ds1.uuid, job.uuid, None)
        .await
        .unwrap();
    job_version::upsert_output_dataset(&db.pool, jv.uuid, ds2.uuid, job.uuid, None)
        .await
        .unwrap();

    let outputs = job_version::find_output_datasets(&db.pool, jv.uuid)
        .await
        .unwrap();
    assert_eq!(outputs.len(), 2);
}

#[tokio::test]
async fn update_latest_run() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;
    let r = fixtures::create_run(&db.pool, &job).await;

    job_version::update_latest_run(&db.pool, jv.uuid, r.uuid, Utc::now())
        .await
        .unwrap();

    let found = job_version::find_job_version(&db.pool, &ns.name, &job.name, jv.version)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.latest_run_uuid, Some(r.uuid));
}

#[tokio::test]
async fn count() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    fixtures::create_job_version(&db.pool, &job).await;
    fixtures::create_job_version(&db.pool, &job).await;

    let n = job_version::count(&db.pool, &ns.name, &job.name)
        .await
        .unwrap();
    assert_eq!(n, 2);
}

// ---------------------------------------------------------------------------
// Bug 50: Job version ON CONFLICT does NOT update location (immutable)
// ---------------------------------------------------------------------------

/// Bug 50: Verify that upserting a job version with the same version UUID
/// does NOT change the location (location should be immutable after creation).
#[tokio::test]
async fn upsert_does_not_update_location() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let version = generators::new_version();

    // First upsert with a location
    let row1 = job_version::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        job.uuid,
        Some("https://original-location"),
        version,
        None,
        job.namespace_uuid,
        &ns.name,
        &job.name,
    )
    .await
    .unwrap();

    assert_eq!(row1.location.as_deref(), Some("https://original-location"));

    // Second upsert with different location — should NOT change location
    let row2 = job_version::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        job.uuid,
        Some("https://new-location"),
        version,
        None,
        job.namespace_uuid,
        &ns.name,
        &job.name,
    )
    .await
    .unwrap();

    assert_eq!(row1.uuid, row2.uuid, "Should be same row (conflict)");
    assert_eq!(
        row2.location.as_deref(),
        Some("https://original-location"),
        "Location should NOT change on conflict (Bug 50)"
    );
}

// ---------------------------------------------------------------------------
// Batch IO upsert tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upsert_input_datasets_batch() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;

    let ds1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds3 = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let ds_uuids = vec![ds1.uuid, ds2.uuid, ds3.uuid];
    job_version::upsert_input_datasets_batch(&db.pool, jv.uuid, &ds_uuids, job.uuid, None)
        .await
        .unwrap();

    let inputs = job_version::find_input_datasets(&db.pool, jv.uuid)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 3, "batch should insert all 3 input datasets");
    for input in &inputs {
        assert_eq!(input.io_type, "INPUT");
        assert_eq!(input.is_current_job_version, Some(true));
    }
}

#[tokio::test]
async fn upsert_output_datasets_batch() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;

    let ds1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let ds_uuids = vec![ds1.uuid, ds2.uuid];
    job_version::upsert_output_datasets_batch(&db.pool, jv.uuid, &ds_uuids, job.uuid, None)
        .await
        .unwrap();

    let outputs = job_version::find_output_datasets(&db.pool, jv.uuid)
        .await
        .unwrap();
    assert_eq!(
        outputs.len(),
        2,
        "batch should insert all 2 output datasets"
    );
    for output in &outputs {
        assert_eq!(output.io_type, "OUTPUT");
        assert_eq!(output.is_current_job_version, Some(true));
    }
}

#[tokio::test]
async fn upsert_input_datasets_batch_empty() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job).await;

    // Empty batch should be a no-op.
    job_version::upsert_input_datasets_batch(&db.pool, jv.uuid, &[], job.uuid, None)
        .await
        .unwrap();

    let inputs = job_version::find_input_datasets(&db.pool, jv.uuid)
        .await
        .unwrap();
    assert!(inputs.is_empty());
}

#[tokio::test]
async fn upsert_input_datasets_batch_marks_previous_not_current() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;
    let jv1 = fixtures::create_job_version(&db.pool, &job).await;
    let jv2 = fixtures::create_job_version(&db.pool, &job).await;

    let ds1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(&db.pool, &ns, &src).await;

    // Insert inputs for jv1.
    job_version::upsert_input_datasets_batch(&db.pool, jv1.uuid, &[ds1.uuid], job.uuid, None)
        .await
        .unwrap();

    // Insert inputs for jv2 — should mark jv1's inputs as not current.
    job_version::upsert_input_datasets_batch(
        &db.pool,
        jv2.uuid,
        &[ds1.uuid, ds2.uuid],
        job.uuid,
        None,
    )
    .await
    .unwrap();

    let current_inputs = job_version::find_current_input_dataset_uuids(&db.pool, job.uuid)
        .await
        .unwrap();
    assert_eq!(
        current_inputs.len(),
        2,
        "jv2's batch inputs should be current"
    );
}
