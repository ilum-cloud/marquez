use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api;
use crate::models::common::{RunId, RunState};
use crate::models::db::ExtendedRunWithFacetsRow;

pub struct RunService {
    pool: PgPool,
}

impl RunService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new run for a job.
    pub async fn create_run(
        &self,
        ns_name: &str,
        job_name: &str,
        args: Option<&serde_json::Value>,
        nominal_start: Option<DateTime<Utc>>,
        nominal_end: Option<DateTime<Utc>>,
    ) -> Result<api::Run, AppError> {
        let job = db::job::find_by_name(&self.pool, ns_name, job_name)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Job '{}/{}' not found", ns_name, job_name))
            })?;

        let run_uuid = Uuid::new_v4();
        let now = Utc::now();

        // Optionally store run args
        let run_args_uuid = if let Some(args_val) = args {
            let args_str = serde_json::to_string(args_val).unwrap_or_default();
            let checksum = kv_checksum_from_json(args_val);
            let args_row =
                db::run_args::upsert(&self.pool, Uuid::new_v4(), now, &args_str, &checksum).await?;
            Some(args_row.uuid)
        } else {
            None
        };

        let row = db::run::upsert(
            &self.pool,
            run_uuid,
            now,
            Some(job.uuid),
            None,
            None,
            run_args_uuid,
            nominal_start,
            nominal_end,
            Some("NEW"),
            None,
            None,
            None,
            None,
            ns_name,
            job_name,
            None,
            None,
        )
        .await?;

        // Insert initial run state
        db::run_state::upsert(&self.pool, Uuid::new_v4(), now, run_uuid, "NEW").await?;

        // Update job's current_run_uuid so run state filtering works
        db::job::update_current_run(&self.pool, job.uuid, run_uuid, now).await?;

        self.get(row.uuid).await
    }

    pub async fn get(&self, run_uuid: Uuid) -> Result<api::Run, AppError> {
        let row = db::run::find_run_by_uuid_with_facets(&self.pool, run_uuid)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Run '{}' not found", run_uuid)))?;
        Ok(map_extended_run_to_api(&row))
    }

    pub async fn list(&self, limit: i32, offset: i32) -> Result<Vec<api::Run>, AppError> {
        let base_rows = db::run::find_all(&self.pool, limit, offset).await?;
        if base_rows.is_empty() {
            return Ok(vec![]);
        }
        let uuids: Vec<Uuid> = base_rows.iter().map(|r| r.uuid).collect();
        let enriched = db::run::find_runs_by_uuids_with_facets(&self.pool, &uuids).await?;
        // Preserve the original order from find_all (started_at DESC)
        let order: std::collections::HashMap<Uuid, usize> =
            uuids.iter().enumerate().map(|(i, u)| (*u, i)).collect();
        let mut runs: Vec<(usize, api::Run)> = enriched
            .iter()
            .map(|r| {
                let idx = order.get(&r.uuid).copied().unwrap_or(usize::MAX);
                (idx, map_extended_run_to_api(r))
            })
            .collect();
        runs.sort_by_key(|(idx, _)| *idx);
        Ok(runs.into_iter().map(|(_, r)| r).collect())
    }

    pub async fn list_by_job(
        &self,
        ns_name: &str,
        job_name: &str,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<api::Run>, AppError> {
        let rows =
            db::run::find_by_latest_job(&self.pool, ns_name, job_name, limit, offset).await?;
        Ok(rows.iter().map(map_extended_run_to_api).collect())
    }

    pub async fn count_by_job(&self, ns_name: &str, job_name: &str) -> Result<i64, AppError> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM runs WHERE namespace_name = $1 AND job_name = $2")
                .bind(ns_name)
                .bind(job_name)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    /// Transition a run to a new state.
    ///
    /// Wrapped in a transaction to match Java's `@Transaction` annotation.
    pub async fn mark_run_as(
        &self,
        run_uuid: Uuid,
        state: &str,
        at: DateTime<Utc>,
    ) -> Result<api::Run, AppError> {
        let mut tx = self.pool.begin().await?;

        // Update run state
        db::run::update_run_state(&mut *tx, run_uuid, at, state).await?;

        // Insert run state record
        let state_row =
            db::run_state::upsert(&mut *tx, Uuid::new_v4(), at, run_uuid, state).await?;

        // Handle start/end state transitions
        match state {
            "RUNNING" => {
                db::run::update_start_state(&mut *tx, run_uuid, at, state_row.uuid).await?;
            }
            "COMPLETED" | "FAILED" | "ABORTED" => {
                db::run::update_end_state(&mut *tx, run_uuid, at, state_row.uuid).await?;

                // Update last_modified_at on output datasets (matches Java RunService)
                let output_ds_uuids =
                    db::dataset::find_output_dataset_uuids_by_run(&mut *tx, run_uuid).await?;
                if !output_ds_uuids.is_empty() {
                    db::dataset::update_last_modified_at(&mut *tx, &output_ds_uuids, at).await?;
                }

                // Create job version on run completion (matches Java RunService.markRunAs)
                if let Some(job_row) = db::run::find_job_row_by_run_uuid(&mut *tx, run_uuid).await?
                {
                    let ns_name = job_row.namespace_name.as_deref().unwrap_or("");
                    let job_name = &job_row.name;
                    let run_row = db::run::find_by_uuid(&mut *tx, run_uuid).await?;
                    let location = run_row.as_ref().and_then(|r| r.location.as_deref());

                    let input_uuids =
                        db::job_version::find_current_input_dataset_uuids(&mut *tx, job_row.uuid)
                            .await?;
                    let output_uuids =
                        db::job_version::find_current_output_dataset_uuids(&mut *tx, job_row.uuid)
                            .await?;

                    // compute_job_version_uuid needs &PgPool for multiple reads;
                    // use pool directly — reading committed data is fine here.
                    let version_uuid = db::openlineage::compute_job_version_uuid(
                        &self.pool,
                        ns_name,
                        job_name,
                        &input_uuids,
                        &output_uuids,
                        location,
                    )
                    .await?;

                    let jv_row = db::job_version::upsert(
                        &mut *tx,
                        Uuid::new_v4(),
                        at,
                        job_row.uuid,
                        location,
                        version_uuid,
                        None,
                        job_row.namespace_uuid,
                        ns_name,
                        job_name,
                    )
                    .await?;

                    db::job_version::upsert_input_datasets_batch(
                        &self.pool,
                        jv_row.uuid,
                        &input_uuids,
                        job_row.uuid,
                        None,
                    )
                    .await?;
                    db::job_version::upsert_output_datasets_batch(
                        &self.pool,
                        jv_row.uuid,
                        &output_uuids,
                        job_row.uuid,
                        None,
                    )
                    .await?;

                    db::run::update_job_version(&mut *tx, run_uuid, jv_row.uuid).await?;
                    db::job_version::update_latest_run(&mut *tx, jv_row.uuid, run_uuid, at).await?;
                    db::job::update_version(&mut *tx, job_row.uuid, at, jv_row.uuid).await?;
                    // Link job facets to this job version (matches Java)
                    db::job_version::link_job_facets_to_job_version(
                        &mut *tx,
                        run_uuid,
                        jv_row.uuid,
                    )
                    .await?;
                }
            }
            _ => {}
        }

        tx.commit().await?;
        self.get(run_uuid).await
    }

    pub async fn start(&self, run_uuid: Uuid, at: DateTime<Utc>) -> Result<api::Run, AppError> {
        self.mark_run_as(run_uuid, "RUNNING", at).await
    }

    pub async fn complete(&self, run_uuid: Uuid, at: DateTime<Utc>) -> Result<api::Run, AppError> {
        self.mark_run_as(run_uuid, "COMPLETED", at).await
    }

    pub async fn fail(&self, run_uuid: Uuid, at: DateTime<Utc>) -> Result<api::Run, AppError> {
        self.mark_run_as(run_uuid, "FAILED", at).await
    }

    pub async fn abort(&self, run_uuid: Uuid, at: DateTime<Utc>) -> Result<api::Run, AppError> {
        self.mark_run_as(run_uuid, "ABORTED", at).await
    }
}

/// Enrich input/output dataset version JSON arrays with facets from DB.
///
/// Looks up dataset facets for the given run (type = 'input' or 'output')
/// and patches the `facets` field in each entry that matches by
/// `datasetVersionId.version`.
pub async fn enrich_io_dataset_versions(
    pool: &PgPool,
    run_uuid: Uuid,
    input_versions: &mut serde_json::Value,
    output_versions: &mut serde_json::Value,
) -> Result<(), AppError> {
    let facet_map = db::facets::find_dataset_facets_by_run(pool, run_uuid).await?;
    if facet_map.is_empty() {
        return Ok(());
    }
    for arr in [input_versions, output_versions] {
        if let Some(items) = arr.as_array_mut() {
            for item in items {
                if let Some(version_str) = item["datasetVersionId"]["version"].as_str() {
                    if let Ok(version_uuid) = Uuid::parse_str(version_str) {
                        if let Some(facets) = facet_map.get(&version_uuid) {
                            item["facets"] = facets.clone();
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Convert an `ExtendedRunWithFacetsRow` (from a single enriched SQL query) into
/// an `api::Run` without any additional database queries.
///
/// Handles:
/// - args JSON parsing
/// - facets (already merged by SQL via `jsonb_object_agg`)
/// - job_version link (uses the run's own namespace/name)
/// - input/output dataset versions → API format with `datasetVersionId` wrapper
/// - dataset facets patching into input/output version entries
pub fn map_extended_run_to_api(row: &ExtendedRunWithFacetsRow) -> api::Run {
    let duration_ms = match (row.started_at, row.ended_at) {
        (Some(start), Some(end)) => Some((end - start).num_milliseconds()),
        _ => None,
    };

    let args = row
        .args
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::json!({}));

    let facets = row.facets.clone().unwrap_or(serde_json::json!({}));

    let job_version = row.job_version.map(|version| api::JobVersionLink {
        namespace: row.namespace_name.clone().unwrap_or_default(),
        name: row.job_name.clone().unwrap_or_default(),
        version,
    });

    // Build dataset facet map keyed by internal dataset_version_uuid
    let facet_map = build_dataset_facet_map(&row.dataset_facets);

    let input_dataset_versions =
        format_dataset_versions_with_facets(&row.input_versions, &facet_map);
    let output_dataset_versions =
        format_dataset_versions_with_facets(&row.output_versions, &facet_map);

    api::Run {
        id: RunId::new(row.uuid),
        created_at: row.created_at,
        updated_at: row.updated_at,
        nominal_start_time: row.nominal_start_time,
        nominal_end_time: row.nominal_end_time,
        state: row
            .current_run_state
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(RunState::New),
        started_at: row.started_at,
        ended_at: row.ended_at,
        duration_ms,
        args,
        job_version,
        input_dataset_versions,
        output_dataset_versions,
        facets,
    }
}

/// Build a map from internal `dataset_version_uuid` → merged facets object.
///
/// Input: raw `dataset_facets` JSON array with entries like
/// `{dataset_version_uuid, name, type, facet}` where `facet` is a wrapped
/// facet object like `{"facetName": {...}}`.
fn build_dataset_facet_map(
    raw: &Option<serde_json::Value>,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut map: std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>> =
        std::collections::HashMap::new();

    let Some(arr) = raw.as_ref().and_then(|v| v.as_array()) else {
        return std::collections::HashMap::new();
    };

    for entry in arr {
        let dv_uuid = entry["dataset_version_uuid"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if dv_uuid.is_empty() {
            continue;
        }
        // Each facet entry's `facet` field is a wrapped JSON like {"facetName": {...}}.
        // Expand and merge all keys.
        if let Some(facet_obj) = entry["facet"].as_object() {
            let merged = map.entry(dv_uuid).or_default();
            for (k, v) in facet_obj {
                merged.insert(k.clone(), v.clone());
            }
        }
    }

    map.into_iter()
        .map(|(k, v)| (k, serde_json::Value::Object(v)))
        .collect()
}

/// Transform raw SQL JSON array `[{namespace, name, version, dataset_version_uuid}]`
/// into API format `[{datasetVersionId: {namespace, name, version}, facets: {...}}]`,
/// patching in dataset facets by matching `dataset_version_uuid`.
fn format_dataset_versions_with_facets(
    raw: &Option<serde_json::Value>,
    facet_map: &std::collections::HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    let Some(arr) = raw.as_ref().and_then(|v| v.as_array()) else {
        return serde_json::json!([]);
    };
    let formatted: Vec<serde_json::Value> = arr
        .iter()
        .map(|entry| {
            let dv_uuid = entry["dataset_version_uuid"].as_str().unwrap_or_default();
            let facets = facet_map
                .get(dv_uuid)
                .cloned()
                .unwrap_or(serde_json::json!({}));
            serde_json::json!({
                "datasetVersionId": {
                    "namespace": entry["namespace"].as_str().unwrap_or_default(),
                    "name": entry["name"].as_str().unwrap_or_default(),
                    "version": entry["version"]
                },
                "facets": facets
            })
        })
        .collect();
    serde_json::json!(formatted)
}

/// Compute SHA-256 hex digest of a string.
fn sha256_checksum(s: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

/// Compute SHA256 checksum for run args using Java's KV-joined format.
///
/// Java's `Utils.checksumFor()` uses `Hashing.sha256().hashString(KV_JOINER.join(kvMap))`
/// where KV_JOINER is `Joiner.on("#").withKeyValueSeparator("=")`.
fn kv_checksum_from_json(args: &serde_json::Value) -> String {
    let mut sorted = std::collections::BTreeMap::new();
    if let Some(obj) = args.as_object() {
        for (k, v) in obj {
            sorted.insert(k.as_str(), v.as_str().unwrap_or(""));
        }
    }
    let kv_string: String = sorted
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("#");
    sha256_checksum(&kv_string)
}
