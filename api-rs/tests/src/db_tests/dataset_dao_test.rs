// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;
use chrono::Utc;
use marquez_api::db::{dataset, tag};
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let uuid = Uuid::new_v4();
    let name = generators::new_dataset_name();
    let physical_name = generators::new_physical_name();
    let now = Utc::now();

    let row = dataset::upsert(
        &db.pool,
        uuid,
        "DB_TABLE",
        now,
        ns.uuid,
        &ns.name,
        src.uuid,
        &src.name,
        &name,
        &physical_name,
        None,
        false,
    )
    .await
    .unwrap();

    assert_eq!(row.uuid, uuid);
    assert_eq!(row.type_, "DB_TABLE");
    assert_eq!(row.name, name);
    assert_eq!(row.physical_name, physical_name);
    assert_eq!(row.namespace_uuid, Some(ns.uuid));
    assert_eq!(row.namespace_name.as_deref(), Some(ns.name.as_str()));
    assert_eq!(row.source_uuid, Some(src.uuid));
    assert_eq!(row.source_name.as_deref(), Some(src.name.as_str()));
    assert!(row.description.is_none());
    assert_eq!(row.is_deleted, Some(false));
    assert_eq!(row.is_hidden, Some(false));
}

#[tokio::test]
async fn upsert_updates_on_conflict() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let uuid = Uuid::new_v4();
    let name = generators::new_dataset_name();
    let now = Utc::now();

    let row1 = dataset::upsert(
        &db.pool, uuid, "DB_TABLE", now, ns.uuid, &ns.name, src.uuid, &src.name, &name, "phys1",
        None, false,
    )
    .await
    .unwrap();

    // Upsert again with same UUID but different physical_name and type.
    let row2 = dataset::upsert(
        &db.pool,
        uuid,
        "STREAM",
        Utc::now(),
        ns.uuid,
        &ns.name,
        src.uuid,
        &src.name,
        &name,
        "phys2",
        None,
        false,
    )
    .await
    .unwrap();

    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row2.type_, "STREAM");
    assert_eq!(row2.physical_name, "phys2");
    assert!(row2.updated_at >= row1.updated_at);
}

#[tokio::test]
async fn upsert_with_description() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let desc = generators::new_description();

    let row = dataset::upsert(
        &db.pool,
        Uuid::new_v4(),
        "DB_TABLE",
        Utc::now(),
        ns.uuid,
        &ns.name,
        src.uuid,
        &src.name,
        &generators::new_dataset_name(),
        &generators::new_physical_name(),
        Some(&desc),
        false,
    )
    .await
    .unwrap();

    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

#[tokio::test]
async fn exists_true() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    assert!(dataset::exists(&db.pool, &ns.name, &ds.name).await.unwrap());
}

#[tokio::test]
async fn exists_false() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    assert!(!dataset::exists(&db.pool, &ns.name, "nonexistent_ds")
        .await
        .unwrap());
}

#[tokio::test]
async fn find_dataset_as_row() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let found = dataset::find_dataset_as_row(&db.pool, &ns.name, &ds.name)
        .await
        .unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.uuid, ds.uuid);
    assert_eq!(found.name, ds.name);
}

#[tokio::test]
async fn find_by_name() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let found = dataset::find_by_name(&db.pool, &ns.name, &ds.name)
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().uuid, ds.uuid);
}

#[tokio::test]
async fn find_by_name_not_found() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    let found = dataset::find_by_name(&db.pool, &ns.name, "nonexistent_ds")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    for _ in 0..3 {
        fixtures::create_dataset(&db.pool, &ns, &src).await;
    }

    let page1 = dataset::find_all(&db.pool, &ns.name, 2, 0).await.unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = dataset::find_all(&db.pool, &ns.name, 2, 2).await.unwrap();
    assert_eq!(page2.len(), 1);
}

#[tokio::test]
async fn find_all_empty() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    let result = dataset::find_all(&db.pool, &ns.name, 10, 0).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn count() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    fixtures::create_dataset(&db.pool, &ns, &src).await;
    fixtures::create_dataset(&db.pool, &ns, &src).await;

    let n = dataset::count(&db.pool, &ns.name).await.unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn count_empty() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    let n = dataset::count(&db.pool, &ns.name).await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn update_version() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let version_uuid = Uuid::new_v4();

    dataset::update_version(&db.pool, ds.uuid, version_uuid, Utc::now())
        .await
        .unwrap();

    let found = dataset::find_dataset_as_row(&db.pool, &ns.name, &ds.name)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.current_version_uuid, Some(version_uuid));
}

#[tokio::test]
async fn update_last_modified_at() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds1 = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let ds2 = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let modified_at = Utc::now();
    dataset::update_last_modified_at(&db.pool, &[ds1.uuid, ds2.uuid], modified_at)
        .await
        .unwrap();

    let found1 = dataset::find_dataset_as_row(&db.pool, &ns.name, &ds1.name)
        .await
        .unwrap()
        .unwrap();
    let found2 = dataset::find_dataset_as_row(&db.pool, &ns.name, &ds2.name)
        .await
        .unwrap()
        .unwrap();

    assert!(found1.last_modified_at.is_some());
    assert!(found2.last_modified_at.is_some());
}

#[tokio::test]
async fn update_tag_mapping() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let t = tag::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_tag_name(),
        None,
    )
    .await
    .unwrap();

    dataset::update_tag_mapping(&db.pool, ds.uuid, t.uuid, Utc::now())
        .await
        .unwrap();

    // Tagging again should not error (ON CONFLICT DO NOTHING).
    dataset::update_tag_mapping(&db.pool, ds.uuid, t.uuid, Utc::now())
        .await
        .unwrap();
}

#[tokio::test]
async fn delete_dataset_tag() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let tag_name = generators::new_tag_name();
    let t = tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &tag_name, None)
        .await
        .unwrap();

    dataset::update_tag_mapping(&db.pool, ds.uuid, t.uuid, Utc::now())
        .await
        .unwrap();

    // Remove the tag.
    dataset::delete_dataset_tag(&db.pool, &ns.name, &ds.name, &tag_name)
        .await
        .unwrap();

    // Deleting again should not error.
    dataset::delete_dataset_tag(&db.pool, &ns.name, &ds.name, &tag_name)
        .await
        .unwrap();
}

#[tokio::test]
async fn delete_sets_hidden() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let deleted = dataset::delete(&db.pool, &ns.name, &ds.name).await.unwrap();
    assert!(deleted.is_some());
    let deleted = deleted.unwrap();
    assert_eq!(deleted.uuid, ds.uuid);
    assert_eq!(deleted.is_hidden, Some(true));

    // Should no longer appear in datasets_view (hidden).
    assert!(!dataset::exists(&db.pool, &ns.name, &ds.name).await.unwrap());
}
