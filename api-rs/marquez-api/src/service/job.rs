use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api;
use crate::models::common::{
    DatasetId, DatasetName, JobName, JobType, JobVersionId, NamespaceName, RunId, RunState,
};
use crate::models::db::EnrichedJobVersionRow;

pub struct JobService {
    pool: PgPool,
}

impl JobService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_or_update(
        &self,
        ns_name: &str,
        job_name: &str,
        type_: &str,
        description: Option<&str>,
        location: Option<&str>,
    ) -> Result<api::Job, AppError> {
        let ns = db::namespace::find_by_name(&self.pool, ns_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Namespace '{}' not found", ns_name)))?;

        let now = Utc::now();
        let row = db::job::upsert(
            &self.pool,
            Uuid::new_v4(),
            type_,
            now,
            ns.uuid,
            ns_name,
            job_name,
            description,
            location,
            None,
            Some(job_name), // simple_name
            None,           // parent_job_uuid
            None,           // current_run_uuid
        )
        .await?;

        // Create a job version (matches Java PUT behavior)
        let jv_uuid = Uuid::new_v4();
        let version_uuid = Uuid::new_v4();
        db::job_version::upsert(
            &self.pool,
            jv_uuid,
            now,
            row.uuid,
            location,
            version_uuid,
            None, // job_context_uuid
            Some(ns.uuid),
            ns_name,
            job_name,
        )
        .await?;

        // Update job's current_version_uuid
        db::job::update_version(&self.pool, row.uuid, now, jv_uuid).await?;

        // Re-fetch to get updated current_version_uuid
        let row = db::job::find_by_name(&self.pool, ns_name, job_name)
            .await?
            .unwrap_or(row);

        self.enrich_job(row).await
    }

    pub async fn list_all(
        &self,
        limit: i32,
        offset: i32,
        states: &[String],
    ) -> Result<Vec<api::Job>, AppError> {
        let rows = db::job::find_all_jobs(&self.pool, limit, offset, states).await?;
        self.enrich_jobs_batch(rows).await
    }

    pub async fn count_all(&self) -> Result<i64, AppError> {
        Ok(db::job::count_all(&self.pool).await?)
    }

    pub async fn get(&self, ns_name: &str, job_name: &str) -> Result<api::Job, AppError> {
        let row = db::job::find_by_name(&self.pool, ns_name, job_name)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Job '{}/{}' not found", ns_name, job_name))
            })?;
        self.enrich_job(row).await
    }

    pub async fn get_by(
        &self,
        ns_name: &str,
        job_name: &str,
    ) -> Result<crate::models::db::JobRow, AppError> {
        db::job::find_by_name(&self.pool, ns_name, job_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Job '{}/{}' not found", ns_name, job_name)))
    }

    pub async fn list(
        &self,
        ns_name: &str,
        limit: i32,
        offset: i32,
        states: &[String],
    ) -> Result<Vec<api::Job>, AppError> {
        let rows = db::job::find_all(&self.pool, ns_name, limit, offset, states).await?;
        self.enrich_jobs_batch(rows).await
    }

    pub async fn count(&self, ns_name: &str) -> Result<i64, AppError> {
        Ok(db::job::count(&self.pool, ns_name).await?)
    }

    pub async fn delete(&self, ns_name: &str, job_name: &str) -> Result<(), AppError> {
        db::job::delete(&self.pool, ns_name, job_name).await?;
        Ok(())
    }

    pub async fn get_version(
        &self,
        ns_name: &str,
        job_name: &str,
        version: Uuid,
    ) -> Result<api::JobVersion, AppError> {
        let row =
            db::job_version::find_job_version_enriched(&self.pool, ns_name, job_name, version)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Job version '{}' not found", version))
                })?;
        Ok(map_enriched_job_version_to_api(&row))
    }

    pub async fn list_versions(
        &self,
        ns_name: &str,
        job_name: &str,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<api::JobVersion>, AppError> {
        let rows = db::job_version::find_all_job_versions_enriched(
            &self.pool, ns_name, job_name, limit, offset,
        )
        .await?;
        Ok(rows.iter().map(map_enriched_job_version_to_api).collect())
    }

    pub async fn count_versions(&self, ns_name: &str, job_name: &str) -> Result<i64, AppError> {
        Ok(db::job_version::count(&self.pool, ns_name, job_name).await?)
    }

    pub async fn tag_job(
        &self,
        ns_name: &str,
        job_name: &str,
        tag_name: &str,
    ) -> Result<api::Job, AppError> {
        let job_row = self.get_by(ns_name, job_name).await?;
        let tag_row = db::tag::find_by_name(&self.pool, tag_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tag '{}' not found", tag_name)))?;

        db::job::update_job_tag(&self.pool, job_row.uuid, tag_row.uuid, Utc::now()).await?;
        self.get(ns_name, job_name).await
    }

    pub async fn untag_job(
        &self,
        ns_name: &str,
        job_name: &str,
        tag_name: &str,
    ) -> Result<api::Job, AppError> {
        let job_row = self.get_by(ns_name, job_name).await?;
        let tag_row = db::tag::find_by_name(&self.pool, tag_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tag '{}' not found", tag_name)))?;

        db::job::delete_job_tag(&self.pool, job_row.uuid, tag_row.uuid).await?;
        self.get(ns_name, job_name).await
    }

    // ---- Private enrichment ----

    /// Batch-enrich a list of jobs with runs, tags, and facets using ~4 batch
    /// queries instead of ~2*N per-job queries.
    ///
    /// 1. Batch-fetch tags (already batched)
    /// 2. Batch-fetch latest run UUIDs for all jobs (1 query)
    /// 3. Batch-enrich all run UUIDs via LATERAL JOINs (1 query)
    /// 4. Batch-fetch job facets for all runs (1 query)
    async fn enrich_jobs_batch(
        &self,
        rows: Vec<crate::models::db::JobRow>,
    ) -> Result<Vec<api::Job>, AppError> {
        if rows.is_empty() {
            return Ok(vec![]);
        }

        let job_uuids: Vec<Uuid> = rows.iter().map(|r| r.uuid).collect();

        // Step 0: Batch-fetch parent job names
        let parent_uuids: Vec<Uuid> = rows.iter().filter_map(|r| r.parent_job_uuid).collect();
        let parent_names = db::job::find_names_by_uuids(&self.pool, &parent_uuids).await?;

        // Step 1: Batch-fetch tags (1 query)
        let tags_map = db::job::find_tags_batch(&self.pool, &job_uuids).await?;

        // Step 2: Batch-fetch latest run UUIDs for all jobs (1 query)
        let run_uuid_tuples =
            db::run::find_latest_run_uuids_for_jobs(&self.pool, &job_uuids, 10).await?;

        let all_run_uuids: Vec<Uuid> = run_uuid_tuples.iter().map(|(uuid, _, _)| *uuid).collect();

        // Step 3: Batch-enrich all run UUIDs with facets (1 query)
        let enriched_rows = if !all_run_uuids.is_empty() {
            db::run::find_runs_by_uuids_with_facets(&self.pool, &all_run_uuids).await?
        } else {
            vec![]
        };

        // Build a map: run_uuid → enriched row
        let enriched_by_uuid: std::collections::HashMap<
            Uuid,
            &crate::models::db::ExtendedRunWithFacetsRow,
        > = enriched_rows.iter().map(|r| (r.uuid, r)).collect();

        // Group run UUIDs by (namespace, job_name) preserving order
        let mut runs_by_job: std::collections::HashMap<(String, String), Vec<Uuid>> =
            std::collections::HashMap::new();
        for (run_uuid, job_name, namespace_name) in &run_uuid_tuples {
            runs_by_job
                .entry((namespace_name.clone(), job_name.clone()))
                .or_default()
                .push(*run_uuid);
        }

        // Step 4: Batch-fetch job facets for all runs (1 query)
        let all_facet_run_uuids: Vec<Uuid> = runs_by_job
            .values()
            .filter_map(|uuids| uuids.first().copied())
            .chain(
                rows.iter()
                    .filter(|r| {
                        let key = (r.namespace_name.clone().unwrap_or_default(), r.name.clone());
                        !runs_by_job.contains_key(&key)
                    })
                    .filter_map(|r| r.current_run_uuid),
            )
            .collect();

        let facets_map = if !all_facet_run_uuids.is_empty() {
            db::facets::find_job_facets_by_runs_batch(&self.pool, &all_facet_run_uuids).await?
        } else {
            std::collections::HashMap::new()
        };

        // Step 5: Assemble jobs from pre-fetched data
        let mut jobs = Vec::with_capacity(rows.len());
        for row in rows {
            let mut job = api::Job::from(row.clone());
            let ns_name = row.namespace_name.clone().unwrap_or_default();
            let job_name = &row.name;
            let key = (ns_name.clone(), job_name.clone());

            // Populate parent job name from pre-fetched map
            if let Some(parent_uuid) = row.parent_job_uuid {
                job.parent_job_name = parent_names.get(&parent_uuid).cloned();
            }

            if let Some(run_uuids) = runs_by_job.get(&key) {
                // Build enriched runs from pre-fetched data, preserving order
                let latest_runs: Vec<api::Run> = run_uuids
                    .iter()
                    .filter_map(|uuid| enriched_by_uuid.get(uuid))
                    .map(|r| crate::service::run::map_extended_run_to_api(r))
                    .collect();

                if !latest_runs.is_empty() {
                    let latest_run = &latest_runs[0];

                    job.inputs = self
                        .get_distinct_dataset_ids_from_versions(&latest_run.input_dataset_versions)
                        .await;
                    job.outputs = self
                        .get_distinct_dataset_ids_from_versions(&latest_run.output_dataset_versions)
                        .await;

                    job.latest_run = Some(Box::new(latest_run.clone()));
                    job.latest_runs = latest_runs;
                }
            } else if let Some(version_uuid) = row.current_version_uuid {
                // Fallback: no runs found, use IO mappings from job version
                let input_mappings =
                    db::job_version::find_input_datasets(&self.pool, version_uuid).await?;
                let output_mappings =
                    db::job_version::find_output_datasets(&self.pool, version_uuid).await?;
                job.inputs = self.resolve_dataset_ids(&input_mappings).await?;
                job.outputs = self.resolve_dataset_ids(&output_mappings).await?;
            }

            // Tags from pre-fetched map
            if let Some(tags) = tags_map.get(&row.uuid) {
                job.tags = tags.clone();
            }

            // Facets from pre-fetched map
            let run_uuid_for_facets = runs_by_job
                .get(&key)
                .and_then(|uuids| uuids.first().copied())
                .or(row.current_run_uuid);
            if let Some(run_uuid) = run_uuid_for_facets {
                if let Some(f) = facets_map.get(&run_uuid) {
                    job.facets = f.clone();
                }
            }
            // Fallback: try facets by job_version_uuid for runless jobs
            if job.facets == serde_json::json!({}) {
                if let Some(version_uuid) = row.current_version_uuid {
                    if let Some(f) =
                        db::facets::find_job_facets_by_job_version(&self.pool, version_uuid).await?
                    {
                        job.facets = f;
                    }
                }
            }

            jobs.push(job);
        }

        Ok(jobs)
    }

    /// Enrich a job with inputs, outputs, latestRun, tags, and facets.
    ///
    /// Uses a single CTE+LATERAL query (`find_by_latest_job`) to fetch up to
    /// 10 fully-enriched runs in one round-trip, eliminating the previous N+1
    /// pattern (~60 queries per job → 1 query for runs).
    async fn enrich_job(&self, row: crate::models::db::JobRow) -> Result<api::Job, AppError> {
        let mut job = api::Job::from(row.clone());
        let ns_name = row.namespace_name.as_deref().unwrap_or_default();
        let job_name = &row.name;

        // Populate parent job name from parent_job_uuid
        if let Some(parent_uuid) = row.parent_job_uuid {
            job.parent_job_name = db::job::find_name_by_uuid(&self.pool, parent_uuid).await?;
        }

        // Single optimized query: fetch up to 10 fully-enriched runs for this job
        let enriched_rows =
            db::run::find_by_latest_job(&self.pool, ns_name, job_name, 10, 0).await?;

        if !enriched_rows.is_empty() {
            let latest_runs: Vec<api::Run> = enriched_rows
                .iter()
                .map(crate::service::run::map_extended_run_to_api)
                .collect();

            let latest_run = &latest_runs[0];

            // Get inputs/outputs from latest run's dataset versions
            job.inputs = self
                .get_distinct_dataset_ids_from_versions(&latest_run.input_dataset_versions)
                .await;
            job.outputs = self
                .get_distinct_dataset_ids_from_versions(&latest_run.output_dataset_versions)
                .await;

            job.latest_run = Some(Box::new(latest_run.clone()));
            job.latest_runs = latest_runs;
        } else {
            // Fallback: try IO mappings if no runs exist
            if let Some(version_uuid) = row.current_version_uuid {
                let input_mappings =
                    db::job_version::find_input_datasets(&self.pool, version_uuid).await?;
                let output_mappings =
                    db::job_version::find_output_datasets(&self.pool, version_uuid).await?;
                job.inputs = self.resolve_dataset_ids(&input_mappings).await?;
                job.outputs = self.resolve_dataset_ids(&output_mappings).await?;
            }
        }

        // Enrich with tags
        let tag_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT t.name FROM tags t \
             INNER JOIN jobs_tag_mapping jtm ON jtm.tag_uuid = t.uuid \
             WHERE jtm.job_uuid = $1 \
             ORDER BY t.name",
        )
        .bind(row.uuid)
        .fetch_all(&self.pool)
        .await?;
        job.tags = tag_rows.into_iter().map(|(name,)| name).collect();

        // Enrich with facets (from latest run via job_facets)
        let run_uuid_for_facets = enriched_rows
            .first()
            .map(|r| r.uuid)
            .or(row.current_run_uuid);
        if let Some(run_uuid) = run_uuid_for_facets {
            if let Some(f) = db::facets::find_job_facets_by_run(&self.pool, run_uuid).await? {
                job.facets = f;
            }
        }
        // Fallback: try facets by job_version_uuid for runless jobs
        if job.facets == serde_json::json!({}) {
            if let Some(version_uuid) = row.current_version_uuid {
                if let Some(f) =
                    db::facets::find_job_facets_by_job_version(&self.pool, version_uuid).await?
                {
                    job.facets = f;
                }
            }
        }

        Ok(job)
    }

    /// Extract distinct DatasetIds from dataset version JSON array.
    async fn get_distinct_dataset_ids_from_versions(
        &self,
        versions: &serde_json::Value,
    ) -> Vec<DatasetId> {
        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        if let Some(arr) = versions.as_array() {
            for v in arr {
                let ns = v["datasetVersionId"]["namespace"]
                    .as_str()
                    .unwrap_or_default();
                let name = v["datasetVersionId"]["name"].as_str().unwrap_or_default();
                let key = format!("{}:{}", ns, name);
                if seen.insert(key) {
                    ids.push(DatasetId {
                        namespace: NamespaceName::new(ns),
                        name: DatasetName::new(name),
                    });
                }
            }
        }
        ids
    }

    /// Resolve IO mapping rows to DatasetIds by looking up dataset uuid -> name/namespace.
    ///
    /// Uses a single `WHERE uuid = ANY($1)` batch query instead of per-mapping lookups.
    async fn resolve_dataset_ids(
        &self,
        mappings: &[crate::models::db::JobVersionIoMappingRow],
    ) -> Result<Vec<DatasetId>, AppError> {
        let uuids: Vec<Uuid> = mappings.iter().filter_map(|m| m.dataset_uuid).collect();
        if uuids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT name, namespace_name FROM datasets WHERE uuid = ANY($1)")
                .bind(&uuids)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(name, ns_name)| DatasetId {
                namespace: NamespaceName::new(ns_name.unwrap_or_default()),
                name: DatasetName::new(&name),
            })
            .collect())
    }
}

/// Convert an `EnrichedJobVersionRow` (from the single CTE query) into an
/// `api::JobVersion` without any additional database queries.
fn map_enriched_job_version_to_api(row: &EnrichedJobVersionRow) -> api::JobVersion {
    let ns = row.namespace_name.clone().unwrap_or_default();
    let name = row.job_name.clone().unwrap_or_default();

    // Parse I/O datasets from JSON aggregates
    let inputs = parse_dataset_ids(&row.input_datasets);
    let outputs = parse_dataset_ids(&row.output_datasets);

    // Build latest run if present
    let latest_run = row.run_uuid.map(|run_uuid| {
        let duration_ms = match (row.run_started_at, row.run_ended_at) {
            (Some(start), Some(end)) => Some((end - start).num_milliseconds()),
            _ => None,
        };

        let args = row
            .run_args
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));

        // For job versions, use job facets (run_facets from the CTE) — matches Java
        let facets = merge_facets_array(&row.run_facets);

        let job_version = Some(api::JobVersionLink {
            namespace: ns.clone(),
            name: name.clone(),
            version: row.version,
        });

        let input_versions = format_run_versions(&row.run_input_versions);
        let output_versions = format_run_versions(&row.run_output_versions);

        Box::new(api::Run {
            id: RunId::new(run_uuid),
            created_at: row.run_created_at.unwrap_or_default(),
            updated_at: row.run_updated_at.unwrap_or_default(),
            nominal_start_time: row.run_nominal_start_time,
            nominal_end_time: row.run_nominal_end_time,
            state: row
                .run_current_run_state
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(RunState::New),
            started_at: row.run_started_at,
            ended_at: row.run_ended_at,
            duration_ms,
            args,
            job_version,
            input_dataset_versions: input_versions,
            output_dataset_versions: output_versions,
            facets,
        })
    });

    api::JobVersion {
        id: JobVersionId {
            namespace: NamespaceName::new(&ns),
            name: JobName::new(&name),
            version: row.version,
        },
        type_: JobType::Batch,
        name,
        created_at: row.created_at,
        version: row.version,
        namespace: ns,
        inputs,
        outputs,
        location: row.location.clone(),
        latest_run,
    }
}

/// Parse JSON array of `{namespace, name}` objects into `Vec<DatasetId>`.
fn parse_dataset_ids(json: &Option<serde_json::Value>) -> Vec<DatasetId> {
    let Some(arr) = json.as_ref().and_then(|v| v.as_array()) else {
        return vec![];
    };
    arr.iter()
        .map(|v| DatasetId {
            namespace: NamespaceName::new(v["namespace"].as_str().unwrap_or_default()),
            name: DatasetName::new(v["name"].as_str().unwrap_or_default()),
        })
        .collect()
}

/// Merge a JSON array of facet objects into a single flat JSON object
/// (last-wins semantics, matching Java's MapperUtils).
fn merge_facets_array(json: &Option<serde_json::Value>) -> serde_json::Value {
    let Some(arr) = json.as_ref().and_then(|v| v.as_array()) else {
        return serde_json::json!({});
    };
    let mut merged = serde_json::Map::new();
    for facet in arr {
        if let Some(obj) = facet.as_object() {
            for (k, v) in obj {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    serde_json::Value::Object(merged)
}

/// Format raw SQL JSON array `[{namespace, name, version}]` into API format
/// `[{datasetVersionId: {namespace, name, version}, facets: {}}]`.
fn format_run_versions(raw: &Option<serde_json::Value>) -> serde_json::Value {
    let Some(arr) = raw.as_ref().and_then(|v| v.as_array()) else {
        return serde_json::json!([]);
    };
    let formatted: Vec<serde_json::Value> = arr
        .iter()
        .map(|entry| {
            serde_json::json!({
                "datasetVersionId": {
                    "namespace": entry["namespace"].as_str().unwrap_or_default(),
                    "name": entry["name"].as_str().unwrap_or_default(),
                    "version": entry["version"]
                },
                "facets": {}
            })
        })
        .collect();
    serde_json::json!(formatted)
}
