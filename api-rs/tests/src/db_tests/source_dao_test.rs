// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::generators;
use chrono::Utc;
use marquez_api::db::source;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let name = generators::new_source_name();
    let conn = generators::new_connection_url();

    let row = source::upsert(
        &db.pool,
        Uuid::new_v4(),
        &generators::new_source_type(),
        Utc::now(),
        &name,
        &conn,
        None,
    )
    .await
    .unwrap();

    assert_eq!(row.name, name);
    assert_eq!(row.type_, "POSTGRESQL");
    assert_eq!(row.connection_url, conn);
    assert!(row.description.is_none());
}

#[tokio::test]
async fn upsert_updates() {
    let db = TestDb::new().await;
    let name = generators::new_source_name();
    let conn1 = generators::new_connection_url();
    let conn2 = generators::new_connection_url();

    let row1 = source::upsert(
        &db.pool,
        Uuid::new_v4(),
        "POSTGRESQL",
        Utc::now(),
        &name,
        &conn1,
        None,
    )
    .await
    .unwrap();

    let row2 = source::upsert(
        &db.pool,
        Uuid::new_v4(),
        "POSTGRESQL",
        Utc::now(),
        &name,
        &conn2,
        None,
    )
    .await
    .unwrap();

    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row2.connection_url, conn2);
    assert!(row2.updated_at >= row1.updated_at);
}

#[tokio::test]
async fn upsert_with_description() {
    let db = TestDb::new().await;
    let name = generators::new_source_name();
    let desc = generators::new_description();

    let row = source::upsert(
        &db.pool,
        Uuid::new_v4(),
        "POSTGRESQL",
        Utc::now(),
        &name,
        &generators::new_connection_url(),
        Some(&desc),
    )
    .await
    .unwrap();

    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

#[tokio::test]
async fn exists_true() {
    let db = TestDb::new().await;
    let name = generators::new_source_name();

    source::upsert(
        &db.pool,
        Uuid::new_v4(),
        "POSTGRESQL",
        Utc::now(),
        &name,
        &generators::new_connection_url(),
        None,
    )
    .await
    .unwrap();

    assert!(source::exists(&db.pool, &name).await.unwrap());
}

#[tokio::test]
async fn exists_false() {
    let db = TestDb::new().await;
    assert!(!source::exists(&db.pool, "nonexistent_source")
        .await
        .unwrap());
}

#[tokio::test]
async fn find_by_name() {
    let db = TestDb::new().await;
    let name = generators::new_source_name();

    source::upsert(
        &db.pool,
        Uuid::new_v4(),
        "POSTGRESQL",
        Utc::now(),
        &name,
        &generators::new_connection_url(),
        None,
    )
    .await
    .unwrap();

    let found = source::find_by_name(&db.pool, &name).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, name);
}

#[tokio::test]
async fn find_by_name_not_found() {
    let db = TestDb::new().await;
    let found = source::find_by_name(&db.pool, "nonexistent_source")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;

    for _ in 0..3 {
        source::upsert(
            &db.pool,
            Uuid::new_v4(),
            "POSTGRESQL",
            Utc::now(),
            &generators::new_source_name(),
            &generators::new_connection_url(),
            None,
        )
        .await
        .unwrap();
    }

    let page1 = source::find_all(&db.pool, 2, 0).await.unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = source::find_all(&db.pool, 2, 2).await.unwrap();
    assert!(page2.len() >= 1, "expected at least 1 result on page 2");
}
