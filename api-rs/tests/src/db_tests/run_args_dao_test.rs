// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::generators;
use chrono::Utc;
use marquez_api::db::run_args;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let args = r#"{"key": "value"}"#;
    let checksum = generators::new_checksum();

    let row = run_args::upsert(&db.pool, Uuid::new_v4(), Utc::now(), args, &checksum)
        .await
        .unwrap();

    assert_eq!(row.args, args);
    assert_eq!(row.checksum, checksum);
}

#[tokio::test]
async fn upsert_idempotent_on_checksum() {
    let db = TestDb::new().await;
    let args = r#"{"key": "value"}"#;
    let checksum = generators::new_checksum();

    let row1 = run_args::upsert(&db.pool, Uuid::new_v4(), Utc::now(), args, &checksum)
        .await
        .unwrap();

    let row2 = run_args::upsert(&db.pool, Uuid::new_v4(), Utc::now(), args, &checksum)
        .await
        .unwrap();

    // Same checksum returns the same row (same uuid).
    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row1.checksum, row2.checksum);
}

#[tokio::test]
async fn find_by_checksum() {
    let db = TestDb::new().await;
    let args = r#"{"foo": "bar"}"#;
    let checksum = generators::new_checksum();

    run_args::upsert(&db.pool, Uuid::new_v4(), Utc::now(), args, &checksum)
        .await
        .unwrap();

    let found = run_args::find_by_checksum(&db.pool, &checksum)
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().checksum, checksum);
}

#[tokio::test]
async fn find_by_checksum_not_found() {
    let db = TestDb::new().await;
    let found = run_args::find_by_checksum(&db.pool, "nonexistent_checksum")
        .await
        .unwrap();
    assert!(found.is_none());
}
