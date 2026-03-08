// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::generators;
use chrono::Utc;
use marquez_api::db::tag;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let name = generators::new_tag_name();

    let row = tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, None)
        .await
        .unwrap();

    assert_eq!(row.name, name);
    assert!(row.description.is_none());
}

#[tokio::test]
async fn upsert_with_description() {
    let db = TestDb::new().await;
    let name = generators::new_tag_name();
    let desc = generators::new_description();

    let row = tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, Some(&desc))
        .await
        .unwrap();

    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

#[tokio::test]
async fn upsert_updates() {
    let db = TestDb::new().await;
    let name = generators::new_tag_name();
    let desc1 = generators::new_description();
    let desc2 = generators::new_description();

    let row1 = tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, Some(&desc1))
        .await
        .unwrap();

    let row2 = tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, Some(&desc2))
        .await
        .unwrap();

    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row2.description.as_deref(), Some(desc2.as_str()));
    assert!(row2.updated_at >= row1.updated_at);
}

#[tokio::test]
async fn exists_true() {
    let db = TestDb::new().await;
    let name = generators::new_tag_name();

    tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, None)
        .await
        .unwrap();

    assert!(tag::exists(&db.pool, &name).await.unwrap());
}

#[tokio::test]
async fn exists_false() {
    let db = TestDb::new().await;
    assert!(!tag::exists(&db.pool, "nonexistent_tag").await.unwrap());
}

#[tokio::test]
async fn find_by_name() {
    let db = TestDb::new().await;
    let name = generators::new_tag_name();

    tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &name, None)
        .await
        .unwrap();

    let found = tag::find_by_name(&db.pool, &name).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, name);
}

#[tokio::test]
async fn find_by_name_not_found() {
    let db = TestDb::new().await;
    let found = tag::find_by_name(&db.pool, "nonexistent_tag")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;

    for _ in 0..3 {
        tag::upsert(
            &db.pool,
            Uuid::new_v4(),
            Utc::now(),
            &generators::new_tag_name(),
            None,
        )
        .await
        .unwrap();
    }

    let page1 = tag::find_all(&db.pool, 2, 0).await.unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = tag::find_all(&db.pool, 2, 2).await.unwrap();
    assert!(page2.len() >= 1, "expected at least 1 result on page 2");
}
