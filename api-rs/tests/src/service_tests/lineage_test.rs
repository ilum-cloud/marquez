use chrono::Utc;

use crate::common::TestDb;
use marquez_api::models::api::NodeId;
use marquez_api::models::openlineage::{
    InputDatasetRef, JobRef, LineageEvent, OutputDatasetRef, RunRef,
};
use marquez_api::service::lineage::LineageService;
use marquez_api::service::openlineage::OpenLineageService;

/// Helper to create a lineage event with inputs and outputs.
fn make_lineage_event(
    event_type: &str,
    ns: &str,
    job_name: &str,
    run_id: &str,
    inputs: Vec<(&str, &str)>,
    outputs: Vec<(&str, &str)>,
) -> LineageEvent {
    LineageEvent {
        event_type: Some(event_type.to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.to_string(),
            name: job_name.to_string(),
            facets: None,
        },
        inputs: Some(
            inputs
                .into_iter()
                .map(|(ns, name)| InputDatasetRef {
                    namespace: ns.to_string(),
                    name: name.to_string(),
                    facets: None,
                    input_facets: None,
                })
                .collect(),
        ),
        outputs: Some(
            outputs
                .into_iter()
                .map(|(ns, name)| OutputDatasetRef {
                    namespace: ns.to_string(),
                    name: name.to_string(),
                    facets: None,
                    output_facets: None,
                })
                .collect(),
        ),
        producer: "test".to_string(),
        schema_url: None,
    }
}

/// Helper: send START then COMPLETE for a job so that IO mappings are created.
///
/// The orchestrator creates the job version at the end of processing. IO
/// mappings require a pre-existing job version, so on the first event for a
/// job they are skipped. Sending a second event ensures the version exists.
async fn create_job_with_io(
    ol_svc: &OpenLineageService,
    ns: &str,
    job_name: &str,
    inputs: Vec<(&str, &str)>,
    outputs: Vec<(&str, &str)>,
) {
    let run_id = uuid::Uuid::new_v4().to_string();
    // First event creates the job + job version
    ol_svc
        .create_lineage_event(&make_lineage_event(
            "START",
            ns,
            job_name,
            &run_id,
            inputs.clone(),
            outputs.clone(),
        ))
        .await
        .unwrap();
    // Second event finds the existing job version and inserts IO mappings
    ol_svc
        .create_lineage_event(&make_lineage_event(
            "COMPLETE", ns, job_name, &run_id, inputs, outputs,
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn single_hop_lineage() {
    let db = TestDb::new().await;
    let ol_svc = OpenLineageService::new(db.pool.clone(), None);
    let lineage_svc = LineageService::new(db.pool.clone());

    // Job A reads from ds-input, writes to ds-output
    create_job_with_io(
        &ol_svc,
        "lineage-ns",
        "job-a",
        vec![("lineage-ns", "ds-input")],
        vec![("lineage-ns", "ds-output")],
    )
    .await;

    let lineage = lineage_svc
        .get_lineage(&NodeId::new("job:lineage-ns:job-a"), 10)
        .await
        .unwrap();

    assert!(!lineage.graph.is_empty());
}

#[tokio::test]
async fn multi_hop_lineage() {
    let db = TestDb::new().await;
    let ol_svc = OpenLineageService::new(db.pool.clone(), None);
    let lineage_svc = LineageService::new(db.pool.clone());

    // Job A -> ds-mid
    create_job_with_io(
        &ol_svc,
        "multi-ns",
        "job-a",
        vec![],
        vec![("multi-ns", "ds-mid")],
    )
    .await;

    // Job B reads ds-mid -> ds-out
    create_job_with_io(
        &ol_svc,
        "multi-ns",
        "job-b",
        vec![("multi-ns", "ds-mid")],
        vec![("multi-ns", "ds-out")],
    )
    .await;

    let lineage = lineage_svc
        .get_lineage(&NodeId::new("job:multi-ns:job-a"), 10)
        .await
        .unwrap();

    // Should include both jobs and datasets
    assert!(
        lineage.graph.len() >= 2,
        "Expected multi-hop lineage, got {} nodes",
        lineage.graph.len()
    );
}

#[tokio::test]
async fn lineage_with_depth_limit() {
    let db = TestDb::new().await;
    let ol_svc = OpenLineageService::new(db.pool.clone(), None);
    let lineage_svc = LineageService::new(db.pool.clone());

    create_job_with_io(
        &ol_svc,
        "depth-ns",
        "job-depth",
        vec![("depth-ns", "ds-in")],
        vec![("depth-ns", "ds-out")],
    )
    .await;

    let lineage = lineage_svc
        .get_lineage(&NodeId::new("job:depth-ns:job-depth"), 0)
        .await
        .unwrap();

    // With depth 0 we still get at least the seed job + its datasets
    assert!(!lineage.graph.is_empty());
}

#[tokio::test]
async fn lineage_from_dataset_node() {
    let db = TestDb::new().await;
    let ol_svc = OpenLineageService::new(db.pool.clone(), None);
    let lineage_svc = LineageService::new(db.pool.clone());

    create_job_with_io(
        &ol_svc,
        "ds-lineage-ns",
        "ds-job",
        vec![],
        vec![("ds-lineage-ns", "output-ds")],
    )
    .await;

    let lineage = lineage_svc
        .get_lineage(&NodeId::new("dataset:ds-lineage-ns:output-ds"), 10)
        .await
        .unwrap();

    assert!(!lineage.graph.is_empty());
}

#[tokio::test]
async fn upstream_runs() {
    let db = TestDb::new().await;
    let ol_svc = OpenLineageService::new(db.pool.clone(), None);
    let lineage_svc = LineageService::new(db.pool.clone());

    let run_id = uuid::Uuid::new_v4();
    ol_svc
        .create_lineage_event(&make_lineage_event(
            "START",
            "upstream-ns",
            "upstream-job",
            &run_id.to_string(),
            vec![("upstream-ns", "up-ds")],
            vec![],
        ))
        .await
        .unwrap();

    let upstream = lineage_svc.get_upstream_runs(run_id, 10).await.unwrap();
    // The run itself should be in the results (depth 0)
    assert!(!upstream.runs.is_empty());
}

#[tokio::test]
async fn upstream_runs_empty() {
    let db = TestDb::new().await;
    let lineage_svc = LineageService::new(db.pool.clone());
    // Non-existent run - should return empty (the CTE has no seed)
    let upstream = lineage_svc
        .get_upstream_runs(uuid::Uuid::new_v4(), 10)
        .await
        .unwrap();
    assert!(upstream.runs.is_empty());
}
