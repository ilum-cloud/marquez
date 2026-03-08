use crate::common::TestDb;
use crate::fixtures;
use marquez_api::service::search::SearchService;

#[tokio::test]
async fn search_empty() {
    let db = TestDb::new().await;
    let svc = SearchService::new(db.pool.clone());
    let results = svc
        .search("nonexistent", None, None, 10, None, None, None)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn search_datasets() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let svc = SearchService::new(db.pool.clone());
    let results = svc
        .search(&ds.name, Some("DATASET"), None, 10, None, None, None)
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(
        results[0].type_,
        marquez_api::models::api::SearchResultType::Dataset
    );
}

#[tokio::test]
async fn search_jobs() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let job = fixtures::create_job(&db.pool, &ns).await;

    let svc = SearchService::new(db.pool.clone());
    let results = svc
        .search(&job.name, Some("JOB"), None, 10, None, None, None)
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(
        results[0].type_,
        marquez_api::models::api::SearchResultType::Job
    );
}

#[tokio::test]
async fn search_both() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    // Use a common prefix so search finds both
    let common_prefix = format!("findme_{}", rand::random_range::<u32, _>(0..u32::MAX));

    // Create a dataset with custom name
    let ds_uuid = uuid::Uuid::new_v4();
    let ds_name = format!("{}_dataset", common_prefix);
    marquez_api::db::dataset::upsert(
        &db.pool,
        ds_uuid,
        "DB_TABLE",
        chrono::Utc::now(),
        ns.uuid,
        &ns.name,
        src.uuid,
        &src.name,
        &ds_name,
        &ds_name,
        None,
        false,
    )
    .await
    .unwrap();
    // Create symlink for datasets_view (dataset_uuid must match the dataset row)
    marquez_api::db::dataset_version::upsert_symlink(
        &db.pool,
        ds_uuid,
        &ds_name,
        ns.uuid,
        None,
        true,
        chrono::Utc::now(),
    )
    .await
    .unwrap();

    // Create a job with custom name
    marquez_api::db::job::upsert(
        &db.pool,
        uuid::Uuid::new_v4(),
        "BATCH",
        chrono::Utc::now(),
        ns.uuid,
        &ns.name,
        &format!("{}_job", common_prefix),
        None,
        None,
        None,
        Some(&format!("{}_job", common_prefix)),
        None,
        None,
    )
    .await
    .unwrap();

    let svc = SearchService::new(db.pool.clone());
    let results = svc
        .search(&common_prefix, None, None, 10, None, None, None)
        .await
        .unwrap();
    assert!(
        results.len() >= 2,
        "expected at least 2 results, got {}",
        results.len()
    );
}
