use chrono::Utc;
use uuid::Uuid;

use crate::common::TestDb;
use marquez_api::db;
use marquez_api::models::api::NodeId;
use marquez_api::models::db::{
    DatasetFieldRow, DatasetRow, DatasetVersionRow, NamespaceRow, SourceRow,
};
use marquez_api::service::column_lineage::ColumnLineageService;

/// Create a namespace with a simple (colon-free) name for node ID parsing.
async fn create_simple_namespace(pool: &sqlx::PgPool) -> NamespaceRow {
    db::namespace::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &format!("test-ns-{}", rand::random_range(0..u32::MAX)),
        "test-owner",
        None,
    )
    .await
    .unwrap()
}

/// Create a source.
async fn create_source(pool: &sqlx::PgPool) -> SourceRow {
    db::source::upsert(
        pool,
        Uuid::new_v4(),
        "POSTGRESQL",
        Utc::now(),
        &format!("test-src-{}", rand::random_range(0..u32::MAX)),
        "postgresql://localhost:5432/test",
        None,
    )
    .await
    .unwrap()
}

/// Create a dataset with a symlink (so datasets_view can find it).
async fn create_dataset(pool: &sqlx::PgPool, ns: &NamespaceRow, src: &SourceRow) -> DatasetRow {
    let uuid = Uuid::new_v4();
    let name = format!("test-dataset-{}", rand::random_range(0..u32::MAX));
    let now = Utc::now();
    let row = db::dataset::upsert(
        pool, uuid, "DB_TABLE", now, ns.uuid, &ns.name, src.uuid, &src.name, &name, &name, None,
        false,
    )
    .await
    .unwrap();
    db::dataset_version::upsert_symlink(pool, uuid, &name, ns.uuid, None, true, now)
        .await
        .unwrap();
    row
}

/// Create a dataset field.
async fn create_field(pool: &sqlx::PgPool, ds: &DatasetRow) -> DatasetFieldRow {
    db::dataset_field::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &format!("field-{}", rand::random_range(0..u32::MAX)),
        Some("VARCHAR"),
        None,
        ds.uuid,
    )
    .await
    .unwrap()
}

/// Create a dataset version.
async fn create_version(pool: &sqlx::PgPool, ds: &DatasetRow) -> DatasetVersionRow {
    db::dataset_version::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        ds.uuid,
        Uuid::new_v4(),
        None,
        None,
        None,
        ds.namespace_name.as_deref().unwrap_or("default"),
        &ds.name,
        None,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn get_empty_lineage_returns_not_found() {
    let db = TestDb::new().await;
    let ns = create_simple_namespace(&db.pool).await;
    let src = create_source(&db.pool).await;
    let ds = create_dataset(&db.pool, &ns, &src).await;

    let dv = create_version(&db.pool, &ds).await;
    let field = create_field(&db.pool, &ds).await;
    // Do NOT create field mapping — so find_dataset_fields_uuids returns empty
    // (the field exists but has no version mapping)
    let _ = (dv, field); // suppress unused warnings

    let svc = ColumnLineageService::new(db.pool.clone());
    let node_id = NodeId::new(format!("dataset:{}:{}", ns.name, ds.name));
    // With no field mappings, the service should return NotFound
    let result = svc.get_lineage(&node_id, 10, false, Utc::now()).await;
    assert!(
        result.is_err(),
        "Expected error when no field mappings exist"
    );
}

#[tokio::test]
async fn get_empty_lineage_with_field_mapping() {
    let db = TestDb::new().await;
    let ns = create_simple_namespace(&db.pool).await;
    let src = create_source(&db.pool).await;
    let ds = create_dataset(&db.pool, &ns, &src).await;

    let dv = create_version(&db.pool, &ds).await;
    let field = create_field(&db.pool, &ds).await;
    db::dataset_field::update_field_mapping(&db.pool, dv.uuid, &[field.uuid])
        .await
        .unwrap();
    db::dataset::update_version(&db.pool, ds.uuid, dv.uuid, Utc::now())
        .await
        .unwrap();

    let svc = ColumnLineageService::new(db.pool.clone());
    let node_id = NodeId::new(format!("dataset:{}:{}", ns.name, ds.name));
    let lineage = svc
        .get_lineage(&node_id, 10, false, Utc::now())
        .await
        .unwrap();
    // Fields exist but no column lineage rows, so graph is empty
    assert!(lineage.graph.is_empty());
}

#[tokio::test]
async fn get_upstream_lineage() {
    let db = TestDb::new().await;
    let ns = create_simple_namespace(&db.pool).await;
    let src = create_source(&db.pool).await;

    let ds_in = create_dataset(&db.pool, &ns, &src).await;
    let ds_out = create_dataset(&db.pool, &ns, &src).await;
    let dv_in = create_version(&db.pool, &ds_in).await;
    let dv_out = create_version(&db.pool, &ds_out).await;
    let field_in = create_field(&db.pool, &ds_in).await;
    let field_out = create_field(&db.pool, &ds_out).await;

    db::dataset_field::update_field_mapping(&db.pool, dv_in.uuid, &[field_in.uuid])
        .await
        .unwrap();
    db::dataset_field::update_field_mapping(&db.pool, dv_out.uuid, &[field_out.uuid])
        .await
        .unwrap();
    db::dataset::update_version(&db.pool, ds_out.uuid, dv_out.uuid, Utc::now())
        .await
        .unwrap();

    // Create column lineage: field_out derives from field_in
    db::column_lineage::upsert(
        &db.pool,
        dv_out.uuid,
        field_out.uuid,
        dv_in.uuid,
        field_in.uuid,
        Some("identity"),
        Some("IDENTITY"),
        Utc::now(),
    )
    .await
    .unwrap();

    let svc = ColumnLineageService::new(db.pool.clone());
    let node_id = NodeId::new(format!("dataset:{}:{}", ns.name, ds_out.name));
    let lineage = svc
        .get_lineage(&node_id, 10, false, Utc::now())
        .await
        .unwrap();
    assert!(!lineage.graph.is_empty(), "Expected column lineage nodes");

    // Verify nodes have correct structure
    for node in &lineage.graph {
        assert_eq!(node.type_.to_string(), "DATASET_FIELD");
        assert!(node.id.value().starts_with("datasetField:"));
        assert!(node.data.is_some());
    }

    // Find the output node and verify it has inEdges
    let output_node = lineage.graph.iter().find(|n| {
        n.data
            .as_ref()
            .and_then(|d| d.get("field"))
            .and_then(|f| f.as_str())
            .map(|f| f == field_out.name)
            .unwrap_or(false)
    });
    assert!(output_node.is_some(), "Expected output field node");
    let output_node = output_node.unwrap();
    assert!(
        !output_node.in_edges.is_empty(),
        "Expected inEdges on output node"
    );

    // Find the input node and verify it has outEdges
    let input_node = lineage.graph.iter().find(|n| {
        n.data
            .as_ref()
            .and_then(|d| d.get("field"))
            .and_then(|f| f.as_str())
            .map(|f| f == field_in.name)
            .unwrap_or(false)
    });
    assert!(input_node.is_some(), "Expected input field node");
    let input_node = input_node.unwrap();
    assert!(
        !input_node.out_edges.is_empty(),
        "Expected outEdges on input node"
    );
}

#[tokio::test]
async fn enrich_dataset_no_version() {
    let db = TestDb::new().await;
    let svc = ColumnLineageService::new(db.pool.clone());
    let mut dataset = marquez_api::models::api::Dataset {
        id: marquez_api::models::common::DatasetId {
            namespace: marquez_api::models::common::NamespaceName::new("ns"),
            name: marquez_api::models::common::DatasetName::new("ds"),
        },
        type_: marquez_api::models::common::DatasetType::DbTable,
        name: "ds".to_string(),
        physical_name: "ds".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        namespace: "ns".to_string(),
        source_name: "src".to_string(),
        fields: vec![],
        tags: vec![],
        last_modified_at: None,
        description: None,
        current_version: None,
        facets: serde_json::json!({}),
        is_deleted: false,
        column_lineage: serde_json::Value::Null,
        last_lifecycle_state: None,
    };
    // Should succeed even with no current version
    svc.enrich_dataset_with_column_lineage(&mut dataset)
        .await
        .unwrap();
}

#[tokio::test]
async fn invalid_node_id() {
    let db = TestDb::new().await;
    let svc = ColumnLineageService::new(db.pool.clone());
    let result = svc
        .get_lineage(&NodeId::new("invalid"), 10, false, Utc::now())
        .await;
    assert!(result.is_err());
}
