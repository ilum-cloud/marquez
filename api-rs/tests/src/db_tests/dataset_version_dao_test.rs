// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;
use chrono::Utc;
use marquez_api::db::dataset_version;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let uuid = Uuid::new_v4();
    let version = generators::new_version();

    let row = dataset_version::upsert(
        &db.pool,
        uuid,
        Utc::now(),
        ds.uuid,
        version,
        None,
        None,
        None,
        &ns.name,
        &ds.name,
        None,
    )
    .await
    .unwrap();

    assert_eq!(row.uuid, uuid);
    assert_eq!(row.dataset_uuid, Some(ds.uuid));
    assert_eq!(row.version, version);
    assert!(row.run_uuid.is_none());
    assert_eq!(row.namespace_name.as_deref(), Some(ns.name.as_str()));
    assert_eq!(row.dataset_name.as_deref(), Some(ds.name.as_str()));
}

#[tokio::test]
async fn upsert_updates_run_uuid_on_conflict() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let version = generators::new_version();
    let run1 = Uuid::new_v4();
    let run2 = Uuid::new_v4();

    let row1 = dataset_version::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        ds.uuid,
        version,
        None,
        Some(run1),
        None,
        &ns.name,
        &ds.name,
        None,
    )
    .await
    .unwrap();

    // Same version => conflict => update run_uuid.
    let row2 = dataset_version::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        ds.uuid,
        version,
        None,
        Some(run2),
        None,
        &ns.name,
        &ds.name,
        None,
    )
    .await
    .unwrap();

    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row2.run_uuid, Some(run2));
}

#[tokio::test]
async fn find_by_uuid() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let dv = fixtures::create_dataset_version(&db.pool, &ds, None).await;

    let found = dataset_version::find_by_uuid(&db.pool, dv.uuid)
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().uuid, dv.uuid);
}

#[tokio::test]
async fn find_by_uuid_not_found() {
    let db = TestDb::new().await;

    let found = dataset_version::find_by_uuid(&db.pool, Uuid::new_v4())
        .await
        .unwrap();
    assert!(found.is_none());
}

/// Bug 62: find_all now returns EnrichedDatasetVersionRow with tags, facets, etc.
#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    for _ in 0..3 {
        fixtures::create_dataset_version(&db.pool, &ds, None).await;
    }

    let page1 = dataset_version::find_all(&db.pool, &ns.name, &ds.name, 2, 0)
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);
    // Verify enriched fields are present (Bug 62)
    for row in &page1 {
        assert!(row.name.is_some(), "enriched row should have name");
        assert!(
            row.namespace_name.is_some(),
            "enriched row should have namespace_name"
        );
    }

    let page2 = dataset_version::find_all(&db.pool, &ns.name, &ds.name, 2, 2)
        .await
        .unwrap();
    assert_eq!(page2.len(), 1);
}

#[tokio::test]
async fn find_input_versions_for_run() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let dv = fixtures::create_dataset_version(&db.pool, &ds, None).await;
    let run_uuid = Uuid::new_v4();

    // Create a runs_input_mapping entry.
    // First we need a run row. For simplicity, insert a minimal run directly.
    sqlx::query("INSERT INTO runs (uuid, created_at, updated_at) VALUES ($1, $2, $2)")
        .bind(run_uuid)
        .bind(Utc::now())
        .execute(&db.pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO runs_input_mapping (run_uuid, dataset_version_uuid) \
         VALUES ($1, $2)",
    )
    .bind(run_uuid)
    .bind(dv.uuid)
    .execute(&db.pool)
    .await
    .unwrap();

    let inputs = dataset_version::find_input_versions_for(&db.pool, run_uuid)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].uuid, dv.uuid);
}

#[tokio::test]
async fn find_output_versions_for_run() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let run_uuid = Uuid::new_v4();

    let dv = dataset_version::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        ds.uuid,
        generators::new_version(),
        None,
        Some(run_uuid),
        None,
        &ns.name,
        &ds.name,
        None,
    )
    .await
    .unwrap();

    let outputs = dataset_version::find_output_versions_for(&db.pool, run_uuid)
        .await
        .unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].uuid, dv.uuid);
}

#[tokio::test]
async fn count() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    fixtures::create_dataset_version(&db.pool, &ds, None).await;
    fixtures::create_dataset_version(&db.pool, &ds, None).await;

    let n = dataset_version::count(&db.pool, &ns.name, &ds.name)
        .await
        .unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn upsert_symlink() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let symlink_name = generators::new_dataset_name();

    let row = dataset_version::upsert_symlink(
        &db.pool,
        ds.uuid,
        &symlink_name,
        ns.uuid,
        None,
        false,
        Utc::now(),
    )
    .await
    .unwrap();

    assert_eq!(row.dataset_uuid, Some(ds.uuid));
    assert_eq!(row.name, symlink_name);
    assert_eq!(row.namespace_uuid, Some(ns.uuid));
}

#[tokio::test]
async fn upsert_symlink_idempotent() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let symlink_name = generators::new_dataset_name();

    let row1 = dataset_version::upsert_symlink(
        &db.pool,
        ds.uuid,
        &symlink_name,
        ns.uuid,
        None,
        false,
        Utc::now(),
    )
    .await
    .unwrap();

    // Same name + namespace => ON CONFLICT DO NOTHING => returns same row.
    let row2 = dataset_version::upsert_symlink(
        &db.pool,
        ds.uuid,
        &symlink_name,
        ns.uuid,
        None,
        false,
        Utc::now(),
    )
    .await
    .unwrap();

    assert_eq!(row1.name, row2.name);
    assert_eq!(row1.namespace_uuid, row2.namespace_uuid);
    assert_eq!(row1.dataset_uuid, row2.dataset_uuid);
}

#[tokio::test]
async fn find_symlink_by_namespace_and_name() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let symlink_name = generators::new_dataset_name();

    let created = fixtures::create_symlink(&db.pool, &symlink_name, ns.uuid).await;

    let found =
        dataset_version::find_symlink_by_namespace_and_name(&db.pool, ns.uuid, &symlink_name)
            .await
            .unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.name, created.name);
    assert_eq!(found.namespace_uuid, created.namespace_uuid);
}

#[tokio::test]
async fn upsert_schema_version() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let sv = fixtures::create_schema_version(&db.pool, &ds).await;
    assert_eq!(sv.dataset_uuid, Some(ds.uuid));
}

#[tokio::test]
async fn upsert_schema_field_mappings() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let sv = fixtures::create_schema_version(&db.pool, &ds).await;
    let f1 = fixtures::create_dataset_field(&db.pool, &ds).await;
    let f2 = fixtures::create_dataset_field(&db.pool, &ds).await;

    dataset_version::upsert_schema_field_mappings(&db.pool, sv.uuid, &[f1.uuid, f2.uuid])
        .await
        .unwrap();

    // Calling again should not error (ON CONFLICT DO NOTHING).
    dataset_version::upsert_schema_field_mappings(&db.pool, sv.uuid, &[f1.uuid])
        .await
        .unwrap();
}
