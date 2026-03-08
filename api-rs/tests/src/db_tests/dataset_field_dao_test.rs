// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;
use chrono::Utc;
use marquez_api::db::{dataset_field, tag};
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let field_name = generators::new_field_name();

    let row = dataset_field::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &field_name,
        Some("VARCHAR"),
        Some("a test field"),
        ds.uuid,
    )
    .await
    .unwrap();

    assert_eq!(row.name, field_name);
    assert_eq!(row.type_.as_deref(), Some("VARCHAR"));
    assert_eq!(row.description.as_deref(), Some("a test field"));
    assert_eq!(row.dataset_uuid, Some(ds.uuid));
}

#[tokio::test]
async fn upsert_updates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let field_name = generators::new_field_name();

    let row1 = dataset_field::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &field_name,
        Some("VARCHAR"),
        Some("desc1"),
        ds.uuid,
    )
    .await
    .unwrap();

    let row2 = dataset_field::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &field_name,
        Some("VARCHAR"),
        Some("desc2"),
        ds.uuid,
    )
    .await
    .unwrap();

    // Same row (conflict on dataset_uuid, name, type).
    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row2.description.as_deref(), Some("desc2"));
    assert!(row2.updated_at >= row1.updated_at);
}

#[tokio::test]
async fn upsert_type_defaults_to_unknown() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let row = dataset_field::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_field_name(),
        None, // NULL type => COALESCE to 'UNKNOWN'
        None,
        ds.uuid,
    )
    .await
    .unwrap();

    assert_eq!(row.type_.as_deref(), Some("UNKNOWN"));
}

#[tokio::test]
async fn exists_true() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let field = fixtures::create_dataset_field(&db.pool, &ds).await;

    assert!(dataset_field::exists(&db.pool, ds.uuid, &field.name)
        .await
        .unwrap());
}

#[tokio::test]
async fn exists_false() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    assert!(
        !dataset_field::exists(&db.pool, ds.uuid, "nonexistent_field")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn find_uuid() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let field = fixtures::create_dataset_field(&db.pool, &ds).await;

    let found = dataset_field::find_uuid(&db.pool, ds.uuid, &field.name)
        .await
        .unwrap();
    assert_eq!(found, Some(field.uuid));
}

#[tokio::test]
async fn find_uuid_not_found() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let found = dataset_field::find_uuid(&db.pool, ds.uuid, "nonexistent")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_by_dataset_version() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;

    let f1 = fixtures::create_dataset_field(&db.pool, &ds).await;
    let f2 = fixtures::create_dataset_field(&db.pool, &ds).await;

    let dv = fixtures::create_dataset_version(&db.pool, &ds, None).await;

    // Map fields to the version.
    dataset_field::update_field_mapping(&db.pool, dv.uuid, &[f1.uuid, f2.uuid])
        .await
        .unwrap();

    let fields = dataset_field::find_by_dataset_version(&db.pool, dv.uuid)
        .await
        .unwrap();
    assert_eq!(fields.len(), 2);

    let uuids: Vec<_> = fields.iter().map(|f| f.uuid).collect();
    assert!(uuids.contains(&f1.uuid));
    assert!(uuids.contains(&f2.uuid));
}

#[tokio::test]
async fn update_field_mapping() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let f1 = fixtures::create_dataset_field(&db.pool, &ds).await;
    let dv = fixtures::create_dataset_version(&db.pool, &ds, None).await;

    dataset_field::update_field_mapping(&db.pool, dv.uuid, &[f1.uuid])
        .await
        .unwrap();

    // Calling again should not error (ON CONFLICT DO NOTHING).
    dataset_field::update_field_mapping(&db.pool, dv.uuid, &[f1.uuid])
        .await
        .unwrap();

    let fields = dataset_field::find_by_dataset_version(&db.pool, dv.uuid)
        .await
        .unwrap();
    assert_eq!(fields.len(), 1);
}

#[tokio::test]
async fn update_tags() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;
    let ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let field = fixtures::create_dataset_field(&db.pool, &ds).await;
    let t = tag::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_tag_name(),
        None,
    )
    .await
    .unwrap();

    dataset_field::update_tags(&db.pool, field.uuid, t.uuid, Utc::now())
        .await
        .unwrap();

    // Tagging again should not error (ON CONFLICT DO NOTHING).
    dataset_field::update_tags(&db.pool, field.uuid, t.uuid, Utc::now())
        .await
        .unwrap();
}
