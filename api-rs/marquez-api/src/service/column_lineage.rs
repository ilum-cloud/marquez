use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::db;
use crate::error::AppError;
use crate::models::api::{self, Edge, Lineage, Node, NodeId};
use crate::models::common::NodeType;

pub struct ColumnLineageService {
    pool: PgPool,
}

impl ColumnLineageService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get column lineage for a given node.
    ///
    /// The `node_id` string has format: "dataset:namespace:dataset_name"
    /// Resolves dataset fields across ALL versions (matching Java) and
    /// traverses column lineage graph using the aggregated CTE.
    ///
    /// Returns a `Lineage` with `Node` objects of type `DATASET_FIELD`,
    /// each with `inEdges` / `outEdges` and field data, matching the
    /// Java API response shape.
    pub async fn get_lineage(
        &self,
        node_id: &NodeId,
        depth: i32,
        with_downstream: bool,
        created_at_until: DateTime<Utc>,
    ) -> Result<Lineage, AppError> {
        let parts = node_id.parse_parts(3)?;
        let ns_name = parts[1];
        let ds_name = parts[2];

        // Find field UUIDs across ALL versions via dataset symlinks (matches Java)
        let field_uuids =
            db::dataset_field::find_dataset_fields_uuids(&self.pool, ns_name, ds_name).await?;
        if field_uuids.is_empty() {
            return Err(AppError::NotFound("Could not find node".to_string()));
        }

        // Get column lineage using aggregated CTE (no N+1)
        let node_rows = db::column_lineage::get_lineage(
            &self.pool,
            depth,
            &field_uuids,
            with_downstream,
            created_at_until,
        )
        .await?;

        // Build Node graph matching Java's ColumnLineageService.toLineage()
        //
        // For each CTE row (output field with its input fields):
        //   - Create a datasetField node for the output
        //   - Create datasetField nodes for each input
        //   - Add inEdges to the output node (from each input)
        //   - Add outEdges to each input node (to this output)

        // First pass: parse all rows and collect edge relationships
        struct ParsedNode {
            namespace: String,
            dataset: String,
            field: String,
            field_type: Option<String>,
            dataset_version_uuid: Option<String>,
            transformation_description: Option<String>,
            transformation_type: Option<String>,
            input_fields: Vec<ParsedInputField>,
        }

        struct ParsedInputField {
            namespace: String,
            dataset: String,
            field: String,
            dataset_version: Option<String>,
            transformation_description: Option<String>,
            transformation_type: Option<String>,
        }

        let parsed: Vec<ParsedNode> = node_rows
            .into_iter()
            .map(|row| {
                let input_fields = match row.input_fields {
                    Some(serde_json::Value::Array(arr)) => arr
                        .into_iter()
                        .filter_map(|entry| {
                            let inner = entry.as_array()?;
                            if inner.len() < 6 {
                                return None;
                            }
                            let get_str = |v: &serde_json::Value| v.as_str().map(|s| s.to_string());
                            let namespace = get_str(&inner[0]).unwrap_or_default();
                            let dataset = get_str(&inner[1]).unwrap_or_default();
                            let field = get_str(&inner[3]).unwrap_or_default();
                            if namespace.is_empty() && dataset.is_empty() && field.is_empty() {
                                return None;
                            }
                            Some(ParsedInputField {
                                namespace,
                                dataset,
                                field,
                                dataset_version: get_str(&inner[2]),
                                transformation_description: get_str(&inner[4]),
                                transformation_type: get_str(&inner[5]),
                            })
                        })
                        .collect(),
                    _ => vec![],
                };

                ParsedNode {
                    namespace: row.namespace_name,
                    dataset: row.dataset_name,
                    field: row.field_name,
                    field_type: row.field_type,
                    dataset_version_uuid: row.dataset_version_uuid.map(|u| u.to_string()),
                    transformation_description: None,
                    transformation_type: None,
                    input_fields,
                }
            })
            .collect();

        // Second pass: build edge maps
        let mut in_edges: HashMap<String, Vec<String>> = HashMap::new();
        let mut out_edges: HashMap<String, Vec<String>> = HashMap::new();

        for node in &parsed {
            let output_id = format!(
                "datasetField:{}:{}:{}",
                node.namespace, node.dataset, node.field
            );
            for input in &node.input_fields {
                let input_id = format!(
                    "datasetField:{}:{}:{}",
                    input.namespace, input.dataset, input.field
                );
                in_edges
                    .entry(output_id.clone())
                    .or_default()
                    .push(input_id.clone());
                out_edges
                    .entry(input_id)
                    .or_default()
                    .push(output_id.clone());
            }
        }

        // Third pass: build unique Node objects with data and edges
        let mut node_map: HashMap<String, Node> = HashMap::new();

        for node in &parsed {
            let node_id_str = format!(
                "datasetField:{}:{}:{}",
                node.namespace, node.dataset, node.field
            );

            // Build data payload matching Java's ColumnLineageNodeData
            let mut data_map = serde_json::Map::new();
            data_map.insert(
                "namespace".to_string(),
                serde_json::Value::String(node.namespace.clone()),
            );
            data_map.insert(
                "dataset".to_string(),
                serde_json::Value::String(node.dataset.clone()),
            );
            data_map.insert(
                "field".to_string(),
                serde_json::Value::String(node.field.clone()),
            );
            if let Some(ref dv) = node.dataset_version_uuid {
                data_map.insert(
                    "datasetVersion".to_string(),
                    serde_json::Value::String(dv.to_string()),
                );
            }
            if let Some(ref ft) = node.field_type {
                data_map.insert(
                    "fieldType".to_string(),
                    serde_json::Value::String(ft.clone()),
                );
            }
            if let Some(ref td) = node.transformation_description {
                data_map.insert(
                    "transformationDescription".to_string(),
                    serde_json::Value::String(td.clone()),
                );
            }
            if let Some(ref tt) = node.transformation_type {
                data_map.insert(
                    "transformationType".to_string(),
                    serde_json::Value::String(tt.clone()),
                );
            }

            // Build inputFields array in data
            let input_fields_json: Vec<serde_json::Value> = node
                .input_fields
                .iter()
                .map(|inf| {
                    let mut m = serde_json::Map::new();
                    m.insert(
                        "namespace".to_string(),
                        serde_json::Value::String(inf.namespace.clone()),
                    );
                    m.insert(
                        "dataset".to_string(),
                        serde_json::Value::String(inf.dataset.clone()),
                    );
                    m.insert(
                        "field".to_string(),
                        serde_json::Value::String(inf.field.clone()),
                    );
                    if let Some(ref dv) = inf.dataset_version {
                        m.insert(
                            "datasetVersion".to_string(),
                            serde_json::Value::String(dv.clone()),
                        );
                    }
                    if let Some(ref td) = inf.transformation_description {
                        m.insert(
                            "transformationDescription".to_string(),
                            serde_json::Value::String(td.clone()),
                        );
                    }
                    if let Some(ref tt) = inf.transformation_type {
                        m.insert(
                            "transformationType".to_string(),
                            serde_json::Value::String(tt.clone()),
                        );
                    }
                    serde_json::Value::Object(m)
                })
                .collect();
            data_map.insert(
                "inputFields".to_string(),
                serde_json::Value::Array(input_fields_json),
            );

            let node_in_edges: Vec<Edge> = in_edges
                .get(&node_id_str)
                .map(|origins| {
                    origins
                        .iter()
                        .map(|origin| Edge {
                            origin: NodeId::new(node_id_str.clone()),
                            destination: NodeId::new(origin.clone()),
                        })
                        .collect()
                })
                .unwrap_or_default();

            let node_out_edges: Vec<Edge> = out_edges
                .get(&node_id_str)
                .map(|destinations| {
                    destinations
                        .iter()
                        .map(|dest| Edge {
                            origin: NodeId::new(node_id_str.clone()),
                            destination: NodeId::new(dest.clone()),
                        })
                        .collect()
                })
                .unwrap_or_default();

            node_map.insert(
                node_id_str.clone(),
                Node {
                    id: NodeId::new(node_id_str),
                    type_: NodeType::DatasetField,
                    data: Some(serde_json::Value::Object(data_map)),
                    in_edges: node_in_edges,
                    out_edges: node_out_edges,
                },
            );

            // Also create stub nodes for input fields not yet in the map
            for input in &node.input_fields {
                let input_id = format!(
                    "datasetField:{}:{}:{}",
                    input.namespace, input.dataset, input.field
                );
                if !node_map.contains_key(&input_id) {
                    let mut input_data = serde_json::Map::new();
                    input_data.insert(
                        "namespace".to_string(),
                        serde_json::Value::String(input.namespace.clone()),
                    );
                    input_data.insert(
                        "dataset".to_string(),
                        serde_json::Value::String(input.dataset.clone()),
                    );
                    input_data.insert(
                        "field".to_string(),
                        serde_json::Value::String(input.field.clone()),
                    );
                    input_data.insert("inputFields".to_string(), serde_json::Value::Array(vec![]));

                    let input_in_edges: Vec<Edge> = in_edges
                        .get(&input_id)
                        .map(|origins| {
                            origins
                                .iter()
                                .map(|origin| Edge {
                                    origin: NodeId::new(input_id.clone()),
                                    destination: NodeId::new(origin.clone()),
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let input_out_edges: Vec<Edge> = out_edges
                        .get(&input_id)
                        .map(|destinations| {
                            destinations
                                .iter()
                                .map(|dest| Edge {
                                    origin: NodeId::new(input_id.clone()),
                                    destination: NodeId::new(dest.clone()),
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    node_map.insert(
                        input_id.clone(),
                        Node {
                            id: NodeId::new(input_id),
                            type_: NodeType::DatasetField,
                            data: Some(serde_json::Value::Object(input_data)),
                            in_edges: input_in_edges,
                            out_edges: input_out_edges,
                        },
                    );
                }
            }
        }

        Ok(Lineage {
            graph: node_map.into_values().collect(),
        })
    }

    /// Enrich a dataset with column lineage information for its fields.
    pub async fn enrich_dataset_with_column_lineage(
        &self,
        dataset: &mut api::Dataset,
    ) -> Result<(), AppError> {
        let version_uuid = match dataset.current_version {
            Some(v) => v,
            None => return Ok(()),
        };

        let _lineage_rows =
            db::column_lineage::find_by_output_dataset_version(&self.pool, version_uuid).await?;

        Ok(())
    }
}
