use std::collections::{HashMap, HashSet};

use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api::{
    self, DatasetSummary, Edge, JobSummary, Lineage, Node, NodeId, RunSummary, UpstreamRun,
    UpstreamRunLineage,
};
use crate::models::common::NodeType;
use crate::models::db::{DatasetDataRow, JobDataRow, RunWithFacetsRow};
use crate::models::iso8601;

pub struct LineageService {
    pool: PgPool,
}

impl LineageService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get lineage graph for a given node.
    ///
    /// The `node_id` string has format: "job:namespace:name" or "dataset:namespace:name"
    pub async fn get_lineage(
        &self,
        node_id: &api::NodeId,
        depth: i32,
    ) -> Result<api::Lineage, AppError> {
        let parts = node_id.parse_parts(3)?;
        let node_type = parts[0];
        let ns_name = parts[1];
        let name = parts[2];

        match node_type {
            "job" => {
                let job = db::job::find_by_name(&self.pool, ns_name, name)
                    .await?
                    .ok_or_else(|| {
                        AppError::NotFound(format!("Job '{}/{}' not found", ns_name, name))
                    })?;
                let (lineage, _ds_uuids) = self.build_lineage_graph(&[job.uuid], depth).await?;
                Ok(lineage)
            }
            "dataset" => {
                let job_uuid =
                    db::lineage::get_job_from_input_or_output(&self.pool, name, ns_name).await?;
                if let Some(job_uuid) = job_uuid {
                    let (lineage, ds_uuids) = self.build_lineage_graph(&[job_uuid], depth).await?;

                    // Java-style validation (LineageService lines 111-122):
                    // Resolve the queried dataset to its primary UUID and verify
                    // it appears in the lineage graph. If not, the job found via
                    // get_job_from_input_or_output references a different dataset
                    // (e.g. via symlink) — return an orphan graph instead of
                    // wrong data.
                    let ds_rows =
                        db::lineage::get_dataset_data_by_name(&self.pool, ns_name, name).await?;
                    if let Some(ds) = ds_rows.first() {
                        if !ds_uuids.contains(&ds.uuid) {
                            return Ok(Lineage {
                                graph: vec![Node {
                                    id: NodeId::new(format!("dataset:{}:{}", ns_name, name)),
                                    type_: NodeType::Dataset,
                                    data: Some(build_dataset_node_data(ds)),
                                    in_edges: vec![],
                                    out_edges: vec![],
                                }],
                            });
                        }
                    }

                    Ok(lineage)
                } else {
                    // Fallback: dataset exists but no job references it
                    let ds_rows =
                        db::lineage::get_dataset_data_by_name(&self.pool, ns_name, name).await?;
                    match ds_rows.into_iter().next() {
                        Some(ds) => Ok(Lineage {
                            graph: vec![Node {
                                id: NodeId::new(format!("dataset:{}:{}", ns_name, name)),
                                type_: NodeType::Dataset,
                                data: Some(build_dataset_node_data(&ds)),
                                in_edges: vec![],
                                out_edges: vec![],
                            }],
                        }),
                        None => Err(AppError::NotFound(format!(
                            "Dataset '{}/{}' not found",
                            ns_name, name
                        ))),
                    }
                }
            }
            _ => Err(AppError::BadRequest(format!(
                "Unknown node type: {}",
                node_type
            ))),
        }
    }

    /// Get upstream runs for a given run.
    ///
    /// Groups flat DB rows by run UUID into nested `UpstreamRunLineage`
    /// with `{job, run, inputs}` objects matching Java's response shape.
    pub async fn get_upstream_runs(
        &self,
        run_id: Uuid,
        depth: i32,
    ) -> Result<UpstreamRunLineage, AppError> {
        let rows = db::lineage::get_upstream_runs(&self.pool, run_id, depth).await?;

        // Group by run UUID preserving insertion order (SQL returns ORDER BY depth, job_name)
        let mut order: Vec<Uuid> = Vec::new();
        let mut grouped: HashMap<Uuid, Vec<usize>> = HashMap::new();
        for (i, row) in rows.iter().enumerate() {
            if !grouped.contains_key(&row.r_uuid) {
                order.push(row.r_uuid);
            }
            grouped.entry(row.r_uuid).or_default().push(i);
        }

        let runs = order
            .into_iter()
            .filter_map(|uuid| grouped.remove(&uuid))
            .map(|indices| {
                let first = &rows[indices[0]];
                let inputs: Vec<DatasetSummary> = indices
                    .iter()
                    .map(|&i| &rows[i])
                    .filter(|r| r.dataset_uuid.is_some())
                    .map(|r| DatasetSummary {
                        namespace: r.dataset_namespace.clone().unwrap_or_default(),
                        name: r.dataset_name.clone().unwrap_or_default(),
                        version: r.dataset_version_uuid,
                        produced_by_run_id: r.u_r_uuid,
                    })
                    .collect();
                UpstreamRun {
                    job: JobSummary {
                        namespace: first.job_namespace.clone().unwrap_or_default(),
                        name: first.job_name.clone().unwrap_or_default(),
                        version: first.job_version_uuid,
                    },
                    run: RunSummary {
                        id: first.r_uuid,
                        start: first.started_at.map(|t| iso8601::fmt(&t)),
                        end: first.ended_at.map(|t| iso8601::fmt(&t)),
                        status: first.state.clone(),
                    },
                    inputs,
                }
            })
            .collect();

        Ok(UpstreamRunLineage { runs })
    }

    // ---- Private helpers ----

    /// Build a lineage graph starting from the given job UUIDs.
    ///
    /// Uses the enriched DAO functions to get full job and dataset data,
    /// populating each node's `data` field so the frontend can render them.
    ///
    /// Returns the lineage graph together with the set of dataset UUIDs
    /// present in the graph (used by callers for post-build validation).
    async fn build_lineage_graph(
        &self,
        seed_job_ids: &[Uuid],
        depth: i32,
    ) -> Result<(Lineage, HashSet<Uuid>), AppError> {
        // Get enriched job data with input/output UUID arrays
        let job_data_rows = db::lineage::get_lineage(&self.pool, depth, seed_job_ids).await?;

        // Java check: if getLineage returns no job data, return empty graph
        if job_data_rows.is_empty() {
            return Ok((Lineage { graph: vec![] }, HashSet::new()));
        }

        // Collect unique dataset UUIDs from inputs and outputs
        let mut all_ds_uuids: Vec<Uuid> = Vec::new();
        for jdr in &job_data_rows {
            if let Some(inputs) = &jdr.input_uuids {
                all_ds_uuids.extend(inputs);
            }
            if let Some(outputs) = &jdr.output_uuids {
                all_ds_uuids.extend(outputs);
            }
        }
        all_ds_uuids.sort();
        all_ds_uuids.dedup();

        // Get dataset data for all referenced datasets
        let dataset_data_rows = if all_ds_uuids.is_empty() {
            vec![]
        } else {
            db::lineage::get_dataset_data(&self.pool, &all_ds_uuids).await?
        };

        // Get current runs with facets for all jobs
        let job_uuids: Vec<Uuid> = job_data_rows.iter().map(|j| j.uuid).collect();
        let run_rows = if job_uuids.is_empty() {
            vec![]
        } else {
            db::lineage::get_current_runs_with_facets(&self.pool, &job_uuids)
                .await
                .unwrap_or_default()
        };

        // Build lookup maps
        let mut run_map: HashMap<(String, String), &RunWithFacetsRow> = HashMap::new();
        for run in &run_rows {
            if let (Some(job_name), Some(ns)) = (&run.job_name, &run.namespace_name) {
                run_map.insert((job_name.clone(), ns.clone()), run);
            }
        }

        let mut ds_map: HashMap<Uuid, &DatasetDataRow> = HashMap::new();
        for ds in &dataset_data_rows {
            ds_map.insert(ds.uuid, ds);
        }

        let mut nodes: Vec<Node> = Vec::new();
        let mut dataset_uuids_seen: HashSet<Uuid> = HashSet::new();

        // Two-pass approach (matches Java LineageService):
        // Pass 1: Build job nodes, collect maps of dataset→jobs for edge computation.
        let mut ds_input_to_jobs: HashMap<Uuid, Vec<NodeId>> = HashMap::new();
        let mut ds_output_to_jobs: HashMap<Uuid, Vec<NodeId>> = HashMap::new();

        for jdr in &job_data_rows {
            let ns = jdr.namespace_name.as_deref().unwrap_or_default();
            let job_node_id = NodeId::new(format!("job:{}:{}", ns, jdr.name));

            let mut in_edges = Vec::new();
            let mut out_edges = Vec::new();

            // Process inputs: dataset → job (edge into the job)
            if let Some(input_uuids) = &jdr.input_uuids {
                for ds_uuid in input_uuids {
                    if let Some(ds) = ds_map.get(ds_uuid) {
                        let ds_ns = ds.namespace_name.as_deref().unwrap_or_default();
                        let ds_node_id = NodeId::new(format!("dataset:{}:{}", ds_ns, ds.name));
                        in_edges.push(Edge {
                            origin: ds_node_id.clone(),
                            destination: job_node_id.clone(),
                        });
                        dataset_uuids_seen.insert(*ds_uuid);
                        ds_input_to_jobs
                            .entry(*ds_uuid)
                            .or_default()
                            .push(job_node_id.clone());
                    }
                }
            }

            // Process outputs: job → dataset (edge out of the job)
            if let Some(output_uuids) = &jdr.output_uuids {
                for ds_uuid in output_uuids {
                    if let Some(ds) = ds_map.get(ds_uuid) {
                        let ds_ns = ds.namespace_name.as_deref().unwrap_or_default();
                        let ds_node_id = NodeId::new(format!("dataset:{}:{}", ds_ns, ds.name));
                        out_edges.push(Edge {
                            origin: job_node_id.clone(),
                            destination: ds_node_id.clone(),
                        });
                        dataset_uuids_seen.insert(*ds_uuid);
                        ds_output_to_jobs
                            .entry(*ds_uuid)
                            .or_default()
                            .push(job_node_id.clone());
                    }
                }
            }

            // Build job node with data
            let latest_run = run_map.get(&(jdr.name.clone(), ns.to_string()));
            let job_data = build_job_node_data(jdr, latest_run.copied(), &ds_map);

            nodes.push(Node {
                id: job_node_id,
                type_: NodeType::Job,
                data: Some(job_data),
                in_edges,
                out_edges,
            });
        }

        // Pass 2: Build dataset nodes with bidirectional edges.
        for ds_uuid in &dataset_uuids_seen {
            if let Some(ds) = ds_map.get(ds_uuid) {
                let ds_ns = ds.namespace_name.as_deref().unwrap_or_default();
                let ds_node_id = NodeId::new(format!("dataset:{}:{}", ds_ns, ds.name));

                // Jobs that produce this dataset → edges INTO this dataset
                let in_edges: Vec<Edge> = ds_output_to_jobs
                    .get(ds_uuid)
                    .map(|job_ids| {
                        job_ids
                            .iter()
                            .map(|j| Edge {
                                origin: j.clone(),
                                destination: ds_node_id.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Jobs that consume this dataset → edges OUT OF this dataset
                let out_edges: Vec<Edge> = ds_input_to_jobs
                    .get(ds_uuid)
                    .map(|job_ids| {
                        job_ids
                            .iter()
                            .map(|j| Edge {
                                origin: ds_node_id.clone(),
                                destination: j.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                nodes.push(Node {
                    id: ds_node_id,
                    type_: NodeType::Dataset,
                    data: Some(build_dataset_node_data(ds)),
                    in_edges,
                    out_edges,
                });
            }
        }

        // Sort nodes by NodeId (lexicographic) to match Java's ImmutableSortedSet
        for node in &mut nodes {
            node.in_edges.sort();
            node.out_edges.sort();
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        Ok((Lineage { graph: nodes }, dataset_uuids_seen))
    }
}

/// Wrap flat `[{namespace, name, version}]` dataset version arrays into
/// `[{datasetVersionId: {...}, facets: {}}]` to match Java's serialization.
fn wrap_dataset_versions(raw: &serde_json::Value) -> serde_json::Value {
    match raw.as_array() {
        Some(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| {
                    serde_json::json!({
                        "datasetVersionId": v,
                        "facets": {}
                    })
                })
                .collect(),
        ),
        None => serde_json::json!([]),
    }
}

/// Build the `data` JSON for a job node in the lineage graph.
///
/// Matches the frontend's `LineageJob` interface.
fn build_job_node_data(
    jdr: &JobDataRow,
    run: Option<&RunWithFacetsRow>,
    ds_map: &HashMap<Uuid, &DatasetDataRow>,
) -> serde_json::Value {
    let ns = jdr.namespace_name.as_deref().unwrap_or_default();

    // Build inputs/outputs as dataset ID objects
    let inputs: Vec<serde_json::Value> = jdr
        .input_uuids
        .as_ref()
        .map(|uuids| {
            uuids
                .iter()
                .filter_map(|u| ds_map.get(u))
                .map(|ds| {
                    serde_json::json!({
                        "namespace": ds.namespace_name.as_deref().unwrap_or_default(),
                        "name": ds.name
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let outputs: Vec<serde_json::Value> = jdr
        .output_uuids
        .as_ref()
        .map(|uuids| {
            uuids
                .iter()
                .filter_map(|u| ds_map.get(u))
                .map(|ds| {
                    serde_json::json!({
                        "namespace": ds.namespace_name.as_deref().unwrap_or_default(),
                        "name": ds.name
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Build latest run if available
    let latest_run = run.map(|r| {
        let duration_ms = match (r.started_at, r.ended_at) {
            (Some(start), Some(end)) => Some((end - start).num_milliseconds()),
            _ => None,
        };
        let args = r
            .args
            .as_deref()
            .and_then(|a| serde_json::from_str::<serde_json::Value>(a).ok())
            .unwrap_or(serde_json::json!({}));
        let job_version = match (&r.namespace_name, &r.job_name, &r.job_version) {
            (Some(ns), Some(jn), Some(ver)) => serde_json::json!({
                "namespace": ns, "name": jn, "version": ver
            }),
            _ => serde_json::Value::Null,
        };
        let input_dv =
            wrap_dataset_versions(r.input_versions.as_ref().unwrap_or(&serde_json::json!([])));
        let output_dv =
            wrap_dataset_versions(r.output_versions.as_ref().unwrap_or(&serde_json::json!([])));
        serde_json::json!({
            "id": r.uuid,
            "createdAt": iso8601::fmt(&r.created_at),
            "updatedAt": iso8601::fmt(&r.updated_at),
            "nominalStartTime": iso8601::fmt_opt(&r.nominal_start_time),
            "nominalEndTime": iso8601::fmt_opt(&r.nominal_end_time),
            "state": r.current_run_state,
            "startedAt": iso8601::fmt_opt(&r.started_at),
            "endedAt": iso8601::fmt_opt(&r.ended_at),
            "durationMs": duration_ms,
            "args": args,
            "jobVersion": job_version,
            "inputDatasetVersions": input_dv,
            "outputDatasetVersions": output_dv,
            "facets": r.facets.as_ref().unwrap_or(&serde_json::json!({})),
        })
    });

    // Only include latestRun when current_run_uuid is set (matches Java behaviour)
    let effective_latest_run = if jdr.current_run_uuid.is_some() {
        latest_run
    } else {
        None
    };

    serde_json::json!({
        "id": {"namespace": ns, "name": jdr.name},
        "type": jdr.type_,
        "name": jdr.name,
        "simpleName": jdr.simple_name,
        "createdAt": iso8601::fmt(&jdr.created_at),
        "updatedAt": iso8601::fmt(&jdr.updated_at),
        "namespace": ns,
        "inputs": inputs,
        "outputs": outputs,
        "location": jdr.current_location,
        "description": jdr.description,
        "latestRun": effective_latest_run,
        "currentRunUuid": jdr.current_run_uuid,
        "parentJobName": jdr.parent_job_name,
        "parentJobUuid": jdr.parent_job_uuid,
    })
}

/// Build the `data` JSON for a dataset node in the lineage graph.
///
/// Matches the frontend's `LineageDataset` interface.
fn build_dataset_node_data(ds: &DatasetDataRow) -> serde_json::Value {
    let ns = ds.namespace_name.as_deref().unwrap_or_default();
    serde_json::json!({
        "id": {"namespace": ns, "name": ds.name},
        "type": ds.type_,
        "name": ds.name,
        "physicalName": ds.physical_name,
        "createdAt": iso8601::fmt(&ds.created_at),
        "updatedAt": iso8601::fmt(&ds.updated_at),
        "namespace": ns,
        "sourceName": ds.source_name.as_deref().unwrap_or_default(),
        "fields": ds.fields.as_ref().unwrap_or(&serde_json::json!([])),
        "tags": [],
        "lastModifiedAt": iso8601::fmt_opt(&ds.last_modified_at),
        "description": ds.description,
        "lastLifecycleState": ds.lifecycle_state.as_deref().unwrap_or(""),
    })
}
