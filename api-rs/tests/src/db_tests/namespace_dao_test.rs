// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::generators;
use chrono::Utc;
use marquez_api::db::namespace;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();

    let row = namespace::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, &owner, None)
        .await
        .unwrap();

    assert_eq!(row.name, name);
    assert_eq!(row.current_owner_name.as_deref(), Some(owner.as_str()));
    assert!(row.description.is_none());
    assert_eq!(row.is_hidden, Some(false));
}

#[tokio::test]
async fn upsert_updates_on_conflict() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let desc = generators::new_description();

    let row1 = namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &owner,
        Some(&desc),
    )
    .await
    .unwrap();

    // Upsert again with description to trigger ON CONFLICT DO UPDATE.
    let row2 = namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &owner,
        Some(&desc),
    )
    .await
    .unwrap();

    // Same row (same uuid), but updated_at should have changed.
    assert_eq!(row1.uuid, row2.uuid);
    assert!(row2.updated_at >= row1.updated_at);
}

#[tokio::test]
async fn upsert_with_description() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let desc = generators::new_description();

    let row = namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &owner,
        Some(&desc),
    )
    .await
    .unwrap();

    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

#[tokio::test]
async fn upsert_without_description_does_not_overwrite() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let desc = generators::new_description();

    // First upsert WITH description.
    namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &owner,
        Some(&desc),
    )
    .await
    .unwrap();

    // Second upsert WITHOUT description (DO NOTHING path).
    let row = namespace::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, &owner, None)
        .await
        .unwrap();

    // Description should be preserved from the first upsert.
    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

#[tokio::test]
async fn exists_true() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();

    namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &generators::new_owner_name(),
        None,
    )
    .await
    .unwrap();

    assert!(namespace::exists(&db.pool, &name).await.unwrap());
}

#[tokio::test]
async fn exists_false() {
    let db = TestDb::new().await;
    assert!(!namespace::exists(&db.pool, "nonexistent_ns").await.unwrap());
}

#[tokio::test]
async fn find_by_name() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();

    namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &generators::new_owner_name(),
        None,
    )
    .await
    .unwrap();

    let found = namespace::find_by_name(&db.pool, &name).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, name);
}

#[tokio::test]
async fn find_by_name_not_found() {
    let db = TestDb::new().await;
    let found = namespace::find_by_name(&db.pool, "nonexistent_ns")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;

    // Create 3 namespaces.
    for _ in 0..3 {
        namespace::upsert(
            &db.pool,
            Uuid::new_v4(),
            Utc::now(),
            &generators::new_namespace_name(),
            &generators::new_owner_name(),
            None,
        )
        .await
        .unwrap();
    }

    let page1 = namespace::find_all(&db.pool, 2, 0).await.unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = namespace::find_all(&db.pool, 2, 2).await.unwrap();
    assert!(page2.len() >= 1, "expected at least 1 result on page 2");
}

#[tokio::test]
async fn delete_sets_hidden() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();

    namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &generators::new_owner_name(),
        None,
    )
    .await
    .unwrap();

    namespace::delete(&db.pool, &name).await.unwrap();

    let row = namespace::find_by_name(&db.pool, &name)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.is_hidden, Some(true));
}

#[tokio::test]
async fn undelete_clears_hidden() {
    let db = TestDb::new().await;
    let name = generators::new_namespace_name();

    namespace::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &name,
        &generators::new_owner_name(),
        None,
    )
    .await
    .unwrap();

    namespace::delete(&db.pool, &name).await.unwrap();
    let restored = namespace::undelete(&db.pool, &name).await.unwrap();

    assert!(restored.is_some());
    assert_eq!(restored.unwrap().is_hidden, Some(false));
}

#[tokio::test]
async fn upsert_owner() {
    let db = TestDb::new().await;
    let name = generators::new_owner_name();

    let row = namespace::upsert_owner(&db.pool, Uuid::new_v4(), Utc::now(), &name)
        .await
        .unwrap();

    assert_eq!(row.name, name);
    assert!(!row.uuid.is_nil());
}
