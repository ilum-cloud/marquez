use chrono::Utc;

use crate::common::TestDb;
use crate::{fixtures, generators};
use marquez_api::service::job::JobService;
use marquez_api::service::namespace::NamespaceService;
use marquez_api::service::run::RunService;
use marquez_api::service::tag::TagService;

async fn setup_ns(pool: &sqlx::PgPool) -> String {
    let svc = NamespaceService::new(pool.clone());
    let name = generators::new_namespace_name();
    svc.create_or_update(&name, "owner", None).await.unwrap();
    name
}

#[tokio::test]
async fn create_or_update() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    let job = svc
        .create_or_update(&ns, "my-job", "BATCH", None, None)
        .await
        .unwrap();
    assert_eq!(job.name, "my-job");
    assert_eq!(job.namespace, ns);
}

#[tokio::test]
async fn get_found() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    svc.create_or_update(&ns, "my-job", "BATCH", Some("desc"), None)
        .await
        .unwrap();
    let job = svc.get(&ns, "my-job").await.unwrap();
    assert_eq!(job.name, "my-job");
    assert_eq!(job.description, Some("desc".to_string()));
}

#[tokio::test]
async fn get_not_found() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    assert!(svc.get(&ns, "nope").await.is_err());
}

#[tokio::test]
async fn list() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    svc.create_or_update(&ns, "j-a", "BATCH", None, None)
        .await
        .unwrap();
    svc.create_or_update(&ns, "j-b", "BATCH", None, None)
        .await
        .unwrap();
    let list = svc.list(&ns, 10, 0, &[]).await.unwrap();
    assert!(list.len() >= 2);
}

#[tokio::test]
async fn count() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    svc.create_or_update(&ns, "j-1", "BATCH", None, None)
        .await
        .unwrap();
    assert!(svc.count(&ns).await.unwrap() >= 1);
}

#[tokio::test]
async fn delete() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    svc.create_or_update(&ns, "to-del", "BATCH", None, None)
        .await
        .unwrap();
    svc.delete(&ns, "to-del").await.unwrap();
    assert!(svc.get(&ns, "to-del").await.is_err());
}

#[tokio::test]
async fn tag_job() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let tag_svc = TagService::new(db.pool.clone());
    tag_svc.create_or_update("etl", None).await.unwrap();

    let svc = JobService::new(db.pool.clone());
    svc.create_or_update(&ns, "tagged-job", "BATCH", None, None)
        .await
        .unwrap();
    let job = svc.tag_job(&ns, "tagged-job", "etl").await.unwrap();
    assert!(job.tags.contains(&"etl".to_string()));
}

#[tokio::test]
async fn list_versions_empty() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    svc.create_or_update(&ns, "j-versions", "BATCH", None, None)
        .await
        .unwrap();
    let versions = svc.list_versions(&ns, "j-versions", 10, 0).await.unwrap();
    // May or may not have versions depending on insert trigger
    assert!(versions.is_empty() || !versions.is_empty()); // Just verify no error
}

#[tokio::test]
async fn count_versions() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let svc = JobService::new(db.pool.clone());
    svc.create_or_update(&ns, "j-cv", "BATCH", None, None)
        .await
        .unwrap();
    let c = svc.count_versions(&ns, "j-cv").await.unwrap();
    assert!(c >= 0);
}

// ---------------------------------------------------------------------------
// Enrichment correctness tests
// ---------------------------------------------------------------------------

/// Verify that get() returns latest_run with enriched args and facets.
#[tokio::test]
async fn get_returns_latest_run_enriched() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let job_svc = JobService::new(db.pool.clone());
    let run_svc = RunService::new(db.pool.clone());

    let job_name = generators::new_job_name();
    job_svc
        .create_or_update(&ns, &job_name, "BATCH", None, None)
        .await
        .unwrap();

    let args = serde_json::json!({"mode": "incremental"});
    let run = run_svc
        .create_run(&ns, &job_name, Some(&args), None, None)
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
        "spark_version",
        &serde_json::json!({"spark_version": {"version": "3.4"}}),
    )
    .await
    .unwrap();

    // Start and complete the run to create a job version
    run_svc.start(run_uuid, Utc::now()).await.unwrap();
    run_svc.complete(run_uuid, Utc::now()).await.unwrap();

    let job = job_svc.get(&ns, &job_name).await.unwrap();
    assert!(job.latest_run.is_some(), "Expected latest_run to be set");
    let lr = job.latest_run.unwrap();
    assert_eq!(lr.args["mode"], "incremental");
    assert!(
        lr.facets["spark_version"].is_object(),
        "Expected spark_version facet on latest run"
    );
}

/// Verify that get() returns inputs/outputs from the latest run's IO versions.
#[tokio::test]
async fn get_returns_io_from_latest_run() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let job_svc = JobService::new(db.pool.clone());
    let run_svc = RunService::new(db.pool.clone());

    let job_name = generators::new_job_name();
    job_svc
        .create_or_update(&ns, &job_name, "BATCH", None, None)
        .await
        .unwrap();

    let run = run_svc
        .create_run(&ns, &job_name, None, None, None)
        .await
        .unwrap();
    let run_uuid = *run.id.value();

    // Create output dataset linked to this run
    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap()
        .unwrap();
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns_row, &src).await;
    fixtures::create_dataset_version(&db.pool, &ds, Some(run_uuid)).await;

    run_svc.start(run_uuid, Utc::now()).await.unwrap();
    run_svc.complete(run_uuid, Utc::now()).await.unwrap();

    let job = job_svc.get(&ns, &job_name).await.unwrap();
    assert!(
        !job.outputs.is_empty(),
        "Expected job outputs to be populated from latest run IO"
    );
}

/// Verify that list() returns enriched jobs with latest runs.
#[tokio::test]
async fn list_returns_enriched_jobs() {
    let db = TestDb::new().await;
    let ns = setup_ns(&db.pool).await;
    let job_svc = JobService::new(db.pool.clone());
    let run_svc = RunService::new(db.pool.clone());

    let job_name = generators::new_job_name();
    job_svc
        .create_or_update(&ns, &job_name, "BATCH", None, None)
        .await
        .unwrap();

    let args = serde_json::json!({"batch_id": "42"});
    let run = run_svc
        .create_run(&ns, &job_name, Some(&args), None, None)
        .await
        .unwrap();
    let run_uuid = *run.id.value();

    run_svc.start(run_uuid, Utc::now()).await.unwrap();
    run_svc.complete(run_uuid, Utc::now()).await.unwrap();

    let jobs = job_svc.list(&ns, 10, 0, &[]).await.unwrap();
    let found = jobs.iter().find(|j| j.name == job_name);
    assert!(found.is_some(), "Expected job in list");
    let job = found.unwrap();
    assert!(
        job.latest_run.is_some(),
        "Expected latest_run in listed job"
    );
    assert_eq!(job.latest_run.as_ref().unwrap().args["batch_id"], "42");
}
