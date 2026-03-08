use chrono::Utc;
use uuid::Uuid;

use crate::common::TestDb;
use crate::{fixtures, generators};
use marquez_api::service::job::JobService;
use marquez_api::service::namespace::NamespaceService;
use marquez_api::service::run::RunService;

async fn setup(pool: &sqlx::PgPool) -> (String, String) {
    let ns_svc = NamespaceService::new(pool.clone());
    let job_svc = JobService::new(pool.clone());
    let ns = generators::new_namespace_name();
    let job = generators::new_job_name();
    ns_svc.create_or_update(&ns, "owner", None).await.unwrap();
    job_svc
        .create_or_update(&ns, &job, "BATCH", None, None)
        .await
        .unwrap();
    (ns, job)
}

#[tokio::test]
async fn create_run() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    assert_eq!(run.state, marquez_api::models::common::RunState::New);
}

#[tokio::test]
async fn create_run_with_args() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let args = serde_json::json!({"key": "value"});
    let run = svc
        .create_run(&ns, &job, Some(&args), None, None)
        .await
        .unwrap();
    assert_eq!(run.state, marquez_api::models::common::RunState::New);
}

#[tokio::test]
async fn get() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let created = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let fetched = svc.get(*created.id.value()).await.unwrap();
    assert_eq!(fetched.id, created.id);
}

#[tokio::test]
async fn get_not_found() {
    let db = TestDb::new().await;
    let svc = RunService::new(db.pool.clone());
    assert!(svc.get(uuid::Uuid::new_v4()).await.is_err());
}

#[tokio::test]
async fn list() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let list = svc.list(10, 0).await.unwrap();
    assert!(!list.is_empty());
}

#[tokio::test]
async fn list_by_job() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let list = svc.list_by_job(&ns, &job, 10, 0).await.unwrap();
    assert!(!list.is_empty());
}

#[tokio::test]
async fn mark_as_running() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let started = svc.start(*run.id.value(), Utc::now()).await.unwrap();
    assert_eq!(
        started.state,
        marquez_api::models::common::RunState::Running
    );
    assert!(started.started_at.is_some());
}

#[tokio::test]
async fn mark_as_completed() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    svc.start(*run.id.value(), Utc::now()).await.unwrap();
    let completed = svc.complete(*run.id.value(), Utc::now()).await.unwrap();
    assert_eq!(
        completed.state,
        marquez_api::models::common::RunState::Completed
    );
    assert!(completed.ended_at.is_some());
}

#[tokio::test]
async fn mark_as_failed() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    svc.start(*run.id.value(), Utc::now()).await.unwrap();
    let failed = svc.fail(*run.id.value(), Utc::now()).await.unwrap();
    assert_eq!(failed.state, marquez_api::models::common::RunState::Failed);
    assert!(failed.ended_at.is_some());
}

#[tokio::test]
async fn mark_as_aborted() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    svc.start(*run.id.value(), Utc::now()).await.unwrap();
    let aborted = svc.abort(*run.id.value(), Utc::now()).await.unwrap();
    assert_eq!(
        aborted.state,
        marquez_api::models::common::RunState::Aborted
    );
    assert!(aborted.ended_at.is_some());
}

#[tokio::test]
async fn state_machine_start_complete() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    assert_eq!(run.state, marquez_api::models::common::RunState::New);

    let running = svc.start(*run.id.value(), Utc::now()).await.unwrap();
    assert_eq!(
        running.state,
        marquez_api::models::common::RunState::Running
    );

    let completed = svc.complete(*running.id.value(), Utc::now()).await.unwrap();
    assert_eq!(
        completed.state,
        marquez_api::models::common::RunState::Completed
    );
    assert!(completed.started_at.is_some());
    assert!(completed.ended_at.is_some());
    assert!(completed.duration_ms.is_some());
}

#[tokio::test]
async fn state_machine_start_fail() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    svc.start(*run.id.value(), Utc::now()).await.unwrap();
    let failed = svc.fail(*run.id.value(), Utc::now()).await.unwrap();
    assert_eq!(failed.state, marquez_api::models::common::RunState::Failed);
}

#[tokio::test]
async fn state_machine_start_abort() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    svc.start(*run.id.value(), Utc::now()).await.unwrap();
    let aborted = svc.abort(*run.id.value(), Utc::now()).await.unwrap();
    assert_eq!(
        aborted.state,
        marquez_api::models::common::RunState::Aborted
    );
}

// ---------------------------------------------------------------------------
// Bug 49: last_modified_at updated on output datasets when run completes
// ---------------------------------------------------------------------------

/// Bug 49: Verify that completing a run updates last_modified_at on output
/// datasets.
#[tokio::test]
async fn complete_updates_output_dataset_last_modified_at() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let run_uuid = *run.id.value();

    // Use fixtures to create dataset (includes symlink so it appears in datasets_view)
    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap()
        .expect("Namespace should exist after setup");
    let src = crate::fixtures::create_source(&db.pool).await;
    let ds = crate::fixtures::create_dataset(&db.pool, &ns_row, &src).await;

    // Create dataset version linked to this run (makes it an "output" of the run)
    marquez_api::db::dataset_version::upsert(
        &db.pool,
        uuid::Uuid::new_v4(),
        Utc::now(),
        ds.uuid,
        uuid::Uuid::new_v4(),
        None,
        Some(run_uuid),
        None,
        &ns,
        &ds.name,
        None,
    )
    .await
    .unwrap();

    let ds_ns = ds.namespace_name.as_deref().unwrap_or(&ns);
    let before = marquez_api::db::dataset::find_dataset_as_row(&db.pool, ds_ns, &ds.name)
        .await
        .unwrap()
        .expect("Dataset should appear in datasets_view");
    let before_modified = before.last_modified_at;

    // Start and complete the run
    svc.start(run_uuid, Utc::now()).await.unwrap();
    let completion_time = Utc::now() + chrono::Duration::seconds(1);
    svc.complete(run_uuid, completion_time).await.unwrap();

    let after = marquez_api::db::dataset::find_dataset_as_row(&db.pool, ds_ns, &ds.name)
        .await
        .unwrap()
        .expect("Dataset should still appear in datasets_view");

    assert!(
        after.last_modified_at > before_modified
            || (before_modified.is_none() && after.last_modified_at.is_some()),
        "last_modified_at should be updated on output datasets after run completion (Bug 49)"
    );
}

// ---------------------------------------------------------------------------
// Enrichment correctness tests — verify optimized query paths return full data
// ---------------------------------------------------------------------------

/// Verify that get() returns args populated from the optimized SQL query.
#[tokio::test]
async fn get_returns_args() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let args = serde_json::json!({"env": "prod", "retries": "3"});
    let run = svc
        .create_run(&ns, &job, Some(&args), None, None)
        .await
        .unwrap();
    let fetched = svc.get(*run.id.value()).await.unwrap();
    assert_eq!(fetched.args["env"], "prod");
    assert_eq!(fetched.args["retries"], "3");
}

/// Verify that get() returns merged run facets from the optimized query.
#[tokio::test]
async fn get_returns_facets() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let run_uuid = *run.id.value();
    let now = Utc::now();

    // Insert two run facets
    marquez_api::db::facets::insert_run_facet(
        &db.pool,
        now,
        run_uuid,
        now,
        "COMPLETE",
        "spark_version",
        &serde_json::json!({"spark_version": {"version": "3.4"}}),
    )
    .await
    .unwrap();
    marquez_api::db::facets::insert_run_facet(
        &db.pool,
        now,
        run_uuid,
        now,
        "COMPLETE",
        "environment",
        &serde_json::json!({"environment": {"env": "production"}}),
    )
    .await
    .unwrap();

    let fetched = svc.get(run_uuid).await.unwrap();
    assert!(
        fetched.facets["spark_version"].is_object(),
        "Expected spark_version facet, got: {}",
        fetched.facets
    );
    assert!(
        fetched.facets["environment"].is_object(),
        "Expected environment facet, got: {}",
        fetched.facets
    );
}

/// Verify that completing a run creates a job version and get() returns the link.
#[tokio::test]
async fn get_returns_job_version() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let run_uuid = *run.id.value();

    svc.start(run_uuid, Utc::now()).await.unwrap();
    let completed = svc.complete(run_uuid, Utc::now()).await.unwrap();

    assert!(
        completed.job_version.is_some(),
        "Expected job_version to be populated after completion"
    );
    let jv = completed.job_version.unwrap();
    assert_eq!(jv.namespace, ns);
    assert_eq!(jv.name, job);
}

/// Verify that get() returns input/output dataset versions in the correct API format.
#[tokio::test]
async fn get_returns_io_versions() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let run_uuid = *run.id.value();

    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap()
        .unwrap();
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns_row, &src).await;

    // Create an output dataset version (linked via run_uuid)
    let out_dv = fixtures::create_dataset_version(&db.pool, &ds, Some(run_uuid)).await;

    // Create an input dataset version and link it
    let in_ds = fixtures::create_dataset(&db.pool, &ns_row, &src).await;
    let in_dv = fixtures::create_dataset_version(&db.pool, &in_ds, None).await;
    marquez_api::db::run::update_input_mapping(&db.pool, run_uuid, in_dv.uuid)
        .await
        .unwrap();

    let fetched = svc.get(run_uuid).await.unwrap();

    // Verify outputs
    let outputs = fetched.output_dataset_versions.as_array().unwrap();
    assert!(
        !outputs.is_empty(),
        "Expected output_dataset_versions to be populated"
    );
    assert!(outputs[0]["datasetVersionId"]["version"].is_string());
    assert_eq!(
        outputs[0]["datasetVersionId"]["version"].as_str().unwrap(),
        out_dv.version.to_string()
    );

    // Verify inputs
    let inputs = fetched.input_dataset_versions.as_array().unwrap();
    assert!(
        !inputs.is_empty(),
        "Expected input_dataset_versions to be populated"
    );
    assert_eq!(
        inputs[0]["datasetVersionId"]["version"].as_str().unwrap(),
        in_dv.version.to_string()
    );
}

/// Verify that list_by_job() returns enriched runs via the CTE path.
#[tokio::test]
async fn list_by_job_returns_enriched_runs() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let args = serde_json::json!({"pipeline": "etl"});
    let run = svc
        .create_run(&ns, &job, Some(&args), None, None)
        .await
        .unwrap();
    let run_uuid = *run.id.value();

    // Insert a run facet
    let now = Utc::now();
    marquez_api::db::facets::insert_run_facet(
        &db.pool,
        now,
        run_uuid,
        now,
        "COMPLETE",
        "spark",
        &serde_json::json!({"spark": {"version": "3.5"}}),
    )
    .await
    .unwrap();

    let runs = svc.list_by_job(&ns, &job, 10, 0).await.unwrap();
    assert!(!runs.is_empty());
    let fetched = &runs[0];
    assert_eq!(fetched.args["pipeline"], "etl");
    assert!(
        fetched.facets["spark"].is_object(),
        "Expected spark facet via CTE path, got: {}",
        fetched.facets
    );
}

/// Verify that dataset facets are patched into IO version entries.
#[tokio::test]
async fn get_returns_dataset_facets_on_io_versions() {
    let db = TestDb::new().await;
    let (ns, job) = setup(&db.pool).await;
    let svc = RunService::new(db.pool.clone());
    let run = svc.create_run(&ns, &job, None, None, None).await.unwrap();
    let run_uuid = *run.id.value();

    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap()
        .unwrap();
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns_row, &src).await;
    let out_dv = fixtures::create_dataset_version(&db.pool, &ds, Some(run_uuid)).await;

    // Insert a dataset facet for this output version
    let now = Utc::now();
    marquez_api::db::facets::insert_dataset_facet(
        &db.pool,
        now,
        ds.uuid,
        out_dv.uuid,
        Some(run_uuid),
        now,
        Some("COMPLETE"),
        "output",
        "schema",
        &serde_json::json!({"schema": {"fields": [{"name": "id", "type": "INT"}]}}),
    )
    .await
    .unwrap();

    let fetched = svc.get(run_uuid).await.unwrap();
    let outputs = fetched.output_dataset_versions.as_array().unwrap();
    assert!(!outputs.is_empty());
    let facets = &outputs[0]["facets"];
    assert!(
        facets["schema"].is_object(),
        "Expected schema facet on output dataset version, got: {}",
        facets
    );
}
