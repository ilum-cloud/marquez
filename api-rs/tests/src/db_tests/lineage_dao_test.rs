// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use chrono::Utc;
use marquez_api::db::{dataset_version, facets, job_version, lineage};
use uuid::Uuid;

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

/// Bug 1 regression: `get_lineage()` must respect the depth parameter.
/// A 3-job chain with depth=1 should exclude the third job.
#[tokio::test]
async fn get_lineage_depth_limit_full_query() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    // Chain: job_1 -> ds_1 -> job_2 -> ds_2 -> job_3
    let job_1 = fixtures::create_job(&db.pool, &ns).await;
    let job_2 = fixtures::create_job(&db.pool, &ns).await;
    let job_3 = fixtures::create_job(&db.pool, &ns).await;
    let ds_1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds_2 = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let jv_1 = fixtures::create_job_version(&db.pool, &job_1).await;
    let jv_2 = fixtures::create_job_version(&db.pool, &job_2).await;
    let jv_3 = fixtures::create_job_version(&db.pool, &job_3).await;

    job_version::upsert_output_dataset(&db.pool, jv_1.uuid, ds_1.uuid, job_1.uuid, None)
        .await
        .unwrap();
    job_version::upsert_input_dataset(&db.pool, jv_2.uuid, ds_1.uuid, job_2.uuid, None)
        .await
        .unwrap();
    job_version::upsert_output_dataset(&db.pool, jv_2.uuid, ds_2.uuid, job_2.uuid, None)
        .await
        .unwrap();
    job_version::upsert_input_dataset(&db.pool, jv_3.uuid, ds_2.uuid, job_3.uuid, None)
        .await
        .unwrap();

    // depth=1: should find job_1 + job_2 but NOT job_3
    let shallow = lineage::get_lineage(&db.pool, 1, &[job_1.uuid])
        .await
        .unwrap();
    let shallow_uuids: Vec<_> = shallow.iter().map(|j| j.uuid).collect();
    assert!(
        shallow_uuids.contains(&job_1.uuid),
        "depth=1 should include seed job"
    );
    assert!(
        shallow_uuids.contains(&job_2.uuid),
        "depth=1 should include 1-hop neighbor"
    );
    assert!(
        !shallow_uuids.contains(&job_3.uuid),
        "depth=1 should NOT include 2-hop neighbor"
    );

    // depth=10: should find all three
    let deep = lineage::get_lineage(&db.pool, 10, &[job_1.uuid])
        .await
        .unwrap();
    let deep_uuids: Vec<_> = deep.iter().map(|j| j.uuid).collect();
    assert!(deep_uuids.contains(&job_1.uuid));
    assert!(deep_uuids.contains(&job_2.uuid));
    assert!(deep_uuids.contains(&job_3.uuid));
}

/// Bug 2 regression: `get_current_runs_with_facets()` must return flat
/// facets, not double-wrapped `{"sql": {"sql": {...}}}`.
#[tokio::test]
async fn get_current_runs_with_facets_flat_format() {
    use marquez_api::db::{job, run};

    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job_row = fixtures::create_job(&db.pool, &ns).await;
    let jv = fixtures::create_job_version(&db.pool, &job_row).await;

    // Create a run with job_version_uuid set (required by the INNER JOIN)
    let now = Utc::now();
    let run_row = run::upsert(
        &db.pool,
        Uuid::new_v4(),
        now,
        Some(job_row.uuid),
        Some(jv.uuid),
        None,
        None,
        None,
        None,
        Some("COMPLETED"),
        Some(now),
        None,
        Some(now),
        None,
        job_row.namespace_name.as_deref().unwrap_or("default"),
        &job_row.name,
        None,
        None,
    )
    .await
    .unwrap();

    // Point the job's current_run_uuid at this run
    job::update_current_run(&db.pool, job_row.uuid, run_row.uuid, now)
        .await
        .unwrap();

    let facet_value = serde_json::json!({"sql": {"query": "SELECT 1"}});
    facets::insert_run_facet(
        &db.pool,
        now,
        run_row.uuid,
        now,
        "COMPLETE",
        "sql",
        &facet_value,
    )
    .await
    .unwrap();

    let rows = lineage::get_current_runs_with_facets(&db.pool, &[job_row.uuid])
        .await
        .unwrap();
    assert!(!rows.is_empty(), "should return at least one run");

    let row = &rows[0];
    let facets_json = row.facets.as_ref().expect("facets should not be null");

    // The facets should be flat: {"sql": {"query": "SELECT 1"}}
    // NOT double-wrapped: {"sql": {"sql": {"query": "SELECT 1"}}}
    let sql_facet = facets_json.get("sql").expect("should have 'sql' key");
    assert!(
        sql_facet.get("query").is_some(),
        "sql facet should directly contain 'query', got: {}",
        sql_facet
    );
    assert!(
        sql_facet.get("sql").is_none(),
        "sql facet should NOT be double-wrapped, got: {}",
        facets_json
    );
}

/// Bug 3 regression: `get_dataset_data()` must return datasets even when
/// they have a non-primary symlink (LEFT JOIN semantics).
#[tokio::test]
async fn get_dataset_data_with_non_primary_symlink() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    // create_dataset already creates a primary symlink
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    // Add a non-primary symlink for the same dataset
    let alias_name = format!("alias_{}", uuid::Uuid::new_v4());
    dataset_version::upsert_symlink(
        &db.pool,
        ds.uuid,
        &alias_name,
        ns.uuid,
        None,
        false,
        Utc::now(),
    )
    .await
    .unwrap();

    // Should still return the dataset (not filtered out by non-primary symlink)
    let result = lineage::get_dataset_data(&db.pool, &[ds.uuid])
        .await
        .unwrap();
    assert_eq!(
        result.len(),
        1,
        "dataset with non-primary symlink should still be returned (got {} rows)",
        result.len()
    );
    assert_eq!(result[0].uuid, ds.uuid);
}
