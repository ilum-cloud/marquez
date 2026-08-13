// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for `lineage_events` table and OpenLineage model orchestration.
//!
//! This module contains two parts:
//!
//! 1. **SQL event storage** -- direct CRUD on the `lineage_events` table.
//! 2. **Orchestration logic** -- receives OpenLineage events and creates/updates
//!    the Marquez model by calling other DAO modules within a transaction.

use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use crate::models::db::{
    DatasetFieldRow, DatasetRow, DatasetSymlinkRow, DatasetVersionRow, JobRow, LineageEventRow,
    NamespaceRow,
};
use crate::models::openlineage::{InputDatasetRef, LineageEvent, OutputDatasetRef};

use serde::Serialize;

/// Default source name used when no explicit source is provided.
pub const DEFAULT_SOURCE_NAME: &str = "default";

/// Default namespace owner for auto-created namespaces.
pub const DEFAULT_NAMESPACE_OWNER: &str = "anonymous";

// ---------------------------------------------------------------------------
// Part 1: SQL event storage
// ---------------------------------------------------------------------------

/// Insert a lineage event (run event) into the `lineage_events` table.
///
/// Note: `run_uuid` is NOT stored in the table (dropped in V36). The run
/// reference is embedded in the JSONB `event` payload.
pub async fn create_lineage_event(
    exec: impl Executor<'_, Database = Postgres>,
    event_type: &str,
    event_time: DateTime<Utc>,
    job_name: &str,
    job_namespace: &str,
    event: &serde_json::Value,
    producer: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO lineage_events \
         (event_type, event_time, job_name, job_namespace, event, producer, _event_type) \
         VALUES ($1, $2, $3, $4, $5, $6, 'RUN_EVENT')",
    )
    .bind(event_type)
    .bind(event_time)
    .bind(job_name)
    .bind(job_namespace)
    .bind(event)
    .bind(producer)
    .execute(exec)
    .await?;
    Ok(())
}

/// Insert a dataset event into the `lineage_events` table.
pub async fn create_dataset_event(
    exec: impl Executor<'_, Database = Postgres>,
    event_time: DateTime<Utc>,
    event: &serde_json::Value,
    producer: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO lineage_events \
         (event_time, event, producer, _event_type) \
         VALUES ($1, $2, $3, 'DATASET_EVENT')",
    )
    .bind(event_time)
    .bind(event)
    .bind(producer)
    .execute(exec)
    .await?;
    Ok(())
}

/// Insert a job event into the `lineage_events` table.
pub async fn create_job_event(
    exec: impl Executor<'_, Database = Postgres>,
    event_time: DateTime<Utc>,
    job_name: &str,
    job_namespace: &str,
    event: &serde_json::Value,
    producer: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO lineage_events \
         (event_time, job_name, job_namespace, event, producer, _event_type) \
         VALUES ($1, $2, $3, $4, $5, 'JOB_EVENT')",
    )
    .bind(event_time)
    .bind(job_name)
    .bind(job_namespace)
    .bind(event)
    .bind(producer)
    .execute(exec)
    .await?;
    Ok(())
}

/// Find lineage events by job name and namespace (useful for run events).
pub async fn find_events_by_job(
    pool: &PgPool,
    job_name: &str,
    job_namespace: &str,
) -> Result<Vec<LineageEventRow>, sqlx::Error> {
    sqlx::query_as::<_, LineageEventRow>(
        "SELECT * FROM lineage_events \
         WHERE job_name = $1 AND job_namespace = $2 \
         ORDER BY event_time DESC",
    )
    .bind(job_name)
    .bind(job_namespace)
    .fetch_all(pool)
    .await
}

/// Find lineage events by run UUID.
///
/// Java's `OpenLineageDao.findLineageEventsByRunUuid()` queries `WHERE run_uuid = :runUuid`,
/// but the `run_uuid` column was dropped in V36. This implementation extracts the run ID
/// from the JSONB event payload instead, which works with the current schema.
pub async fn find_lineage_events_by_run_uuid(
    pool: &PgPool,
    run_uuid: Uuid,
) -> Result<Vec<LineageEventRow>, sqlx::Error> {
    sqlx::query_as::<_, LineageEventRow>(
        "SELECT * FROM lineage_events \
         WHERE event->>'run'->>'runId' IS NOT NULL \
         AND event->'run'->>'runId' = $1 \
         AND _event_type = 'RUN_EVENT' \
         ORDER BY event_time DESC",
    )
    .bind(run_uuid.to_string())
    .fetch_all(pool)
    .await
}

/// Get all events descending with cursor-based pagination (before/after timestamps).
///
/// Matches Java: filters `_event_type='RUN_EVENT'` (only run events returned).
pub async fn get_all_events_desc(
    pool: &PgPool,
    before: DateTime<Utc>,
    after: DateTime<Utc>,
    limit: i32,
    offset: i32,
) -> Result<Vec<LineageEventRow>, sqlx::Error> {
    sqlx::query_as::<_, LineageEventRow>(
        "SELECT * FROM lineage_events \
         WHERE event_time < $1 AND event_time >= $2 \
         AND _event_type = 'RUN_EVENT' \
         ORDER BY event_time DESC \
         LIMIT $3 OFFSET $4",
    )
    .bind(before)
    .bind(after)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Get all events ascending with cursor-based pagination (before/after timestamps).
///
/// Matches Java: filters `_event_type='RUN_EVENT'` (only run events returned).
pub async fn get_all_events_asc(
    pool: &PgPool,
    before: DateTime<Utc>,
    after: DateTime<Utc>,
    limit: i32,
    offset: i32,
) -> Result<Vec<LineageEventRow>, sqlx::Error> {
    sqlx::query_as::<_, LineageEventRow>(
        "SELECT * FROM lineage_events \
         WHERE event_time < $1 AND event_time >= $2 \
         AND _event_type = 'RUN_EVENT' \
         ORDER BY event_time ASC \
         LIMIT $3 OFFSET $4",
    )
    .bind(before)
    .bind(after)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Count total events within a time range.
pub async fn get_total_count(
    pool: &PgPool,
    before: DateTime<Utc>,
    after: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM lineage_events \
         WHERE event_time < $1 AND event_time >= $2 AND _event_type = 'RUN_EVENT'",
    )
    .bind(before)
    .bind(after)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Part 2: Utility functions
// ---------------------------------------------------------------------------

/// Map an OpenLineage event type string to a Marquez run state.
///
/// Matches Java behavior: unknown event types default to RUNNING (not OTHER).
/// See Java `OpenLineageDao.getRunState()` (OpenLineageDao.java:1103-1119).
pub fn get_run_state(event_type: &str) -> String {
    match event_type.to_lowercase().as_str() {
        "complete" => "COMPLETED".to_string(),
        "abort" => "ABORTED".to_string(),
        "fail" => "FAILED".to_string(),
        "start" => "RUNNING".to_string(),
        _ => "RUNNING".to_string(),
    }
}

/// Normalize namespace name by replacing disallowed characters with `_`.
///
/// Matches Java behavior: `namespace.replaceAll("[^a-z:/A-Z0-9\\-_.@+]", "_")`
/// See Java `OpenLineageDao.formatNamespaceName()` (OpenLineageDao.java:833-835).
pub fn format_namespace_name(ns: &str) -> String {
    ns.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || ":/-_.@+".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Normalize dataset name (pass-through for now).
pub fn format_dataset_name(name: &str) -> String {
    name.to_string()
}

/// Derive source type from a dataset reference.
///
/// Checks `facets.dataSource.uri` to determine source type. Falls back to
/// `"POSTGRESQL"`.
pub fn get_source_type(dataset: &serde_json::Value) -> String {
    dataset
        .get("facets")
        .and_then(|f| f.get("dataSource"))
        .and_then(|ds| ds.get("uri"))
        .and_then(|u| u.as_str())
        .map(get_source_type_from_uri)
        .unwrap_or_else(|| "POSTGRESQL".to_string())
}

/// Derive dataset type from a dataset reference (always DB_TABLE for now).
pub fn get_dataset_type(_dataset: &serde_json::Value) -> String {
    "DB_TABLE".to_string()
}

/// Parse a run ID string to a UUID. Falls back to a raw-MD5 UUID (matching
/// Java's `UUID.nameUUIDFromBytes()`) if it is not a valid UUID.
pub fn run_to_uuid(run_id: &str) -> Option<Uuid> {
    Uuid::parse_str(run_id)
        .ok()
        .or_else(|| Some(uuid_name_from_bytes(run_id.as_bytes())))
}

// ---------------------------------------------------------------------------
// Part 2b: Java-compatible UUID helpers
// ---------------------------------------------------------------------------

/// Match Java's `UUID.nameUUIDFromBytes(bytes)`.
///
/// Computes MD5 of the raw bytes and sets UUID version 3 + RFC 4122 variant
/// bits.  This is **not** the same as `Uuid::new_v3()`, which prepends
/// namespace bytes before hashing.
///
/// Used for dataset version UUIDs and job version UUIDs where Java joins
/// fields with `:` and hashes directly (no namespace prefix).
pub fn uuid_name_from_bytes(data: &[u8]) -> Uuid {
    let mut hash: [u8; 16] = md5::compute(data).0;
    hash[6] = (hash[6] & 0x0f) | 0x30; // version 3
    hash[8] = (hash[8] & 0x3f) | 0x80; // variant RFC4122
    Uuid::from_bytes(hash)
}

/// Look up `(namespace_name, name)` pairs for a list of dataset UUIDs.
///
/// Unknown UUIDs are silently skipped (mirrors Java's behaviour when the
/// dataset has been deleted between IO-mapping creation and version
/// computation).
pub async fn lookup_dataset_names(
    exec: impl Executor<'_, Database = Postgres>,
    uuids: &[Uuid],
) -> Result<Vec<(String, String)>, sqlx::Error> {
    if uuids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT namespace_name, name FROM datasets WHERE uuid = ANY($1)")
            .bind(uuids)
            .fetch_all(exec)
            .await?;
    Ok(rows)
}

/// Pure computation of job version UUID matching Java's
/// `Utils.newJobVersionFor()`.
///
/// Java joins `namespace : jobName : inputDs1Ns : inputDs1Name : … :
/// outputDs1Ns : outputDs1Name : … : location` with `:` (skipNulls) and
/// hashes the result with raw MD5 (`UUID.nameUUIDFromBytes`).
///
/// Input/output name pairs must already be sorted by `(ns, name)` to match
/// Java's natural `DatasetId` ordering.
pub fn compute_job_version_uuid_from_parts(
    ns_name: &str,
    job_name: &str,
    sorted_input_names: &[(String, String)],
    sorted_output_names: &[(String, String)],
    location: Option<&str>,
) -> Uuid {
    let mut parts: Vec<&str> = vec![ns_name, job_name];
    // Java's collect(joining(":")) on an empty stream produces "" which
    // VERSION_JOINER includes (it only skips null, not empty strings).
    // We must push "" for empty sets to match.
    if sorted_input_names.is_empty() {
        parts.push("");
    } else {
        for (ns, name) in sorted_input_names {
            parts.push(ns);
            parts.push(name);
        }
    }
    if sorted_output_names.is_empty() {
        parts.push("");
    } else {
        for (ns, name) in sorted_output_names {
            parts.push(ns);
            parts.push(name);
        }
    }
    if let Some(loc) = location {
        parts.push(loc);
    }
    let version_string = parts.join(":");
    uuid_name_from_bytes(version_string.as_bytes())
}

/// Compute a job version UUID matching Java's `Utils.newJobVersionFor()`.
///
/// Convenience wrapper that looks up dataset names from UUIDs, sorts them,
/// and delegates to [`compute_job_version_uuid_from_parts`].
pub async fn compute_job_version_uuid(
    pool: &PgPool,
    ns_name: &str,
    job_name: &str,
    input_ds_uuids: &[Uuid],
    output_ds_uuids: &[Uuid],
    location: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let mut input_names = lookup_dataset_names(pool, input_ds_uuids).await?;
    let mut output_names = lookup_dataset_names(pool, output_ds_uuids).await?;
    input_names.sort();
    output_names.sort();
    Ok(compute_job_version_uuid_from_parts(
        ns_name,
        job_name,
        &input_names,
        &output_names,
        location,
    ))
}

// ---------------------------------------------------------------------------
// Part 3: Orchestration logic
// ---------------------------------------------------------------------------

/// Main entry point: process a `LineageEvent` (run event).
///
/// Wraps everything in a single transaction.
pub async fn update_marquez_model(pool: &PgPool, event: &LineageEvent) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    update_base_marquez_model(&mut tx, event).await?;
    tx.commit().await?;
    Ok(())
}

/// Process a `DatasetEvent`: upsert namespace, source, dataset, version, fields, facets.
///
/// Wraps everything in a single transaction.
pub async fn update_marquez_model_dataset_event(
    pool: &PgPool,
    event: &crate::models::openlineage::DatasetEvent,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let now = Utc::now();

    let ds = &event.dataset;
    let ds_namespace = format_namespace_name(&ds.namespace);

    // Use a dummy run UUID (no run for dataset events)
    let dummy_run_uuid = Uuid::nil();

    // Convert DatasetRef → OutputDatasetRef so we can reuse upsert_output_dataset(),
    // which handles column lineage and output facets processing.
    // Java's OpenLineageDao treats dataset events as outputs (isInput=false).
    let output_ref = OutputDatasetRef {
        namespace: ds_namespace.clone(),
        name: ds.name.clone(),
        facets: ds.facets.clone(),
        output_facets: ds.output_facets.clone(),
    };
    let ns_row = upsert_namespace_inline(&mut tx, now, &ds_namespace).await?;
    let (_ds_uuid, _dv_uuid) = upsert_output_dataset(
        &mut tx,
        &output_ref,
        &ns_row,
        dummy_run_uuid,
        now,
        event.event_time,
        "DATASET_EVENT",
    )
    .await?;

    // Store the raw event
    let event_json = serde_json::to_value(event).unwrap_or_default();
    create_dataset_event(&mut *tx, event.event_time, &event_json, &event.producer).await?;

    tx.commit().await?;
    Ok(())
}

/// Process a `JobEvent`: upsert namespace, job, datasets, runless job version, facets.
///
/// Java fires two operations in parallel for job events:
/// 1. `createJobEvent()` — INSERT into `lineage_events`
/// 2. `updateMarquezModel(JobEvent)` — full orchestration
///
/// Wraps everything in a single transaction.
pub async fn update_marquez_model_job_event(
    pool: &PgPool,
    event: &crate::models::openlineage::JobEvent,
) -> Result<(), sqlx::Error> {
    let ns_name = format_namespace_name(&event.job.namespace);
    let now = Utc::now();

    // 1. Store the raw event
    let event_json = serde_json::to_value(event).unwrap_or_default();
    create_job_event(
        pool,
        event.event_time,
        &event.job.name,
        &ns_name,
        &event_json,
        &event.producer,
    )
    .await?;

    // 2. Full model orchestration (matches Java's updateMarquezModel(JobEvent))
    let mut tx = pool.begin().await?;

    // 2a. Upsert namespace
    let ns_row = upsert_namespace_inline(&mut tx, now, &ns_name).await?;

    // 2b. Extract job metadata from facets
    let description = event
        .job
        .facets
        .as_ref()
        .and_then(|f| f.get("documentation"))
        .and_then(|d| d.get("description"))
        .and_then(|d| d.as_str());
    let location = event
        .job
        .facets
        .as_ref()
        .and_then(|f| f.get("sourceCodeLocation"))
        .and_then(|d| d.get("url"))
        .and_then(|d| d.as_str());
    let job_type = get_job_type_from_facets(event.job.facets.as_ref());
    let job_name = &event.job.name;

    // 2c. Upsert job
    let job_row = crate::db::job::upsert(
        &mut *tx,
        Uuid::new_v4(),
        &job_type,
        now,
        ns_row.uuid,
        &ns_name,
        job_name,
        description,
        location,
        None,
        Some(job_name.as_str()),
        None,
        None,
    )
    .await?;

    // 2d. Process input datasets (use Uuid::nil() as run_uuid since JobEvent has no run)
    let nil_run = Uuid::nil();
    let mut input_dataset_pairs: Vec<(Uuid, Uuid)> = Vec::new();
    if let Some(inputs) = &event.inputs {
        for input in inputs {
            let (ds_uuid, dv_uuid) = upsert_input_dataset(
                &mut tx,
                input,
                &ns_row,
                nil_run,
                now,
                event.event_time,
                "JOB_EVENT",
            )
            .await?;
            input_dataset_pairs.push((ds_uuid, dv_uuid));
        }
    }

    // 2e. Process output datasets
    let mut output_dataset_pairs: Vec<(Uuid, Uuid)> = Vec::new();
    if let Some(outputs) = &event.outputs {
        for output in outputs {
            let (ds_uuid, dv_uuid) = upsert_output_dataset(
                &mut tx,
                output,
                &ns_row,
                nil_run,
                now,
                event.event_time,
                "JOB_EVENT",
            )
            .await?;
            output_dataset_pairs.push((ds_uuid, dv_uuid));
        }
    }

    // 2f. Upsert runless job version
    let input_ds_uuids: Vec<Uuid> = input_dataset_pairs.iter().map(|(u, _)| *u).collect();
    let output_ds_uuids: Vec<Uuid> = output_dataset_pairs.iter().map(|(u, _)| *u).collect();
    let mut input_names = lookup_dataset_names(&mut *tx, &input_ds_uuids).await?;
    let mut output_names = lookup_dataset_names(&mut *tx, &output_ds_uuids).await?;
    input_names.sort();
    output_names.sort();
    let version_uuid = compute_job_version_uuid_from_parts(
        &ns_name,
        job_name,
        &input_names,
        &output_names,
        location,
    );

    let jv_row = crate::db::job_version::upsert(
        &mut *tx,
        Uuid::new_v4(),
        now,
        job_row.uuid,
        location,
        version_uuid,
        None,
        Some(ns_row.uuid),
        &ns_name,
        job_name,
    )
    .await?;
    crate::db::job::update_version(&mut *tx, job_row.uuid, now, jv_row.uuid).await?;

    // 2g. Create IO mappings
    for (ds_uuid, _) in &input_dataset_pairs {
        upsert_io_mapping_inline(
            &mut tx,
            jv_row.uuid,
            *ds_uuid,
            job_row.uuid,
            "INPUT",
            now,
            job_row.symlink_target_uuid,
        )
        .await?;
    }
    for (ds_uuid, _) in &output_dataset_pairs {
        upsert_io_mapping_inline(
            &mut tx,
            jv_row.uuid,
            *ds_uuid,
            job_row.uuid,
            "OUTPUT",
            now,
            job_row.symlink_target_uuid,
        )
        .await?;
    }

    // 2h. Save job facets (use insert_job_facet_for_version so facets
    //     are keyed by job_version_uuid and retrievable by version lookup)
    if let Some(facets) = &event.job.facets {
        for (name, facet) in facets {
            let wrapped = wrap_facet(name, facet);
            crate::db::facets::insert_job_facet_for_version(
                &mut *tx,
                now,
                job_row.uuid,
                jv_row.uuid,
                event.event_time,
                name,
                &wrapped,
            )
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

/// Core orchestrator for a `LineageEvent`:
///
/// 1. Upsert namespace
/// 2. Resolve parent job (if parent facet present)
/// 3. Upsert job (via jobs_view)
/// 4. Upsert run + run_state
/// 5. Process input datasets
/// 6. Process output datasets
/// 7. Update job version
/// 8. Insert facets (job, run, dataset)
pub async fn update_base_marquez_model(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    event: &LineageEvent,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let event_type = event.event_type.as_deref().unwrap_or("OTHER");
    let run_state = get_run_state(event_type);

    // Pre-compute streaming/terminal flags for IO cleanup and job version gating
    let is_streaming = is_streaming_job(event.job.facets.as_ref());
    let run_state_done = matches!(run_state.as_str(), "COMPLETED" | "ABORTED" | "FAILED");
    let inputs_empty = event.inputs.as_ref().map(|v| v.is_empty()).unwrap_or(true);
    let outputs_empty = event.outputs.as_ref().map(|v| v.is_empty()).unwrap_or(true);
    // Two separate guards for streaming jobs with no datasets:
    //
    // 1. IO cleanup: protect ALL streaming events with no datasets from
    //    clearing IO mappings (heartbeats should not wipe accumulated I/O).
    let is_streaming_no_datasets = is_streaming && inputs_empty && outputs_empty;
    //
    // 2. Version creation: only skip TERMINAL streaming events with no
    //    datasets (matches Java's isTerminalEventForStreamingJobWithNoDatasets).
    //    Non-terminal events (START, RUNNING) must still create job versions
    //    so that latestRun and the lineage graph are populated.
    let is_terminal_streaming_no_datasets =
        is_streaming && run_state_done && inputs_empty && outputs_empty;

    // 1. Upsert namespace (inline because namespace::upsert takes &PgPool)
    let ns_name = format_namespace_name(&event.job.namespace);
    let ns_row = upsert_namespace_inline(&mut *tx, now, &ns_name).await?;

    // Derive job type from facets (needed early for parent job resolution)
    let job_type = get_job_type_from_facets(event.job.facets.as_ref());

    // 2. Resolve parent job hierarchy
    let parent_info = resolve_parent_job(&mut *tx, event, &ns_row, now, &job_type).await?;

    // 3. Upsert job
    let job_name = &event.job.name;

    // Compute simple_name: if there's a parent, strip the parent name prefix + dot
    let simple_name = if let Some((ref parent_job_row, _)) = parent_info {
        if job_name.starts_with(&format!("{}.", parent_job_row.name)) {
            Some(job_name[parent_job_row.name.len() + 1..].to_string())
        } else {
            Some(job_name.clone())
        }
    } else {
        Some(job_name.clone())
    };

    let description = event
        .job
        .facets
        .as_ref()
        .and_then(|f| f.get("documentation"))
        .and_then(|d| d.get("description"))
        .and_then(|d| d.as_str());
    let location = event
        .job
        .facets
        .as_ref()
        .and_then(|f| f.get("sourceCodeLocation"))
        .and_then(|d| d.get("url"))
        .and_then(|d| d.as_str());

    let parent_job_uuid = parent_info.as_ref().map(|(pj, _)| pj.uuid);

    // 3. Compute run UUID early so we can pass it to job upsert
    let run_uuid = run_to_uuid(&event.run.run_id).unwrap_or_else(Uuid::new_v4);

    let job_row = crate::db::job::upsert(
        &mut **tx,
        Uuid::new_v4(),
        &job_type,
        now,
        ns_row.uuid,
        &ns_name,
        simple_name.as_deref().unwrap_or(job_name),
        description,
        location,
        None,
        simple_name.as_deref(),
        parent_job_uuid,
        Some(run_uuid),
    )
    .await?;
    let nominal_start = event
        .run
        .facets
        .as_ref()
        .and_then(|f| f.get("nominalTime"))
        .and_then(|t| t.get("nominalStartTime"))
        .and_then(|t| t.as_str())
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Utc));
    let nominal_end = event
        .run
        .facets
        .as_ref()
        .and_then(|f| f.get("nominalTime"))
        .and_then(|t| t.get("nominalEndTime"))
        .and_then(|t| t.as_str())
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Utc));

    let started_at = if run_state == "RUNNING" {
        Some(event.event_time)
    } else {
        None
    };
    let ended_at = if matches!(run_state.as_str(), "COMPLETED" | "ABORTED" | "FAILED") {
        Some(event.event_time)
    } else {
        None
    };

    let parent_run_uuid = parent_info.as_ref().map(|(_, pru)| *pru);

    // Extract run args from facets (matches Java OpenLineageDao.createRunArgs)
    let run_args_uuid = {
        let mut run_args_map = serde_json::Map::new();
        if let Some(ref facets) = event.run.facets {
            if let Some(nominal) = facets.get("nominalTime") {
                if let Some(start) = nominal.get("nominalStartTime").and_then(|v| v.as_str()) {
                    run_args_map.insert(
                        "nominal_start_time".into(),
                        serde_json::Value::String(start.to_string()),
                    );
                }
                if let Some(end) = nominal.get("nominalEndTime").and_then(|v| v.as_str()) {
                    run_args_map.insert(
                        "nominal_end_time".into(),
                        serde_json::Value::String(end.to_string()),
                    );
                }
            }
            if let Some(parent) = facets.get("parent").or_else(|| facets.get("parentRun")) {
                if let Some(run_id) = parent.pointer("/run/runId").and_then(|v| v.as_str()) {
                    run_args_map.insert(
                        "run_id".into(),
                        serde_json::Value::String(run_id.to_string()),
                    );
                }
                if let Some(name) = parent.pointer("/job/name").and_then(|v| v.as_str()) {
                    run_args_map.insert("name".into(), serde_json::Value::String(name.to_string()));
                }
                if let Some(ns) = parent.pointer("/job/namespace").and_then(|v| v.as_str()) {
                    run_args_map.insert(
                        "namespace".into(),
                        serde_json::Value::String(ns.to_string()),
                    );
                }
            }
        }
        if !run_args_map.is_empty() {
            let args_json = serde_json::Value::Object(run_args_map.clone()).to_string();
            let checksum = kv_checksum(&run_args_map);
            let row =
                upsert_run_args_inline(tx, Uuid::new_v4(), now, &args_json, &checksum).await?;
            Some(row.uuid)
        } else {
            None
        }
    };

    let _run_row = crate::db::run::upsert(
        &mut **tx,
        run_uuid,
        now,
        Some(job_row.uuid),
        None,
        parent_run_uuid,
        run_args_uuid,
        nominal_start,
        nominal_end,
        Some(&run_state),
        started_at,
        None,
        ended_at,
        None,
        &ns_name,
        // Canonical name from the jobs_view trigger (parent-qualified for jobs
        // created with a parent facet), not the raw event name: runs.job_name
        // must match jobs.name or list and count queries disagree.
        &job_row.name,
        location,
        None,
    )
    .await?;

    // 4. Insert run state
    let state_row = crate::db::run_state::upsert(
        &mut **tx,
        Uuid::new_v4(),
        event.event_time,
        run_uuid,
        &run_state,
    )
    .await?;

    // 5. Update run start/end state
    if run_state == "RUNNING" {
        crate::db::run::update_start_state(&mut **tx, run_uuid, event.event_time, state_row.uuid)
            .await?;
    }
    if matches!(run_state.as_str(), "COMPLETED" | "ABORTED" | "FAILED") {
        crate::db::run::update_end_state(&mut **tx, run_uuid, event.event_time, state_row.uuid)
            .await?;

        // Update last_modified_at on output datasets (matches Java RunService.markRunAs)
        let output_ds_uuids =
            crate::db::dataset::find_output_dataset_uuids_by_run(&mut **tx, run_uuid).await?;
        if !output_ds_uuids.is_empty() {
            crate::db::dataset::update_last_modified_at(
                &mut **tx,
                &output_ds_uuids,
                event.event_time,
            )
            .await?;
        }
    }

    // 6. Process input datasets (collect UUIDs for IO mappings after job version creation)
    let mut input_dataset_pairs: Vec<(Uuid, Uuid)> = Vec::new();
    if !inputs_empty {
        for input in event.inputs.as_ref().unwrap() {
            let (ds_uuid, dv_uuid) = upsert_input_dataset(
                &mut *tx,
                input,
                &ns_row,
                run_uuid,
                now,
                event.event_time,
                event_type,
            )
            .await?;
            input_dataset_pairs.push((ds_uuid, dv_uuid));
        }
    } else if !is_streaming_no_datasets {
        // No inputs (None or empty) — mark all current INPUT mappings as previous
        // for this job (matches Java OpenLineageDao.java:381-384)
        sqlx::query(
            "UPDATE job_versions_io_mapping \
             SET is_current_job_version = false \
             WHERE (job_uuid = $1 OR job_symlink_target_uuid = $1) \
             AND io_type = 'INPUT' AND is_current_job_version = true",
        )
        .bind(job_row.uuid)
        .execute(&mut **tx)
        .await?;
    }

    // 7. Process output datasets (collect UUIDs for IO mappings after job version creation)
    let mut output_dataset_pairs: Vec<(Uuid, Uuid)> = Vec::new();
    if !outputs_empty {
        for output in event.outputs.as_ref().unwrap() {
            let (ds_uuid, dv_uuid) = upsert_output_dataset(
                &mut *tx,
                output,
                &ns_row,
                run_uuid,
                now,
                event.event_time,
                event_type,
            )
            .await?;
            output_dataset_pairs.push((ds_uuid, dv_uuid));
        }
    } else if !is_streaming_no_datasets {
        // No outputs (None or empty) — mark all current OUTPUT mappings as previous
        // for this job (matches Java OpenLineageDao.java:397-400)
        sqlx::query(
            "UPDATE job_versions_io_mapping \
             SET is_current_job_version = false \
             WHERE (job_uuid = $1 OR job_symlink_target_uuid = $1) \
             AND io_type = 'OUTPUT' AND is_current_job_version = true",
        )
        .bind(job_row.uuid)
        .execute(&mut **tx)
        .await?;
    }

    // 7b. Update job's current_inputs JSON (matches Java behavior)
    if let Some(inputs) = &event.inputs {
        if !inputs.is_empty() {
            let inputs_json: Vec<serde_json::Value> = inputs
                .iter()
                .map(|i| serde_json::json!({"namespace": i.namespace, "name": i.name}))
                .collect();
            crate::db::job::update_current_inputs(
                &mut **tx,
                job_row.uuid,
                serde_json::Value::Array(inputs_json),
            )
            .await?;
        }
    }

    // 8. Insert run facets
    if let Some(facets) = &event.run.facets {
        for (name, facet) in facets {
            if name == "spark_unknown" {
                continue;
            }
            let wrapped = wrap_facet(name, facet);
            crate::db::facets::insert_run_facet(
                &mut **tx,
                now,
                run_uuid,
                event.event_time,
                event_type,
                name,
                &wrapped,
            )
            .await?;
        }
    }

    // 9. Insert job facets
    if let Some(facets) = &event.job.facets {
        for (name, facet) in facets {
            let wrapped = wrap_facet(name, facet);
            crate::db::facets::insert_job_facet(
                &mut **tx,
                now,
                job_row.uuid,
                run_uuid,
                event.event_time,
                Some(event_type),
                name,
                &wrapped,
            )
            .await?;
        }
    }

    // 10. Upsert job version — gated by streaming vs batch logic
    //     (matches Java OpenLineageDao.java:167-175)
    //
    // When the event has empty inputs/outputs (common for COMPLETE events in
    // the seed data pattern: START carries I/O, COMPLETE is bare), we must
    // query the DB for datasets accumulated by earlier events for this run.
    // Java does this in JobVersionDao.loadJobRowRunDetails() which always
    // queries findInputDatasetVersionsFor(runUuid) and
    // findOutputDatasetVersionsFor(runUuid).
    let effective_input_pairs: Vec<(Uuid, Uuid)> = if input_dataset_pairs.is_empty() {
        find_accumulated_input_datasets(tx, run_uuid).await?
    } else {
        input_dataset_pairs.clone()
    };
    let effective_output_pairs: Vec<(Uuid, Uuid)> = if output_dataset_pairs.is_empty() {
        find_accumulated_output_datasets(tx, run_uuid).await?
    } else {
        output_dataset_pairs.clone()
    };

    // Compute job version UUID matching Java's Utils.newJobVersionFor():
    // MD5(namespace:jobName:sortedInputNs:sortedInputName:...:sortedOutputNs:sortedOutputName:...:location)
    let input_ds_uuids: Vec<Uuid> = effective_input_pairs.iter().map(|(u, _)| *u).collect();
    let output_ds_uuids: Vec<Uuid> = effective_output_pairs.iter().map(|(u, _)| *u).collect();
    let mut input_names = lookup_dataset_names(&mut **tx, &input_ds_uuids).await?;
    let mut output_names = lookup_dataset_names(&mut **tx, &output_ds_uuids).await?;
    input_names.sort();
    output_names.sort();
    let version_uuid = compute_job_version_uuid_from_parts(
        &ns_name,
        job_name,
        &input_names,
        &output_names,
        location,
    );

    let should_create_job_version = if is_streaming {
        // Streaming: only create if version doesn't exist yet AND not a
        // terminal event with no datasets
        if is_terminal_streaming_no_datasets {
            false
        } else {
            !crate::db::job_version::version_exists_exec(&mut **tx, version_uuid).await?
        }
    } else {
        // Batch: only create when run is done
        event.event_type.is_some() && run_state_done
    };

    if should_create_job_version {
        let jv_row = crate::db::job_version::upsert(
            &mut **tx,
            Uuid::new_v4(),
            now,
            job_row.uuid,
            location,
            version_uuid,
            None,
            Some(ns_row.uuid),
            &ns_name,
            job_name,
        )
        .await?;
        crate::db::job_version::update_latest_run(&mut **tx, jv_row.uuid, run_uuid, now).await?;
        crate::db::run::update_job_version(&mut **tx, run_uuid, jv_row.uuid).await?;
        crate::db::job::update_version(&mut **tx, job_row.uuid, now, jv_row.uuid).await?;
        // Link job facets to this job version (matches Java JobVersionDao.upsertJobVersionOnRunTransition)
        crate::db::job_version::link_job_facets_to_job_version(&mut **tx, run_uuid, jv_row.uuid)
            .await?;

        // 10b. Query old datasets before creating new IO mappings (for lineage statistics)
        let old_inputs =
            crate::db::job_version::find_current_input_dataset_uuids(&mut **tx, job_row.uuid)
                .await?;
        let old_outputs =
            crate::db::job_version::find_current_output_dataset_uuids(&mut **tx, job_row.uuid)
                .await?;

        // 10c. Create IO mappings now that the job version exists.
        // Uses effective pairs (accumulated from DB when event I/O is empty).
        for (ds_uuid, _) in &effective_input_pairs {
            upsert_io_mapping_inline(
                &mut *tx,
                jv_row.uuid,
                *ds_uuid,
                job_row.uuid,
                "INPUT",
                now,
                job_row.symlink_target_uuid,
            )
            .await?;
        }
        for (ds_uuid, _) in &effective_output_pairs {
            upsert_io_mapping_inline(
                &mut *tx,
                jv_row.uuid,
                *ds_uuid,
                job_row.uuid,
                "OUTPUT",
                now,
                job_row.symlink_target_uuid,
            )
            .await?;
        }

        // 10d. Update lineage statistics for old datasets (uses current_version_uuid lookup)
        for ds in &old_inputs {
            crate::db::facets::update_lineage_statistics(&mut **tx, *ds).await?;
        }
        for ds in &old_outputs {
            crate::db::facets::update_lineage_statistics(&mut **tx, *ds).await?;
        }
        // Update lineage statistics for new datasets (explicit version)
        for (ds, dv) in &effective_input_pairs {
            crate::db::facets::update_lineage_statistics_versioned(&mut **tx, *ds, *dv).await?;
        }
        for (ds, dv) in &effective_output_pairs {
            crate::db::facets::update_lineage_statistics_versioned(&mut **tx, *ds, *dv).await?;
        }
    }

    // 11. Store the raw lineage event
    let event_json = serde_json::to_value(event).unwrap_or_default();
    create_lineage_event(
        &mut **tx,
        event_type,
        event.event_time,
        job_name,
        &ns_name,
        &event_json,
        &event.producer,
    )
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers (inlined to work inside transactions)
// ---------------------------------------------------------------------------

/// Upsert a namespace row using a transaction connection.
///
/// Inlined because `namespace::upsert` takes `&PgPool`.
async fn upsert_namespace_inline(
    conn: &mut sqlx::PgConnection,
    now: DateTime<Utc>,
    name: &str,
) -> Result<NamespaceRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name, current_owner_name) \
         VALUES ($1, $2, $2, $3, $4) \
         ON CONFLICT(name) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(now)
    .bind(name)
    .bind(DEFAULT_NAMESPACE_OWNER)
    .execute(&mut *conn)
    .await?;

    let row = sqlx::query_as::<_, NamespaceRow>("SELECT * FROM namespaces WHERE name = $1")
        .bind(name)
        .fetch_one(&mut *conn)
        .await?;

    // Undelete hidden namespaces when re-referenced by events
    if row.is_hidden.unwrap_or(false) {
        sqlx::query("UPDATE namespaces SET is_hidden = false, updated_at = $1 WHERE uuid = $2")
            .bind(now)
            .bind(row.uuid)
            .execute(&mut *conn)
            .await?;
    }

    Ok(row)
}

/// Resolve parent job hierarchy from run facets.
///
/// Checks for `parent` or `parentRun` (alias) facet in `event.run.facets`.
/// If found, upserts the parent namespace, parent job, and parent run.
/// Returns `Some((parent_job_row, parent_run_uuid))` or `None`.
async fn resolve_parent_job(
    conn: &mut sqlx::PgConnection,
    event: &LineageEvent,
    _ns_row: &NamespaceRow,
    now: DateTime<Utc>,
    job_type: &str,
) -> Result<Option<(JobRow, Uuid)>, sqlx::Error> {
    let run_facets = match &event.run.facets {
        Some(f) => f,
        None => return Ok(None),
    };

    // `parent` is the standard OpenLineage facet; `parentRun` is the legacy alias.
    // Check `parent` first to match Java behavior.
    let parent_facet = run_facets
        .get("parent")
        .or_else(|| run_facets.get("parentRun"));

    let parent_facet = match parent_facet {
        Some(f) => f,
        None => return Ok(None),
    };

    // Extract parent run ID and parent job info
    let parent_run_id = parent_facet
        .get("run")
        .and_then(|r| r.get("runId"))
        .and_then(|r| r.as_str());
    let parent_job_ns = parent_facet
        .get("job")
        .and_then(|j| j.get("namespace"))
        .and_then(|n| n.as_str());
    let parent_job_name = parent_facet
        .get("job")
        .and_then(|j| j.get("name"))
        .and_then(|n| n.as_str());

    let (parent_run_id, parent_job_ns, parent_job_name) =
        match (parent_run_id, parent_job_ns, parent_job_name) {
            (Some(rid), Some(ns), Some(name)) => (rid, ns, name),
            _ => return Ok(None),
        };

    // Resolve parent run UUID — match Java's Utils.toUuid(namespace, runId, dagName)
    // which for non-UUID IDs computes UUID v3 with NAMESPACE_URL and "namespace.dagName.runId"
    let mut parent_run_uuid = match Uuid::parse_str(parent_run_id) {
        Ok(u) => u,
        Err(_) => {
            let dag_name = parse_parent_job_name(parent_job_name);
            Uuid::new_v3(
                &Uuid::NAMESPACE_URL,
                format!("{}.{}.{}", parent_job_ns, dag_name, parent_run_id).as_bytes(),
            )
        }
    };

    // Determine parent job name: if parent name equals child name, extract parent prefix
    let resolved_parent_name = if parent_job_name == event.job.name {
        // Same name means the parent facet points to the dag-level name.
        // Parse parent name by stripping the last dot-separated component.
        parse_parent_job_name(parent_job_name)
    } else {
        parent_job_name.to_string()
    };

    // Upsert parent namespace (may differ from child namespace)
    let parent_ns_name = format_namespace_name(parent_job_ns);
    let parent_ns_row = upsert_namespace_inline(&mut *conn, now, &parent_ns_name).await?;

    // Upsert parent job (use child's job type, matching Java's job.type())
    let parent_job_row = crate::db::job::upsert(
        &mut *conn,
        Uuid::new_v4(),
        job_type,
        now,
        parent_ns_row.uuid,
        &parent_ns_name,
        &resolved_parent_name,
        None,
        None,
        None,
        Some(&resolved_parent_name),
        None, // parent of parent not tracked
        None, // no run for parent job
    )
    .await?;

    // Check for Airflow UUID conflict: if an existing run with this UUID
    // belongs to a different job, regenerate the UUID (matches Java OpenLineageDao.java:625-663)
    let existing_run: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT namespace_name, job_name FROM runs WHERE uuid = $1")
            .bind(parent_run_uuid)
            .fetch_optional(&mut *conn)
            .await?;

    if let Some((Some(existing_ns), Some(existing_job))) = existing_run {
        if existing_ns != parent_ns_name || existing_job != resolved_parent_name {
            // Match Java's Utils.toNameBasedUuid(ns, jobName, parentRunId):
            // UUID v3 with NAMESPACE_URL, parts joined by "."
            let new_uuid = Uuid::new_v3(
                &Uuid::NAMESPACE_URL,
                format!(
                    "{}.{}.{}",
                    parent_ns_name, resolved_parent_name, parent_run_uuid
                )
                .as_bytes(),
            );
            tracing::warn!(
                "Parent Run id {} has a different job '{}.{}' from facet '{}.{}'. \
                 Assuming Run UUID conflict, using new UUID {}",
                parent_run_uuid,
                existing_ns,
                existing_job,
                parent_ns_name,
                resolved_parent_name,
                new_uuid
            );
            parent_run_uuid = new_uuid;
        }
    }

    // Upsert parent run
    //
    // Pass None for started_at/ended_at so the upsert's ON CONFLICT COALESCE
    // preserves existing values. We then apply LEAST/GREATEST below to ensure
    // the parent run spans the full range of child events.
    let event_type = event.event_type.as_deref().unwrap_or("OTHER");
    let run_state = get_run_state(event_type);

    let _parent_run = crate::db::run::upsert(
        &mut *conn,
        parent_run_uuid,
        now,
        Some(parent_job_row.uuid),
        None,
        None,
        None,
        None,
        None,
        Some(&run_state),
        None, // started_at — set via LEAST below
        None,
        None, // ended_at — set via GREATEST below
        None,
        &parent_ns_name,
        &resolved_parent_name,
        None,
        Some(parent_run_id),
    )
    .await?;

    // Insert run state for parent
    let state_row = crate::db::run_state::upsert(
        &mut *conn,
        Uuid::new_v4(),
        event.event_time,
        parent_run_uuid,
        &run_state,
    )
    .await?;

    // Use LEAST/GREATEST to keep earliest started_at and latest ended_at
    // across all child events for this parent run.
    if run_state == "RUNNING" {
        sqlx::query(
            "UPDATE runs SET updated_at = $1, \
             started_at = LEAST(COALESCE(started_at, $1), $1), \
             start_run_state_uuid = $2 \
             WHERE uuid = $3",
        )
        .bind(event.event_time)
        .bind(state_row.uuid)
        .bind(parent_run_uuid)
        .execute(&mut *conn)
        .await?;
    }
    if matches!(run_state.as_str(), "COMPLETED" | "ABORTED" | "FAILED") {
        sqlx::query(
            "UPDATE runs SET updated_at = $1, \
             ended_at = GREATEST(COALESCE(ended_at, $1), $1), \
             end_run_state_uuid = $2 \
             WHERE uuid = $3",
        )
        .bind(event.event_time)
        .bind(state_row.uuid)
        .bind(parent_run_uuid)
        .execute(&mut *conn)
        .await?;
    }

    Ok(Some((parent_job_row, parent_run_uuid)))
}

/// Parse a parent job name by stripping the last dot-separated component.
///
/// e.g., `"dag.task_group.task1"` -> `"dag.task_group"`, `"dag.task1"` -> `"dag"`,
/// `"dag"` -> `"dag"` (no stripping).
fn parse_parent_job_name(name: &str) -> String {
    match name.rfind('.') {
        Some(idx) => name[..idx].to_string(),
        None => name.to_string(),
    }
}

/// Derive job type from job facets.
///
/// Checks for `jobType.processingType` facet. Defaults to `"BATCH"`.
fn get_job_type_from_facets(
    facets: Option<&std::collections::HashMap<String, serde_json::Value>>,
) -> String {
    facets
        .and_then(|f| f.get("jobType"))
        .and_then(|jt| jt.get("processingType"))
        .and_then(|pt| pt.as_str())
        .map(|s| {
            // Normalize OpenLineage processingType to Marquez JobType.
            // Only BATCH, STREAM, and SERVICE are valid — unknown values default
            // to BATCH, matching Java API behavior (LineageEvent.java:241-242).
            match s.to_uppercase().as_str() {
                "STREAMING" => "STREAM".to_string(),
                "BATCH" => "BATCH".to_string(),
                "SERVICE" => "SERVICE".to_string(),
                _ => "BATCH".to_string(),
            }
        })
        .unwrap_or_else(|| "BATCH".to_string())
}

/// Check if the job is a streaming job based on `jobType.processingType` facet.
///
/// Matches Java's check for `processingType == "STREAMING"` (case-insensitive).
fn is_streaming_job(facets: Option<&std::collections::HashMap<String, serde_json::Value>>) -> bool {
    facets
        .and_then(|f| f.get("jobType"))
        .and_then(|jt| jt.get("processingType"))
        .and_then(|pt| pt.as_str())
        .map(|s| s.eq_ignore_ascii_case("STREAMING"))
        .unwrap_or(false)
}

/// Derive source type from a connection URI scheme.
///
/// e.g., `"postgres://"` -> `"POSTGRESQL"`, `"s3://"` -> `"S3"`.
/// Defaults to `"POSTGRESQL"`.
pub fn get_source_type_from_uri(uri: &str) -> String {
    let lower = uri.to_lowercase();
    if lower.starts_with("postgres") {
        "POSTGRESQL".to_string()
    } else if lower.starts_with("mysql") {
        "MYSQL".to_string()
    } else if lower.starts_with("s3://") || lower.starts_with("s3a://") {
        "S3".to_string()
    } else if lower.starts_with("gs://") {
        "GCS".to_string()
    } else if lower.starts_with("bigquery") {
        "BIGQUERY".to_string()
    } else if lower.starts_with("kafka") {
        "KAFKA".to_string()
    } else if lower.starts_with("hdfs://") {
        "HDFS".to_string()
    } else {
        "POSTGRESQL".to_string()
    }
}

/// Upsert a dataset symlink row using a transaction connection.
///
/// Inlined because `dataset_version::upsert_symlink` takes `&PgPool`.
async fn upsert_symlink_inline(
    conn: &mut sqlx::PgConnection,
    dataset_uuid: Uuid,
    name: &str,
    namespace_uuid: Uuid,
    now: DateTime<Utc>,
) -> Result<DatasetSymlinkRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO dataset_symlinks \
         (dataset_uuid, name, namespace_uuid, is_primary, type, created_at, updated_at) \
         VALUES ($1, $2, $3, true, NULL, $4, $4) \
         ON CONFLICT (name, namespace_uuid) DO NOTHING",
    )
    .bind(dataset_uuid)
    .bind(name)
    .bind(namespace_uuid)
    .bind(now.naive_utc())
    .execute(&mut *conn)
    .await?;

    sqlx::query_as::<_, DatasetSymlinkRow>(
        "SELECT * FROM dataset_symlinks WHERE namespace_uuid = $1 AND name = $2",
    )
    .bind(namespace_uuid)
    .bind(name)
    .fetch_one(&mut *conn)
    .await
}

/// Upsert an input dataset from a lineage event.
///
/// Returns `(dataset_uuid, dataset_version_uuid)` so the caller can create
/// IO mappings and update lineage statistics after the job version has been created.
async fn upsert_input_dataset(
    conn: &mut sqlx::PgConnection,
    input: &InputDatasetRef,
    _parent_ns: &NamespaceRow,
    run_uuid: Uuid,
    now: DateTime<Utc>,
    event_time: DateTime<Utc>,
    event_type: &str,
) -> Result<(Uuid, Uuid), sqlx::Error> {
    let (ds_row, dv_row) = upsert_lineage_dataset_common(
        conn,
        &input.namespace,
        &input.name,
        input.facets.as_ref(),
        run_uuid,
        now,
        event_time,
        event_type,
        true, // is_input
    )
    .await?;

    // Insert input mapping (skip for JobEvent which has nil run UUID)
    if run_uuid != Uuid::nil() {
        crate::db::run::update_input_mapping(&mut *conn, run_uuid, dv_row.uuid).await?;
    }

    // Insert input-specific facets (e.g. dataQualityMetrics, dataQualityAssertions)
    if let Some(input_facets) = &input.input_facets {
        let opt_run_uuid = if run_uuid != Uuid::nil() {
            Some(run_uuid)
        } else {
            None
        };
        for (name, facet) in input_facets {
            let wrapped = wrap_facet(name, facet);
            crate::db::facets::insert_dataset_facet(
                &mut *conn,
                now,
                ds_row.uuid,
                dv_row.uuid,
                opt_run_uuid,
                event_time,
                Some(event_type),
                "INPUT",
                name,
                &wrapped,
            )
            .await?;
        }
    }

    Ok((ds_row.uuid, dv_row.uuid))
}

/// Upsert an output dataset from a lineage event.
///
/// Returns `(dataset_uuid, dataset_version_uuid)` so the caller can create
/// IO mappings and update lineage statistics after the job version has been created.
async fn upsert_output_dataset(
    conn: &mut sqlx::PgConnection,
    output: &OutputDatasetRef,
    _parent_ns: &NamespaceRow,
    run_uuid: Uuid,
    now: DateTime<Utc>,
    event_time: DateTime<Utc>,
    event_type: &str,
) -> Result<(Uuid, Uuid), sqlx::Error> {
    let (ds_row, dv_row) = upsert_lineage_dataset_common(
        conn,
        &output.namespace,
        &output.name,
        output.facets.as_ref(),
        run_uuid,
        now,
        event_time,
        event_type,
        false, // is_input
    )
    .await?;

    // Populate column_lineage table from columnLineage facet.
    // Matches Java's OpenLineageDao.upsertColumnLineage() (line 1014-1089).
    //
    // Optimized: 1 bulk lookup for input fields + 1 batch INSERT per output column
    // (Java: 1 + M queries; previous Rust: 3×M×K queries).
    if let Some(facets) = &output.facets {
        if let Some(col_lineage) = facets.get("columnLineage") {
            if let Some(fields_obj) = col_lineage.get("fields").and_then(|f| f.as_object()) {
                // Bulk lookup: all input fields associated with this run (1 query).
                let input_fields_data =
                    crate::db::dataset_field::find_input_fields_data_associated_with_run(
                        &mut *conn, run_uuid,
                    )
                    .await?;

                // Build a lookup map: (namespace, dataset_name, field_name) -> (field_uuid, version_uuid)
                let mut input_lookup: std::collections::HashMap<
                    (String, String, String),
                    (Uuid, Uuid),
                > = std::collections::HashMap::new();
                for ifd in &input_fields_data {
                    let key = (
                        ifd.namespace_name.clone().unwrap_or_default(),
                        ifd.dataset_name.clone().unwrap_or_default(),
                        ifd.field_name.clone(),
                    );
                    if let Some(dv_uuid) = ifd.dataset_version_uuid {
                        input_lookup.insert(key, (ifd.dataset_field_uuid, dv_uuid));
                    }
                }

                for (output_field_name, field_data) in fields_obj {
                    // Type guard: skip entries that aren't well-formed ColumnLineageOutputColumn
                    if !field_data.is_object() || field_data.get("inputFields").is_none() {
                        tracing::warn!(
                            "Skipping malformed columnLineage entry for field '{}': missing inputFields",
                            output_field_name
                        );
                        continue;
                    }

                    let output_field: Option<(Uuid,)> = sqlx::query_as(
                        "SELECT uuid FROM dataset_fields \
                         WHERE dataset_uuid = $1 AND name = $2",
                    )
                    .bind(ds_row.uuid)
                    .bind(output_field_name)
                    .fetch_optional(&mut *conn)
                    .await?;

                    if let Some((output_field_uuid,)) = output_field {
                        if let Some(input_fields) =
                            field_data.get("inputFields").and_then(|f| f.as_array())
                        {
                            let mut batch_rows = Vec::with_capacity(input_fields.len());

                            for input_ref in input_fields {
                                let input_ns = input_ref
                                    .get("namespace")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let input_ds_name =
                                    input_ref.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let input_field_name = input_ref
                                    .get("field")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                // Try bulk lookup first (run-scoped), then fallback query
                                let lookup_key = (
                                    input_ns.to_string(),
                                    input_ds_name.to_string(),
                                    input_field_name.to_string(),
                                );
                                let resolved = if let Some(&(field_uuid, dv_uuid)) =
                                    input_lookup.get(&lookup_key)
                                {
                                    Some((field_uuid, dv_uuid))
                                } else {
                                    // Fallback: use current_version_uuid (best-effort
                                    // for cases where runs_input_mapping wasn't populated)
                                    let fallback: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
                                        "SELECT df.uuid, d.current_version_uuid \
                                             FROM dataset_fields df \
                                             JOIN datasets d ON d.uuid = df.dataset_uuid \
                                             WHERE d.namespace_name = $1 AND d.name = $2 \
                                             AND df.name = $3",
                                    )
                                    .bind(input_ns)
                                    .bind(input_ds_name)
                                    .bind(input_field_name)
                                    .fetch_optional(&mut *conn)
                                    .await?;
                                    fallback.map(|(f, cv)| (f, cv.unwrap_or(Uuid::nil())))
                                };

                                if let Some((input_field_uuid, input_dv_uuid)) = resolved {
                                    let desc = field_data
                                        .get("transformationDescription")
                                        .and_then(|v| v.as_str());
                                    let trans_type = field_data
                                        .get("transformationType")
                                        .and_then(|v| v.as_str());

                                    batch_rows.push(
                                        crate::db::column_lineage::ColumnLineageInput {
                                            output_dv_uuid: dv_row.uuid,
                                            output_field_uuid,
                                            input_dv_uuid,
                                            input_field_uuid,
                                            transformation_description: desc.map(|s| s.to_string()),
                                            transformation_type: trans_type.map(|s| s.to_string()),
                                        },
                                    );
                                }
                            }

                            // Batch upsert all rows for this output column (1 query)
                            crate::db::column_lineage::upsert_batch(&mut *conn, &batch_rows, now)
                                .await?;
                        }
                    } else {
                        tracing::error!(
                            "Cannot produce column lineage for missing output field '{}' in output dataset",
                            output_field_name
                        );
                    }
                }
            }
        }
    }

    // Insert output-specific facets (e.g. outputStatistics)
    if let Some(output_facets) = &output.output_facets {
        let opt_run_uuid = if run_uuid != Uuid::nil() {
            Some(run_uuid)
        } else {
            None
        };
        for (name, facet) in output_facets {
            let wrapped = wrap_facet(name, facet);
            crate::db::facets::insert_dataset_facet(
                &mut *conn,
                now,
                ds_row.uuid,
                dv_row.uuid,
                opt_run_uuid,
                event_time,
                Some(event_type),
                "OUTPUT",
                name,
                &wrapped,
            )
            .await?;
        }
    }

    Ok((ds_row.uuid, dv_row.uuid))
}

/// Wrap a facet value with its name, matching Java's `FacetUtils.asJson()`.
/// Stored format: `{"facetName": facetValue}` — required by all retrieval queries
/// that use `jsonb_each()` to extract facet names from the JSONB column.
fn wrap_facet(name: &str, value: &serde_json::Value) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(name.to_string(), value.clone());
    serde_json::Value::Object(map)
}

/// Slim field representation for the `dataset_versions.fields` JSONB column.
///
/// Matches Java's `Field` format: `{name, type, tags, description}` — not the
/// full `DatasetFieldRow` with 7 fields that would break cross-API compatibility.
#[derive(Serialize)]
struct FieldForJson<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    type_: Option<&'a str>,
    tags: Vec<String>,
    description: Option<&'a str>,
}

/// Categorise a dataset facet by name, matching Java's `DatasetFacet.typeFromName()`.
fn dataset_facet_type(name: &str) -> &'static str {
    match name {
        "dataQualityMetrics" | "dataQualityAssertions" => "INPUT",
        "outputStatistics" => "OUTPUT",
        "documentation"
        | "description"
        | "schema"
        | "dataSource"
        | "lifecycleStateChange"
        | "version"
        | "columnLineage"
        | "ownership"
        | "lineageStatistics" => "DATASET",
        _ => "UNKNOWN",
    }
}

/// Common logic for upserting a dataset referenced in a lineage event.
///
/// When `is_input` is true and the dataset already has a `current_version_uuid`,
/// the existing version is reused instead of creating a new one. This matches
/// Java's `OpenLineageDao.java:948-1002` behavior: input references should not
/// overwrite the current version (which would orphan facets from the output version).
///
/// Returns `(DatasetRow, DatasetVersionRow)`.
async fn upsert_lineage_dataset_common(
    conn: &mut sqlx::PgConnection,
    ds_namespace: &str,
    ds_name: &str,
    facets: Option<&std::collections::HashMap<String, serde_json::Value>>,
    run_uuid: Uuid,
    now: DateTime<Utc>,
    event_time: DateTime<Utc>,
    event_type: &str,
    is_input: bool,
) -> Result<(DatasetRow, DatasetVersionRow), sqlx::Error> {
    // Upsert dataset namespace
    let ds_ns_row = upsert_namespace_inline(&mut *conn, now, ds_namespace).await?;

    // Derive source from dataSource facet or use defaults
    let ds_facet = facets.and_then(|f| f.get("dataSource"));
    let source_name = ds_facet
        .and_then(|d| d.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(DEFAULT_SOURCE_NAME);
    let connection_url = ds_facet
        .and_then(|d| d.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    // Match Java: always hardcode source type to POSTGRESQL regardless of URI.
    // Java's getSourceType() returns "POSTGRESQL" unconditionally.
    let source_type = "POSTGRESQL";

    // Upsert source — use upsert_or_default when no explicit dataSource facet
    // is present (don't overwrite existing type/url). Matches Java's
    // SourceDao.upsertOrDefault() vs SourceDao.upsert() distinction.
    let src_row = if ds_facet.is_some() {
        // Explicit dataSource facet — upsert with provided values
        crate::db::source::upsert(
            &mut *conn,
            Uuid::new_v4(),
            source_type,
            now,
            source_name,
            connection_url,
            None,
        )
        .await?
    } else {
        // No dataSource facet — use default semantics (don't overwrite existing)
        crate::db::source::upsert_or_default(
            &mut *conn,
            Uuid::new_v4(),
            source_type,
            now,
            source_name,
            connection_url,
        )
        .await?
    };

    // Upsert symlink
    let dataset_uuid = Uuid::new_v4();
    let symlink_row =
        upsert_symlink_inline(&mut *conn, dataset_uuid, ds_name, ds_ns_row.uuid, now).await?;

    // Use the UUID from the symlink (may be the existing dataset_uuid if symlink already existed)
    let actual_dataset_uuid = symlink_row.dataset_uuid.unwrap_or(dataset_uuid);

    // Process alternate symlinks from symlinks facet (matches Java OpenLineageDao.java:883-901)
    if let Some(symlinks_facet) = facets.and_then(|f| f.get("symlinks")) {
        if let Some(identifiers) = symlinks_facet.get("identifiers").and_then(|v| v.as_array()) {
            for id in identifiers {
                let sym_name = id.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let sym_ns = id.get("namespace").and_then(|v| v.as_str()).unwrap_or("");
                let sym_type = id.get("type").and_then(|v| v.as_str());
                if !sym_name.is_empty() && !sym_ns.is_empty() {
                    let sym_ns_row = upsert_namespace_inline(&mut *conn, now, sym_ns).await?;
                    upsert_symlink_non_primary_inline(
                        &mut *conn,
                        actual_dataset_uuid,
                        sym_name,
                        sym_ns_row.uuid,
                        sym_type,
                        now,
                    )
                    .await?;
                }
            }
        }
    }

    // Get description from facets
    let description = facets
        .and_then(|f| f.get("documentation"))
        .and_then(|d| d.get("description"))
        .and_then(|d| d.as_str());

    // Check lifecycle state — mark as deleted if DROP (matches Java OpenLineageDao.java:904-923)
    let is_deleted = facets
        .and_then(|f| f.get("lifecycleStateChange"))
        .and_then(|lsc| lsc.get("lifecycleStateChange"))
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("DROP"))
        .unwrap_or(false);

    // Upsert dataset
    let ds_row = crate::db::dataset::upsert(
        &mut *conn,
        actual_dataset_uuid,
        "DB_TABLE",
        now,
        ds_ns_row.uuid,
        ds_namespace,
        src_row.uuid,
        &src_row.name,
        ds_name,
        ds_name,
        description,
        is_deleted,
    )
    .await?;

    // For inputs, reuse the existing version if one exists — matches Java's
    // OpenLineageDao.java:948-951 behavior. This prevents overwriting
    // current_version_uuid and orphaning facets from the output version.
    // Java still inserts/updates facets even when reusing the version, so we
    // must persist dataset facets before returning early.
    if is_input {
        if let Some(existing_version_uuid) = ds_row.current_version_uuid {
            if let Some(existing_dv) =
                crate::db::dataset_version::find_by_uuid_exec(&mut *conn, existing_version_uuid)
                    .await?
            {
                // Insert dataset facets even when reusing the existing version
                if let Some(facet_map) = facets {
                    let opt_run_uuid = if run_uuid != Uuid::nil() {
                        Some(run_uuid)
                    } else {
                        None
                    };
                    for (name, facet) in facet_map {
                        let wrapped = wrap_facet(name, facet);
                        crate::db::facets::insert_dataset_facet(
                            &mut *conn,
                            now,
                            ds_row.uuid,
                            existing_dv.uuid,
                            opt_run_uuid,
                            event_time,
                            Some(event_type),
                            dataset_facet_type(name),
                            name,
                            &wrapped,
                        )
                        .await?;
                    }
                }
                return Ok((ds_row, existing_dv));
            }
        }
    }

    // Process schema fields
    let fields = facets
        .and_then(|f| f.get("schema"))
        .and_then(|s| s.get("fields"))
        .and_then(|f| f.as_array());

    let mut field_rows: Vec<DatasetFieldRow> = Vec::new();
    if let Some(schema_fields) = fields {
        for field in schema_fields {
            let field_name = field.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let field_type = field.get("type").and_then(|v| v.as_str());
            let field_desc = field.get("description").and_then(|v| v.as_str());
            if !field_name.is_empty() {
                let field_row = crate::db::dataset_field::upsert(
                    &mut *conn,
                    Uuid::new_v4(),
                    now,
                    field_name,
                    field_type,
                    field_desc,
                    ds_row.uuid,
                )
                .await?;
                field_rows.push(field_row);
            }
        }
    }

    // Compute schema version UUID matching Java's Utils.newDatasetSchemaVersionFor().
    // MD5(namespace:datasetName:field1Name:field1Type:field2Name:field2Type:...)
    // Fields sorted alphabetically by (name, type), null types skipped.
    let schema_version_uuid = if !field_rows.is_empty() {
        let mut sorted_fields: Vec<(&str, Option<&str>)> = field_rows
            .iter()
            .map(|f| (f.name.as_str(), f.type_.as_deref()))
            .collect();
        sorted_fields.sort();
        let mut sv_parts: Vec<&str> = vec![ds_namespace, ds_name];
        for (name, type_) in &sorted_fields {
            sv_parts.push(name);
            if let Some(t) = type_ {
                sv_parts.push(t);
            }
        }
        let sv_string = sv_parts.join(":");
        let sv_uuid = uuid_name_from_bytes(sv_string.as_bytes());

        // Upsert schema version row
        let sv_inserted = crate::db::dataset_version::upsert_schema_version(
            &mut *conn,
            sv_uuid,
            ds_row.uuid,
            now,
        )
        .await?;

        // If newly inserted, add field mappings
        if sv_inserted.is_some() {
            for field_row in &field_rows {
                sqlx::query(
                    "INSERT INTO dataset_schema_versions_field_mapping \
                     (dataset_schema_version_uuid, dataset_field_uuid) \
                     VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(sv_uuid)
                .bind(field_row.uuid)
                .execute(&mut *conn)
                .await?;
            }
        }

        Some(sv_uuid)
    } else {
        None
    };

    // Compute dataset version UUID matching Java's Utils.newDatasetVersionFor().
    // Java: MD5(namespace:sourceName:datasetName:physicalName:schemaLocation:
    //        field1Name:field1Type:field1Desc:...:lifecycleState:runId)
    // Uses VERSION_JOINER (colon-delimited, skipNulls).
    let lifecycle_state = facets
        .and_then(|f| f.get("lifecycleStateChange"))
        .and_then(|lsc| lsc.get("lifecycleStateChange"))
        .and_then(|v| v.as_str());

    let mut version_parts: Vec<&str> = vec![
        ds_namespace,
        &src_row.name, // sourceName
        ds_name,       // datasetName
        ds_name,       // physicalName (same as datasetName in OL path)
    ];
    // schemaLocation is always null → skipNulls omits it
    for f in &field_rows {
        version_parts.push(&f.name);
        if let Some(ref t) = f.type_ {
            version_parts.push(t);
        }
        if let Some(ref d) = f.description {
            version_parts.push(d);
        }
    }
    if let Some(lc) = lifecycle_state {
        version_parts.push(lc);
    }
    let run_uuid_str = run_uuid.to_string();
    version_parts.push(&run_uuid_str);
    let version_string = version_parts.join(":");
    let version_uuid = uuid_name_from_bytes(version_string.as_bytes());

    let fields_json: Option<serde_json::Value> = if !field_rows.is_empty() {
        let slim_fields: Vec<FieldForJson<'_>> = field_rows
            .iter()
            .map(|f| FieldForJson {
                name: &f.name,
                type_: f.type_.as_deref(),
                tags: Vec::new(),
                description: f.description.as_deref(),
            })
            .collect();
        Some(serde_json::to_value(&slim_fields).unwrap_or_default())
    } else {
        None
    };

    // Upsert dataset version — Java sets run_uuid to NULL for inputs
    // (only output datasets are linked to the run that produced them).
    // For dataset events (no run context), run_uuid is Uuid::nil() — always
    // pass None in that case to avoid storing a FK to a nonexistent run.
    let dv_run_uuid = if is_input || run_uuid == Uuid::nil() {
        None
    } else {
        Some(run_uuid)
    };
    let dv_row = crate::db::dataset_version::upsert(
        &mut *conn,
        Uuid::new_v4(),
        now,
        ds_row.uuid,
        version_uuid,
        schema_version_uuid,
        dv_run_uuid,
        fields_json,
        ds_namespace,
        ds_name,
        Some(lifecycle_state.unwrap_or("")),
    )
    .await?;

    // Update field mappings
    for field_row in &field_rows {
        sqlx::query(
            "INSERT INTO dataset_versions_field_mapping \
             (dataset_version_uuid, dataset_field_uuid) \
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(dv_row.uuid)
        .bind(field_row.uuid)
        .execute(&mut *conn)
        .await?;
    }

    // Update dataset current version — skip for inputs that already had a version.
    // Matches Java's OpenLineageDao.java:999: only updates if getCurrentVersionUuid().isEmpty().
    if !is_input || ds_row.current_version_uuid.is_none() {
        crate::db::dataset::update_version(&mut *conn, ds_row.uuid, dv_row.uuid, now).await?;
    }

    // Insert dataset facets (pass NULL run_uuid when no real run exists)
    if let Some(facet_map) = facets {
        let opt_run_uuid = if run_uuid != Uuid::nil() {
            Some(run_uuid)
        } else {
            None
        };
        for (name, facet) in facet_map {
            let wrapped = wrap_facet(name, facet);
            crate::db::facets::insert_dataset_facet(
                &mut *conn,
                now,
                ds_row.uuid,
                dv_row.uuid,
                opt_run_uuid,
                event_time,
                Some(event_type),
                dataset_facet_type(name),
                name,
                &wrapped,
            )
            .await?;
        }
    }

    Ok((ds_row, dv_row))
}

/// Inline IO mapping upsert for use inside transactions.
///
/// The caller provides the `job_version_uuid` directly — this must be called
/// **after** the job version row has been created so the FK is satisfied.
async fn upsert_io_mapping_inline(
    conn: &mut sqlx::PgConnection,
    job_version_uuid: Uuid,
    dataset_uuid: Uuid,
    job_uuid: Uuid,
    io_type: &str,
    now: DateTime<Utc>,
    symlink_target_uuid: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    // Mark previous mappings as not current (matches Java's JobVersionDao)
    sqlx::query(
        "UPDATE job_versions_io_mapping \
         SET is_current_job_version = false \
         WHERE (job_uuid = $1 OR job_symlink_target_uuid = $1 \
                OR ($4::uuid IS NOT NULL AND (job_uuid = $4 OR job_symlink_target_uuid = $4))) \
         AND job_version_uuid != $2 AND io_type = $3 \
         AND is_current_job_version = true",
    )
    .bind(job_uuid)
    .bind(job_version_uuid)
    .bind(io_type)
    .bind(symlink_target_uuid)
    .execute(&mut *conn)
    .await?;

    // Insert new mapping (re-activate stale mappings on conflict)
    sqlx::query(
        "INSERT INTO job_versions_io_mapping (\
            job_version_uuid, dataset_uuid, io_type, job_uuid, \
            job_symlink_target_uuid, is_current_job_version, made_current_at\
        ) VALUES ($1, $2, $3, $4, $5, true, $6) \
        ON CONFLICT (job_version_uuid, dataset_uuid, io_type, job_uuid) \
        DO UPDATE SET is_current_job_version = TRUE, \
                      made_current_at = EXCLUDED.made_current_at",
    )
    .bind(job_version_uuid)
    .bind(dataset_uuid)
    .bind(io_type)
    .bind(job_uuid)
    .bind(symlink_target_uuid)
    .bind(now.naive_utc())
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Find input datasets accumulated across all events for a given run.
///
/// Matches Java's `DatasetVersionDao.findInputDatasetVersionsFor(runUuid)`.
/// Returns `(dataset_uuid, dataset_version_uuid)` pairs from `runs_input_mapping`.
async fn find_accumulated_input_datasets(
    conn: &mut sqlx::PgConnection,
    run_uuid: Uuid,
) -> Result<Vec<(Uuid, Uuid)>, sqlx::Error> {
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT DISTINCT dv.dataset_uuid, dv.uuid \
         FROM runs_input_mapping rim \
         INNER JOIN dataset_versions dv ON dv.uuid = rim.dataset_version_uuid \
         WHERE rim.run_uuid = $1",
    )
    .bind(run_uuid)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows)
}

/// Find output datasets accumulated across all events for a given run.
///
/// Matches Java's `DatasetVersionDao.findOutputDatasetVersionsFor(runId)`.
/// Returns `(dataset_uuid, dataset_version_uuid)` pairs from `dataset_versions`.
async fn find_accumulated_output_datasets(
    conn: &mut sqlx::PgConnection,
    run_uuid: Uuid,
) -> Result<Vec<(Uuid, Uuid)>, sqlx::Error> {
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT DISTINCT dataset_uuid, uuid \
         FROM dataset_versions \
         WHERE run_uuid = $1",
    )
    .bind(run_uuid)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows)
}

/// Compute SHA256 hex digest of a string.
fn sha256_checksum(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(s.as_bytes());
    format!("{:x}", hash)
}

/// Compute SHA256 checksum for run args using Java's KV-joined format.
///
/// Java's `Utils.checksumFor()` uses `Hashing.sha256().hashString(KV_JOINER.join(kvMap))`
/// where KV_JOINER is `Joiner.on("#").withKeyValueSeparator("=")`.
/// This produces `key1=value1#key2=value2` (BTreeMap gives sorted keys).
fn kv_checksum(args_map: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut sorted: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
    for (k, v) in args_map {
        sorted.insert(k.as_str(), v.as_str().unwrap_or(""));
    }
    let kv_string: String = sorted
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("#");
    sha256_checksum(&kv_string)
}

/// Inline upsert for `run_args` table (INSERT ... ON CONFLICT(checksum) DO NOTHING + SELECT).
/// Inlined because `run_args::upsert` takes `&PgPool` but we need to work within a transaction.
async fn upsert_run_args_inline(
    conn: &mut sqlx::PgConnection,
    uuid: Uuid,
    now: DateTime<Utc>,
    args: &str,
    checksum: &str,
) -> Result<crate::models::db::RunArgsRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO run_args (uuid, created_at, args, checksum) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT(checksum) DO NOTHING",
    )
    .bind(uuid)
    .bind(now.naive_utc())
    .bind(args)
    .bind(checksum)
    .execute(&mut *conn)
    .await?;

    sqlx::query_as::<_, crate::models::db::RunArgsRow>("SELECT * FROM run_args WHERE checksum = $1")
        .bind(checksum)
        .fetch_one(&mut *conn)
        .await
}

/// Upsert a non-primary dataset symlink (matches Java DatasetSymlinkDao:883-901).
async fn upsert_symlink_non_primary_inline(
    conn: &mut sqlx::PgConnection,
    dataset_uuid: Uuid,
    name: &str,
    namespace_uuid: Uuid,
    type_: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO dataset_symlinks \
         (dataset_uuid, name, namespace_uuid, is_primary, type, created_at, updated_at) \
         VALUES ($1, $2, $3, false, $4, $5, $5) \
         ON CONFLICT (name, namespace_uuid) DO UPDATE SET \
         updated_at = EXCLUDED.updated_at, type = COALESCE(EXCLUDED.type, dataset_symlinks.type)",
    )
    .bind(dataset_uuid)
    .bind(name)
    .bind(namespace_uuid)
    .bind(type_)
    .bind(now.naive_utc())
    .execute(&mut *conn)
    .await?;
    Ok(())
}
