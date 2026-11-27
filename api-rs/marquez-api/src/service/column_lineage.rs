use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api::{self, Edge, Lineage, Node, NodeId};
use crate::models::common::NodeType;
use crate::models::db::FieldUuidTimestampRow;

/// Where a column-lineage traversal starts, parsed from a node id.
///
/// Mirrors the node-type dispatch in Java's `ColumnLineageService.getColumnNodes`.
/// A `#<uuid>` suffix on the last part selects one version.
#[derive(Debug, PartialEq, Eq)]
enum ColumnLineageStart<'a> {
    /// `dataset:<namespace>:<dataset>`
    Dataset { namespace: &'a str, name: &'a str },
    /// `dataset:<namespace>:<dataset>#<datasetVersion>`
    DatasetVersion { version: Uuid },
    /// `datasetField:<namespace>:<dataset>:<field>`
    DatasetField {
        namespace: &'a str,
        dataset: &'a str,
        field: &'a str,
    },
    /// `datasetField:<namespace>:<dataset>:<field>#<datasetVersion>`
    DatasetFieldVersion { field: &'a str, version: Uuid },
    /// `job:<namespace>:<job>`
    Job { namespace: &'a str, name: &'a str },
    /// `job:<namespace>:<job>#<jobVersion>`
    JobVersion { version: Uuid },
}

impl<'a> ColumnLineageStart<'a> {
    fn parse(node_id: &'a NodeId) -> Result<Self, AppError> {
        let value = node_id.value();
        let kind = value.split_once(':').map_or(value, |(kind, _)| kind);
        let versioned = node_id.has_version();
        match (kind, versioned) {
            ("datasetField", false) => {
                let p = node_id.parse_parts(4)?;
                Ok(Self::DatasetField {
                    namespace: p[1],
                    dataset: p[2],
                    field: p[3],
                })
            }
            ("datasetField", true) => {
                let p = node_id.parse_parts(4)?;
                let (field, version) = split_version(node_id, p[3])?;
                Ok(Self::DatasetFieldVersion { field, version })
            }
            ("dataset", false) => {
                let p = node_id.parse_parts(3)?;
                Ok(Self::Dataset {
                    namespace: p[1],
                    name: p[2],
                })
            }
            ("dataset", true) => {
                let p = node_id.parse_parts(3)?;
                let (_, version) = split_version(node_id, p[2])?;
                Ok(Self::DatasetVersion { version })
            }
            ("job", false) => {
                let p = node_id.parse_parts(3)?;
                Ok(Self::Job {
                    namespace: p[1],
                    name: p[2],
                })
            }
            ("job", true) => {
                let p = node_id.parse_parts(3)?;
                let (_, version) = split_version(node_id, p[2])?;
                Ok(Self::JobVersion { version })
            }
            // Java answers 500 (UnsupportedOperationException) for `run:` ids.
            _ => Err(AppError::BadRequest(format!(
                "Unsupported node ID for column lineage: {value}"
            ))),
        }
    }
}

/// Split `<name>#<version>` into the name and the version UUID.
fn split_version<'a>(node_id: &NodeId, part: &'a str) -> Result<(&'a str, Uuid), AppError> {
    part.split_once('#')
        .and_then(|(name, version)| Some((name, Uuid::parse_str(version).ok()?)))
        .ok_or_else(|| AppError::BadRequest(format!("Invalid node ID: {node_id}")))
}

pub struct ColumnLineageService {
    pool: PgPool,
}

impl ColumnLineageService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Resolve the dataset field UUIDs a traversal starts from.
    ///
    /// For versioned node ids also returns the version's timestamp, which Java
    /// uses as `createdAtUntil` so the graph shows lineage as of that version.
    /// A job version can have several runs; Java takes an arbitrary one
    /// (`findAny()`), this takes the latest so the result is deterministic.
    async fn find_start_fields(
        &self,
        start: &ColumnLineageStart<'_>,
    ) -> Result<(Vec<Uuid>, Option<DateTime<Utc>>), AppError> {
        fn with_timestamp(rows: Vec<FieldUuidTimestampRow>) -> (Vec<Uuid>, Option<DateTime<Utc>>) {
            let created_at = rows.iter().map(|r| r.created_at).max();
            (rows.into_iter().map(|r| r.uuid).collect(), created_at)
        }

        let pool = &self.pool;
        Ok(match *start {
            ColumnLineageStart::Dataset { namespace, name } => (
                // All fields across ALL versions via dataset symlinks (matches Java)
                db::dataset_field::find_dataset_fields_uuids(pool, namespace, name).await?,
                None,
            ),
            ColumnLineageStart::DatasetField {
                namespace,
                dataset,
                field,
            } => (
                db::dataset_field::find_uuids_by_dataset_name(pool, namespace, dataset, field)
                    .await?,
                None,
            ),
            ColumnLineageStart::Job { namespace, name } => (
                db::dataset_field::find_fields_uuids_by_job(pool, namespace, name).await?,
                None,
            ),
            ColumnLineageStart::DatasetVersion { version } => with_timestamp(
                db::dataset_field::find_dataset_version_fields_uuids(pool, version, None).await?,
            ),
            ColumnLineageStart::DatasetFieldVersion { field, version } => with_timestamp(
                db::dataset_field::find_dataset_version_fields_uuids(pool, version, Some(field))
                    .await?,
            ),
            ColumnLineageStart::JobVersion { version } => with_timestamp(
                db::dataset_field::find_fields_uuids_by_job_version(pool, version).await?,
            ),
        })
    }

    /// Get column lineage for a given node.
    ///
    /// Accepts the same node ids as Java's `ColumnLineageService.lineage`:
    /// `dataset:`, `datasetField:` and `job:` ids, each optionally suffixed
    /// with `#<version>` (see [`ColumnLineageStart`]). Resolves the starting
    /// dataset fields for that node and traverses the column lineage graph
    /// using the aggregated CTE.
    ///
    /// Returns a `Lineage` with `Node` objects of type `DATASET_FIELD`,
    /// each with `inEdges` / `outEdges` and field data, matching the
    /// Java API response shape. For a versioned node id the returned node
    /// ids carry the dataset version too (`datasetField:ns:ds:field#<version>`).
    ///
    /// `created_at_until` bounds which column lineage rows are considered;
    /// for a versioned node id the version's own timestamp is used instead.
    pub async fn get_lineage(
        &self,
        node_id: &NodeId,
        depth: i32,
        with_downstream: bool,
        created_at_until: DateTime<Utc>,
    ) -> Result<Lineage, AppError> {
        let start = ColumnLineageStart::parse(node_id)?;
        let (field_uuids, version_created_at) = self.find_start_fields(&start).await?;
        if field_uuids.is_empty() {
            return Err(AppError::NotFound("Could not find node".to_string()));
        }
        let created_at_until = version_created_at.unwrap_or(created_at_until);

        // Java's toLineage(..., nodeId.hasVersion()): versioned requests get
        // versioned node ids (no suffix when the row has no dataset version).
        let include_version = node_id.has_version();
        let field_node_id =
            |namespace: &str, dataset: &str, field: &str, version: Option<&str>| match version
                .filter(|_| include_version)
            {
                Some(v) => format!("datasetField:{namespace}:{dataset}:{field}#{v}"),
                None => format!("datasetField:{namespace}:{dataset}:{field}"),
            };

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
                    input_fields,
                }
            })
            .collect();

        // Second pass: build edge maps
        let mut in_edges: HashMap<String, Vec<String>> = HashMap::new();
        let mut out_edges: HashMap<String, Vec<String>> = HashMap::new();

        for node in &parsed {
            let output_id = field_node_id(
                &node.namespace,
                &node.dataset,
                &node.field,
                node.dataset_version_uuid.as_deref(),
            );
            for input in &node.input_fields {
                let input_id = field_node_id(
                    &input.namespace,
                    &input.dataset,
                    &input.field,
                    input.dataset_version.as_deref(),
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
            let node_id_str = field_node_id(
                &node.namespace,
                &node.dataset,
                &node.field,
                node.dataset_version_uuid.as_deref(),
            );

            // Data payload as Java serializes ColumnLineageNodeData: every key,
            // with explicit nulls (`None` -> null here). The node-level
            // transformation keys come from Java's (deprecated) getters, which
            // return the value of any one of the node's input fields.
            let first_input = node.input_fields.first();
            let input_fields_json: Vec<serde_json::Value> = node
                .input_fields
                .iter()
                .map(|inf| {
                    json!({
                        "namespace": inf.namespace,
                        "dataset": inf.dataset,
                        "datasetVersion": inf.dataset_version,
                        "field": inf.field,
                        "transformationDescription": inf.transformation_description,
                        "transformationType": inf.transformation_type,
                    })
                })
                .collect();
            let data = json!({
                "namespace": node.namespace,
                "dataset": node.dataset,
                "datasetVersion": node.dataset_version_uuid,
                "field": node.field,
                "fieldType": node.field_type,
                "transformationDescription": first_input.and_then(|i| i.transformation_description.as_deref()),
                "transformationType": first_input.and_then(|i| i.transformation_type.as_deref()),
                "inputFields": input_fields_json,
            });

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
                    data: Some(data),
                    in_edges: node_in_edges,
                    out_edges: node_out_edges,
                },
            );

            // Also create stub nodes for input fields not yet in the map
            for input in &node.input_fields {
                let input_id = field_node_id(
                    &input.namespace,
                    &input.dataset,
                    &input.field,
                    input.dataset_version.as_deref(),
                );
                if !node_map.contains_key(&input_id) {
                    // Java's ColumnLineageNodeData(InputFieldNodeData): the input's
                    // version, an empty fieldType and no inputs. With no inputs the
                    // transformation getters serialize as null.
                    let input_data = json!({
                        "namespace": input.namespace,
                        "dataset": input.dataset,
                        "datasetVersion": input.dataset_version,
                        "field": input.field,
                        "fieldType": "",
                        "transformationDescription": null,
                        "transformationType": null,
                        "inputFields": [],
                    });

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
                            data: Some(input_data),
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

#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = "1f90d9a9-d956-430b-b8c5-6a61b437293e";

    fn version() -> Uuid {
        Uuid::parse_str(V).unwrap()
    }

    fn assert_parses(id: &str, expected: ColumnLineageStart<'_>) {
        let node_id = NodeId::new(id);
        assert_eq!(
            ColumnLineageStart::parse(&node_id).unwrap(),
            expected,
            "{id}"
        );
    }

    #[test]
    fn parses_dataset() {
        assert_parses(
            "dataset:repro:b",
            ColumnLineageStart::Dataset {
                namespace: "repro",
                name: "b",
            },
        );
    }

    #[test]
    fn parses_dataset_field() {
        // ilum-cloud/marquez#25: this used to be read as dataset `b:y`.
        assert_parses(
            "datasetField:repro:b:y",
            ColumnLineageStart::DatasetField {
                namespace: "repro",
                dataset: "b",
                field: "y",
            },
        );
    }

    #[test]
    fn parses_dataset_field_with_uri_namespace() {
        assert_parses(
            "datasetField:hive://host:9083:db.t:col",
            ColumnLineageStart::DatasetField {
                namespace: "hive://host:9083",
                dataset: "db.t",
                field: "col",
            },
        );
        assert_parses(
            "datasetField:s3://bucket:out/p:q",
            ColumnLineageStart::DatasetField {
                namespace: "s3://bucket",
                dataset: "out/p",
                field: "q",
            },
        );
    }

    #[test]
    fn parses_job() {
        assert_parses(
            "job:repro:write_b",
            ColumnLineageStart::Job {
                namespace: "repro",
                name: "write_b",
            },
        );
    }

    #[test]
    fn parses_versioned_ids() {
        assert_parses(
            &format!("dataset:repro:b#{V}"),
            ColumnLineageStart::DatasetVersion { version: version() },
        );
        assert_parses(
            &format!("datasetField:repro:b:y#{V}"),
            ColumnLineageStart::DatasetFieldVersion {
                field: "y",
                version: version(),
            },
        );
        assert_parses(
            &format!("job:repro:write_b#{V}"),
            ColumnLineageStart::JobVersion { version: version() },
        );
        assert_parses(
            &format!("datasetField:hive://host:9083:db.t:col#{V}"),
            ColumnLineageStart::DatasetFieldVersion {
                field: "col",
                version: version(),
            },
        );
    }

    #[test]
    fn rejects_unsupported_and_malformed_ids() {
        for id in [
            "run:0b8c1f9e-5a0e-4c3a-9d55-1f2a3b4c5d02",
            "invalid",
            "dataset:repro",
            "datasetField:repro:b",
            "dataset:repro:b#not-a-uuid",
            "datasetField:repro:b:y#",
            "job:repro:write_b#123",
        ] {
            let node_id = NodeId::new(id);
            assert!(
                matches!(
                    ColumnLineageStart::parse(&node_id),
                    Err(AppError::BadRequest(_))
                ),
                "expected BadRequest for {id}"
            );
        }
    }
}
