use chrono::{Duration, Utc};
use sqlx;

use crate::common::TestDb;
use marquez_api::models::openlineage::{
    DatasetEvent, DatasetRef, JobEvent, JobRef, LineageEvent, RunRef,
};
use marquez_api::service::openlineage::OpenLineageService;

fn make_event(event_type: &str, ns: &str, job_name: &str, run_id: &str) -> LineageEvent {
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
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    }
}

#[tokio::test]
async fn create_lineage_event_stores_event() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let event = make_event(
        "START",
        "test-ns",
        "test-job",
        &uuid::Uuid::new_v4().to_string(),
    );
    svc.create_lineage_event(&event).await.unwrap();

    // Verify event was stored by listing events
    let events = svc
        .list_events(
            Utc::now() + Duration::hours(1),
            Utc::now() - Duration::hours(1),
            10,
            0,
            "DESC",
        )
        .await
        .unwrap();
    assert!(!events.is_empty());
}

#[tokio::test]
async fn create_lineage_event_updates_model() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let run_id = uuid::Uuid::new_v4().to_string();
    let event = make_event("START", "model-ns", "model-job", &run_id);
    svc.create_lineage_event(&event).await.unwrap();

    // Verify namespace was created
    let ns = marquez_api::db::namespace::find_by_name(&db.pool, "model-ns")
        .await
        .unwrap();
    assert!(ns.is_some());

    // Verify job was created
    let job = marquez_api::db::job::find_by_name(&db.pool, "model-ns", "model-job")
        .await
        .unwrap();
    assert!(job.is_some());
}

#[tokio::test]
async fn create_lineage_event_start_then_complete() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let run_id = uuid::Uuid::new_v4().to_string();

    // START
    let start_event = make_event("START", "lifecycle-ns", "lifecycle-job", &run_id);
    svc.create_lineage_event(&start_event).await.unwrap();

    // COMPLETE
    let complete_event = make_event("COMPLETE", "lifecycle-ns", "lifecycle-job", &run_id);
    svc.create_lineage_event(&complete_event).await.unwrap();

    // Verify run state
    let run_uuid = uuid::Uuid::parse_str(&run_id).unwrap();
    let run = marquez_api::db::run::find_by_uuid(&db.pool, run_uuid)
        .await
        .unwrap();
    assert!(run.is_some());
    assert_eq!(run.unwrap().current_run_state.as_deref(), Some("COMPLETED"));
}

#[tokio::test]
async fn list_events_desc() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    // Create a couple of events
    svc.create_lineage_event(&make_event(
        "START",
        "ns-desc",
        "job-1",
        &uuid::Uuid::new_v4().to_string(),
    ))
    .await
    .unwrap();
    svc.create_lineage_event(&make_event(
        "START",
        "ns-desc",
        "job-2",
        &uuid::Uuid::new_v4().to_string(),
    ))
    .await
    .unwrap();

    let events = svc
        .list_events(
            Utc::now() + Duration::hours(1),
            Utc::now() - Duration::hours(1),
            10,
            0,
            "DESC",
        )
        .await
        .unwrap();
    assert!(events.len() >= 2);
}

#[tokio::test]
async fn list_events_asc() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    svc.create_lineage_event(&make_event(
        "START",
        "ns-asc",
        "job-asc",
        &uuid::Uuid::new_v4().to_string(),
    ))
    .await
    .unwrap();

    let events = svc
        .list_events(
            Utc::now() + Duration::hours(1),
            Utc::now() - Duration::hours(1),
            10,
            0,
            "ASC",
        )
        .await
        .unwrap();
    assert!(!events.is_empty());
}

#[tokio::test]
async fn total_count() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    svc.create_lineage_event(&make_event(
        "START",
        "ns-count",
        "job-count",
        &uuid::Uuid::new_v4().to_string(),
    ))
    .await
    .unwrap();

    let count = svc
        .get_total_count(
            Utc::now() + Duration::hours(1),
            Utc::now() - Duration::hours(1),
        )
        .await
        .unwrap();
    assert!(count >= 1);
}

#[tokio::test]
async fn create_dataset_event() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let event = DatasetEvent {
        event_time: Utc::now(),
        dataset: DatasetRef {
            namespace: "ds-ns".to_string(),
            name: "ds-name".to_string(),
            facets: None,
            output_facets: None,
        },
        producer: "test".to_string(),
        schema_url: None,
    };
    svc.create_dataset_event(&event).await.unwrap();
}

#[tokio::test]
async fn create_job_event() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let event = JobEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        job: JobRef {
            namespace: "job-ns".to_string(),
            name: "job-name".to_string(),
            facets: None,
        },
        inputs: None,
        outputs: None,
        producer: "test".to_string(),
        schema_url: None,
    };
    svc.create_job_event(&event).await.unwrap();
}

// ---------------------------------------------------------------------------
// Bug 42: ABORT event via service creates job version
// ---------------------------------------------------------------------------

/// Bug 42: Verify that START → ABORT via the service layer results in
/// ABORTED state and ended_at being set.
#[tokio::test]
async fn abort_event_sets_aborted_state() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let run_id = uuid::Uuid::new_v4().to_string();
    let ns = format!("abort-ns-{}", rand::random_range::<u32, _>(0..u32::MAX));
    let job_name = format!("abort-job-{}", rand::random_range::<u32, _>(0..u32::MAX));

    // START
    let start_event = make_event("START", &ns, &job_name, &run_id);
    svc.create_lineage_event(&start_event).await.unwrap();

    // ABORT
    let abort_event = make_event("ABORT", &ns, &job_name, &run_id);
    svc.create_lineage_event(&abort_event).await.unwrap();

    // Verify run state
    let run_uuid = uuid::Uuid::parse_str(&run_id).unwrap();
    let run = marquez_api::db::run::find_by_uuid(&db.pool, run_uuid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        run.current_run_state.as_deref(),
        Some("ABORTED"),
        "Run should be ABORTED (Bug 42)"
    );
    assert!(run.ended_at.is_some(), "ended_at should be set for ABORTED");
}

// ---------------------------------------------------------------------------
// Bug 48: Dataset event output versions have NULL run_uuid (not nil UUID)
// ---------------------------------------------------------------------------

/// Bug 48: Verify that dataset events don't store Uuid::nil() as run_uuid.
#[tokio::test]
async fn dataset_event_no_nil_run_uuid() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let ds_ns = format!("ds-ns-{}", rand::random_range::<u32, _>(0..u32::MAX));
    let ds_name = format!("ds-name-{}", rand::random_range::<u32, _>(0..u32::MAX));

    let event = DatasetEvent {
        event_time: Utc::now(),
        dataset: DatasetRef {
            namespace: ds_ns.clone(),
            name: ds_name.clone(),
            facets: None,
            output_facets: None,
        },
        producer: "test".to_string(),
        schema_url: None,
    };
    svc.create_dataset_event(&event).await.unwrap();

    // Check that no dataset_version has run_uuid = nil UUID
    let nil_uuid = uuid::Uuid::nil();
    let nil_runs: Vec<(uuid::Uuid,)> =
        sqlx::query_as("SELECT uuid FROM dataset_versions WHERE run_uuid = $1")
            .bind(nil_uuid)
            .fetch_all(&db.pool)
            .await
            .unwrap();

    assert!(
        nil_runs.is_empty(),
        "No dataset_versions should have run_uuid = nil UUID (Bug 48)"
    );
}

// ---------------------------------------------------------------------------
// Bug 59: Job event performs full model orchestration (reverting Bug 51)
// ---------------------------------------------------------------------------

/// Bug 59: Verify that a JobEvent creates namespace, job, and job version
/// model objects via full model orchestration (matching Java behavior).
#[tokio::test]
async fn job_event_creates_model_objects() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let ns = format!("je-ns-{}", rand::random_range::<u32, _>(0..u32::MAX));
    let job_name = format!("je-job-{}", rand::random_range::<u32, _>(0..u32::MAX));

    let event = JobEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: None,
        outputs: None,
        producer: "test".to_string(),
        schema_url: None,
    };
    svc.create_job_event(&event).await.unwrap();

    // Bug 59: JobEvent SHOULD create namespace and job model objects
    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap();
    assert!(
        ns_row.is_some(),
        "JobEvent should create namespace (Bug 59)"
    );

    let job_row = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap();
    assert!(job_row.is_some(), "JobEvent should create job (Bug 59)");
}

// ---------------------------------------------------------------------------
// Bug 61: link_job_facets_to_job_version called on job version creation
// ---------------------------------------------------------------------------

/// Bug 61: Verify that after a COMPLETE event with job facets, the
/// `job_facets.job_version_uuid` column is populated (not NULL).
#[tokio::test]
async fn job_facets_linked_to_job_version() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let run_id = uuid::Uuid::new_v4().to_string();
    let ns = format!("jf-ns-{}", rand::random_range::<u32, _>(0..u32::MAX));
    let job_name = format!("jf-job-{}", rand::random_range::<u32, _>(0..u32::MAX));

    // START event with job facets
    let mut job_facets = std::collections::HashMap::new();
    job_facets.insert(
        "documentation".to_string(),
        serde_json::json!({
            "_producer": "test",
            "_schemaURL": "test",
            "description": "test job"
        }),
    );
    let start_event = LineageEvent {
        event_type: Some("START".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.clone(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: Some(job_facets),
        },
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    };
    svc.create_lineage_event(&start_event).await.unwrap();

    // COMPLETE event (triggers job version creation)
    let complete_event = LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.clone(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    };
    svc.create_lineage_event(&complete_event).await.unwrap();

    // Verify job_facets.job_version_uuid is NOT NULL (Bug 61)
    let run_uuid: uuid::Uuid = run_id.parse().unwrap();
    let rows: Vec<(Option<uuid::Uuid>,)> =
        sqlx::query_as("SELECT job_version_uuid FROM job_facets WHERE run_uuid = $1")
            .bind(run_uuid)
            .fetch_all(&db.pool)
            .await
            .unwrap();

    assert!(
        !rows.is_empty(),
        "Should have job facets for the run (Bug 61)"
    );
    for (jv_uuid,) in &rows {
        assert!(
            jv_uuid.is_some(),
            "job_facets.job_version_uuid should NOT be NULL after job version creation (Bug 61)"
        );
    }
}

// ---------------------------------------------------------------------------
// Bug 66: Parent job type matches child's job type
// ---------------------------------------------------------------------------

/// Bug 66: Verify that when a child event has a STREAM job type, the parent
/// job is also created as STREAM (not hardcoded BATCH).
#[tokio::test]
async fn parent_job_inherits_child_job_type() {
    let db = TestDb::new().await;
    let svc = OpenLineageService::new(db.pool.clone(), None);
    let run_id = uuid::Uuid::new_v4().to_string();
    let parent_run_id = uuid::Uuid::new_v4().to_string();
    let ns = format!("pj-ns-{}", rand::random_range::<u32, _>(0..u32::MAX));
    let parent_job_name = format!("parent-job-{}", rand::random_range::<u32, _>(0..u32::MAX));
    let child_job_name = format!("{}.child-task", parent_job_name);

    // Create event with parent facet and jobType=STREAMING
    let mut run_facets = std::collections::HashMap::new();
    run_facets.insert(
        "parent".to_string(),
        serde_json::json!({
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id },
            "job": { "namespace": ns, "name": parent_job_name }
        }),
    );
    let mut job_facets_66 = std::collections::HashMap::new();
    job_facets_66.insert(
        "jobType".to_string(),
        serde_json::json!({
            "_producer": "test",
            "_schemaURL": "test",
            "processingType": "STREAMING",
            "integration": "SPARK",
            "jobType": "JOB"
        }),
    );
    let event = LineageEvent {
        event_type: Some("START".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.clone(),
            facets: Some(run_facets),
        },
        job: JobRef {
            namespace: ns.clone(),
            name: child_job_name.clone(),
            facets: Some(job_facets_66),
        },
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    };
    svc.create_lineage_event(&event).await.unwrap();

    // Verify the parent job was created with STREAM type (not BATCH)
    let parent = marquez_api::db::job::find_by_name(&db.pool, &ns, &parent_job_name)
        .await
        .unwrap();
    assert!(parent.is_some(), "Parent job should be created");
    assert_eq!(
        parent.unwrap().type_,
        "STREAM",
        "Parent job type should be STREAM (matches child), not hardcoded BATCH (Bug 66)"
    );
}
