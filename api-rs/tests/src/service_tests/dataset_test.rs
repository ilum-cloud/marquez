use chrono::Utc;

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;
use marquez_api::service::dataset::DatasetService;
use marquez_api::service::job::JobService;
use marquez_api::service::namespace::NamespaceService;
use marquez_api::service::run::RunService;
use marquez_api::service::source::SourceService;
use marquez_api::service::tag::TagService;

/// Helper: create prerequisite namespace and source for dataset tests.
async fn setup(pool: &sqlx::PgPool) -> (String, String) {
    let ns_svc = NamespaceService::new(pool.clone());
    let src_svc = SourceService::new(pool.clone());
    let ns_name = generators::new_namespace_name();
    let src_name = generators::new_source_name();
    ns_svc
        .create_or_update(&ns_name, "owner", None)
        .await
        .unwrap();
    src_svc
        .create_or_update(
            "POSTGRESQL",
            &src_name,
            "jdbc:postgresql://localhost/db",
            None,
        )
        .await
        .unwrap();
    (ns_name, src_name)
}

#[tokio::test]
async fn create_or_update() {
    let db = TestDb::new().await;
    let (ns, src) = setup(&db.pool).await;
    let svc = DatasetService::new(db.pool.clone());
    let ds = svc
        .create_or_update(
            &ns,
            "my-dataset",
            "DB_TABLE",
            &src,
            "public.my_dataset",
            None,
            &[],
            &[],
        )
        .await
        .unwrap();
    assert_eq!(ds.name, "my-dataset");
    assert_eq!(ds.namespace, ns);
}

#[tokio::test]
async fn get_found() {
    let db = TestDb::new().await;
    let (ns, src) = setup(&db.pool).await;
    let svc = DatasetService::new(db.pool.clone());
    svc.create_or_update(
        &ns,
        "my-ds",
        "DB_TABLE",
        &src,
        "public.my_ds",
        Some("A dataset"),
        &[],
        &[],
    )
    .await
    .unwrap();
    let ds = svc.get(&ns, "my-ds").await.unwrap();
    assert_eq!(ds.name, "my-ds");
    assert_eq!(ds.description, Some("A dataset".to_string()));
}

#[tokio::test]
async fn get_not_found() {
    let db = TestDb::new().await;
    let svc = DatasetService::new(db.pool.clone());
    let ns = generators::new_namespace_name();
    assert!(svc.get(&ns, "nope").await.is_err());
}

#[tokio::test]
async fn list() {
    let db = TestDb::new().await;
    let (ns, src) = setup(&db.pool).await;
    let svc = DatasetService::new(db.pool.clone());
    svc.create_or_update(&ns, "ds-a", "DB_TABLE", &src, "ds_a", None, &[], &[])
        .await
        .unwrap();
    svc.create_or_update(&ns, "ds-b", "DB_TABLE", &src, "ds_b", None, &[], &[])
        .await
        .unwrap();
    let list = svc.list(&ns, 10, 0).await.unwrap();
    assert!(list.len() >= 2);
}

#[tokio::test]
async fn count() {
    let db = TestDb::new().await;
    let (ns, src) = setup(&db.pool).await;
    let svc = DatasetService::new(db.pool.clone());
    svc.create_or_update(&ns, "ds-1", "DB_TABLE", &src, "ds1", None, &[], &[])
        .await
        .unwrap();
    let c = svc.count(&ns).await.unwrap();
    assert!(c >= 1);
}

#[tokio::test]
async fn delete() {
    let db = TestDb::new().await;
    let (ns, src) = setup(&db.pool).await;
    let svc = DatasetService::new(db.pool.clone());
    svc.create_or_update(&ns, "to-del", "DB_TABLE", &src, "to_del", None, &[], &[])
        .await
        .unwrap();
    svc.delete(&ns, "to-del").await.unwrap();
    // After soft-delete the dataset should not appear in find
    assert!(svc.get(&ns, "to-del").await.is_err());
}

#[tokio::test]
async fn tag_dataset() {
    let db = TestDb::new().await;
    let (ns, src) = setup(&db.pool).await;
    let tag_svc = TagService::new(db.pool.clone());
    tag_svc.create_or_update("pii", None).await.unwrap();

    let svc = DatasetService::new(db.pool.clone());
    svc.create_or_update(
        &ns,
        "tagged-ds",
        "DB_TABLE",
        &src,
        "tagged_ds",
        None,
        &[],
        &[],
    )
    .await
    .unwrap();
    let ds = svc.tag_dataset(&ns, "tagged-ds", "pii").await.unwrap();
    assert!(ds.tags.contains(&"pii".to_string()));
}

#[tokio::test]
async fn delete_tag() {
    let db = TestDb::new().await;
    let (ns, src) = setup(&db.pool).await;
    let tag_svc = TagService::new(db.pool.clone());
    tag_svc.create_or_update("removeme", None).await.unwrap();

    let svc = DatasetService::new(db.pool.clone());
    svc.create_or_update(
        &ns,
        "tagged-ds2",
        "DB_TABLE",
        &src,
        "tagged_ds2",
        None,
        &[],
        &[],
    )
    .await
    .unwrap();
    svc.tag_dataset(&ns, "tagged-ds2", "removeme")
        .await
        .unwrap();
    let ds = svc.delete_tag(&ns, "tagged-ds2", "removeme").await.unwrap();
    assert!(!ds.tags.contains(&"removeme".to_string()));
}

#[tokio::test]
async fn get_returns_fields() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    // Create a version and field
    let dv = fixtures::create_dataset_version(&db.pool, &ds, None).await;
    let field = fixtures::create_dataset_field(&db.pool, &ds).await;
    // Link field to version
    marquez_api::db::dataset_field::update_field_mapping(&db.pool, dv.uuid, &[field.uuid])
        .await
        .unwrap();
    // Update dataset current version
    marquez_api::db::dataset::update_version(&db.pool, ds.uuid, dv.uuid, chrono::Utc::now())
        .await
        .unwrap();

    let svc = DatasetService::new(db.pool.clone());
    let result = svc.get(ns.name.as_str(), ds.name.as_str()).await.unwrap();
    assert!(!result.fields.is_empty(), "Expected fields to be populated");
}

#[tokio::test]
async fn list_versions() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    fixtures::create_dataset_version(&db.pool, &ds, None).await;
    fixtures::create_dataset_version(&db.pool, &ds, None).await;

    let svc = DatasetService::new(db.pool.clone());
    let versions = svc.list_versions(&ns.name, &ds.name, 10, 0).await.unwrap();
    assert!(versions.len() >= 2);
}

#[tokio::test]
async fn count_versions() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    fixtures::create_dataset_version(&db.pool, &ds, None).await;

    let svc = DatasetService::new(db.pool.clone());
    let c = svc.count_versions(&ns.name, &ds.name).await.unwrap();
    assert!(c >= 1);
}

#[tokio::test]
async fn list_versions_empty() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let svc = DatasetService::new(db.pool.clone());
    let versions = svc.list_versions(&ns.name, &ds.name, 10, 0).await.unwrap();
    assert!(versions.is_empty());
}

// ---------------------------------------------------------------------------
// Enrichment correctness tests — verify createdByRun on dataset versions
// ---------------------------------------------------------------------------

/// Verify that get_version() returns createdByRun with enriched data.
#[tokio::test]
async fn get_version_returns_created_by_run() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    // Create a job and run
    let ns_svc = NamespaceService::new(db.pool.clone());
    ns_svc
        .create_or_update(&ns.name, "owner", None)
        .await
        .unwrap();
    let job_svc = JobService::new(db.pool.clone());
    let job_name = generators::new_job_name();
    job_svc
        .create_or_update(&ns.name, &job_name, "BATCH", None, None)
        .await
        .unwrap();
    let run_svc = RunService::new(db.pool.clone());
    let args = serde_json::json!({"source": "s3"});
    let run = run_svc
        .create_run(&ns.name, &job_name, Some(&args), None, None)
        .await
        .unwrap();
    let run_uuid = *run.id.value();

    // Create a dataset version linked to this run
    let dv = fixtures::create_dataset_version(&db.pool, &ds, Some(run_uuid)).await;

    let ds_svc = DatasetService::new(db.pool.clone());
    let version = ds_svc
        .get_version(&ns.name, &ds.name, dv.uuid)
        .await
        .unwrap();

    assert!(
        version.run.is_some(),
        "Expected createdByRun to be populated"
    );
    let run_data = version.run.unwrap();
    assert_eq!(run_data.args["source"], "s3");
}

/// Verify that list_versions() returns createdByRun via the optimized enriched path.
#[tokio::test]
async fn list_versions_returns_created_by_run() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    // Create a job and run
    let ns_svc = NamespaceService::new(db.pool.clone());
    ns_svc
        .create_or_update(&ns.name, "owner", None)
        .await
        .unwrap();
    let job_svc = JobService::new(db.pool.clone());
    let job_name = generators::new_job_name();
    job_svc
        .create_or_update(&ns.name, &job_name, "BATCH", None, None)
        .await
        .unwrap();
    let run_svc = RunService::new(db.pool.clone());
    let args = serde_json::json!({"target": "warehouse"});
    let run = run_svc
        .create_run(&ns.name, &job_name, Some(&args), None, None)
        .await
        .unwrap();
    let run_uuid = *run.id.value();

    // Create dataset versions linked to this run
    fixtures::create_dataset_version(&db.pool, &ds, Some(run_uuid)).await;
    fixtures::create_dataset_version(&db.pool, &ds, Some(run_uuid)).await;

    let ds_svc = DatasetService::new(db.pool.clone());
    let versions = ds_svc
        .list_versions(&ns.name, &ds.name, 10, 0)
        .await
        .unwrap();
    assert!(versions.len() >= 2);

    // At least one version should have the run populated
    let has_run = versions.iter().any(|v| v.run.is_some());
    assert!(
        has_run,
        "Expected at least one version to have createdByRun populated"
    );
    let with_run = versions.iter().find(|v| v.run.is_some()).unwrap();
    assert_eq!(with_run.run.as_ref().unwrap().args["target"], "warehouse");
}
