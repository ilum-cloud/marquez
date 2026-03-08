// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use chrono::Utc;
use marquez_api::db::column_lineage;

#[tokio::test]
async fn upsert_creates() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    // Create output dataset + version + field
    let out_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let out_dv = fixtures::create_dataset_version(&db.pool, &out_ds, None).await;
    let out_field = fixtures::create_dataset_field(&db.pool, &out_ds).await;

    // Create input dataset + version + field
    let in_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let in_dv = fixtures::create_dataset_version(&db.pool, &in_ds, None).await;
    let in_field = fixtures::create_dataset_field(&db.pool, &in_ds).await;

    column_lineage::upsert(
        &db.pool,
        out_dv.uuid,
        out_field.uuid,
        in_dv.uuid,
        in_field.uuid,
        Some("identity"),
        Some("IDENTITY"),
        Utc::now(),
    )
    .await
    .unwrap();

    // Verify
    let rows = column_lineage::find_by_output_dataset_version(&db.pool, out_dv.uuid)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].output_dataset_field_uuid, Some(out_field.uuid));
    assert_eq!(rows[0].input_dataset_field_uuid, Some(in_field.uuid));
    assert_eq!(
        rows[0].transformation_description.as_deref(),
        Some("identity")
    );
    assert_eq!(rows[0].transformation_type.as_deref(), Some("IDENTITY"));
}

#[tokio::test]
async fn upsert_updates_on_conflict() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    let out_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let out_dv = fixtures::create_dataset_version(&db.pool, &out_ds, None).await;
    let out_field = fixtures::create_dataset_field(&db.pool, &out_ds).await;

    let in_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let in_dv = fixtures::create_dataset_version(&db.pool, &in_ds, None).await;
    let in_field = fixtures::create_dataset_field(&db.pool, &in_ds).await;

    // First insert
    column_lineage::upsert(
        &db.pool,
        out_dv.uuid,
        out_field.uuid,
        in_dv.uuid,
        in_field.uuid,
        Some("original"),
        Some("IDENTITY"),
        Utc::now(),
    )
    .await
    .unwrap();

    // Second upsert with updated description
    column_lineage::upsert(
        &db.pool,
        out_dv.uuid,
        out_field.uuid,
        in_dv.uuid,
        in_field.uuid,
        Some("updated"),
        Some("TRANSFORM"),
        Utc::now(),
    )
    .await
    .unwrap();

    let rows = column_lineage::find_by_output_dataset_version(&db.pool, out_dv.uuid)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].transformation_description.as_deref(),
        Some("updated")
    );
    assert_eq!(rows[0].transformation_type.as_deref(), Some("TRANSFORM"));
}

#[tokio::test]
async fn get_lineage_simple() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    let out_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let out_dv = fixtures::create_dataset_version(&db.pool, &out_ds, None).await;
    let out_field = fixtures::create_dataset_field(&db.pool, &out_ds).await;

    let in_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let in_dv = fixtures::create_dataset_version(&db.pool, &in_ds, None).await;
    let in_field = fixtures::create_dataset_field(&db.pool, &in_ds).await;

    column_lineage::upsert(
        &db.pool,
        out_dv.uuid,
        out_field.uuid,
        in_dv.uuid,
        in_field.uuid,
        None,
        None,
        Utc::now(),
    )
    .await
    .unwrap();

    // Get upstream lineage from the output field
    let result = column_lineage::get_lineage(&db.pool, 5, &[out_field.uuid], false, Utc::now())
        .await
        .unwrap();
    assert!(!result.is_empty(), "Column lineage should have results");
    assert_eq!(result.len(), 1);
}

#[tokio::test]
async fn find_by_input_dataset_version() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let src = fixtures::create_source(&db.pool).await;

    let out_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let out_dv = fixtures::create_dataset_version(&db.pool, &out_ds, None).await;
    let out_field = fixtures::create_dataset_field(&db.pool, &out_ds).await;

    let in_ds = fixtures::create_dataset(&db.pool, &ns, &src).await;
    let in_dv = fixtures::create_dataset_version(&db.pool, &in_ds, None).await;
    let in_field = fixtures::create_dataset_field(&db.pool, &in_ds).await;

    column_lineage::upsert(
        &db.pool,
        out_dv.uuid,
        out_field.uuid,
        in_dv.uuid,
        in_field.uuid,
        None,
        None,
        Utc::now(),
    )
    .await
    .unwrap();

    let rows = column_lineage::find_by_input_dataset_version(&db.pool, in_dv.uuid)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].output_dataset_version_uuid, Some(out_dv.uuid));
}
