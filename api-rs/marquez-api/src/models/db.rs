// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! Database row types.
//!
//! Each struct maps 1:1 to a PostgreSQL table (or view) and derives
//! `sqlx::FromRow` so it can be used with `query_as`. **Newtypes are not
//! used here** -- all fields are plain `String`, `Uuid`, `Option<T>`, etc.
//! to avoid transparent-type issues with sqlx.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core entity rows
// ---------------------------------------------------------------------------

/// Row from the `namespaces` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct NamespaceRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub name: String,
    pub description: Option<String>,
    pub current_owner_name: Option<String>,
    pub is_hidden: Option<bool>,
}

/// Row from the `owners` table.
///
/// Note: `owners.created_at` is `TIMESTAMP` (not `TIMESTAMPTZ`), so we use
/// `NaiveDateTime`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct OwnerRow {
    pub uuid: Uuid,
    pub created_at: NaiveDateTime,
    pub name: String,
}

/// Row from the `sources` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct SourceRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub name: String,
    pub connection_url: String,
    pub description: Option<String>,
}

/// Row from the `tags` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct TagRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub name: String,
    pub description: Option<String>,
}

/// Row from the `datasets` table.
///
/// Columns accumulated across V1, V6/V11 (last_modified_at), V25
/// (namespace_name, source_name), V41 (is_deleted), V46 (is_hidden).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub source_uuid: Option<Uuid>,
    pub source_name: Option<String>,
    pub name: String,
    pub physical_name: String,
    pub last_modified_at: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub is_deleted: Option<bool>,
    pub is_hidden: Option<bool>,
}

/// Row from the `jobs` table.
///
/// Columns accumulated across V1, V21 (namespace_name), V24
/// (current_job_context_uuid, current_location, current_inputs),
/// V42 (symlink_target_uuid), V43 (parent_job_uuid), V46 (is_hidden),
/// V61 (simple_name, name-as-fqn, aliases), V74 (current_run_uuid).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct JobRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub name: String,
    pub simple_name: Option<String>,
    pub parent_job_uuid: Option<Uuid>,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub current_job_context_uuid: Option<Uuid>,
    pub current_location: Option<String>,
    pub current_inputs: Option<serde_json::Value>,
    pub symlink_target_uuid: Option<Uuid>,
    pub is_hidden: Option<bool>,
    pub current_run_uuid: Option<Uuid>,
    pub aliases: Option<Vec<String>>,
}

/// Row from the `runs` table.
///
/// Columns accumulated across V1, V13 (start/end_run_state_uuid),
/// V19 (external_id), V23 (namespace_name, job_name, location,
/// transitioned_at, started_at, ended_at), V27 (job_context_uuid),
/// V43 (parent_run_uuid), V44 (job_uuid).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub job_uuid: Option<Uuid>,
    pub job_version_uuid: Option<Uuid>,
    pub parent_run_uuid: Option<Uuid>,
    pub run_args_uuid: Option<Uuid>,
    pub nominal_start_time: Option<DateTime<Utc>>,
    pub nominal_end_time: Option<DateTime<Utc>>,
    pub current_run_state: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub start_run_state_uuid: Option<Uuid>,
    pub ended_at: Option<DateTime<Utc>>,
    pub end_run_state_uuid: Option<Uuid>,
    pub job_name: Option<String>,
    pub namespace_name: Option<String>,
    pub external_id: Option<String>,
    pub location: Option<String>,
    pub transitioned_at: Option<DateTime<Utc>>,
    pub job_context_uuid: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Supporting rows
// ---------------------------------------------------------------------------

/// Row from the `run_states` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunStateRow {
    pub uuid: Uuid,
    pub transitioned_at: DateTime<Utc>,
    pub run_uuid: Option<Uuid>,
    pub state: String,
}

/// Row from the `run_args` table.
///
/// Note: `run_args.created_at` is `TIMESTAMP` (not converted in V73).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunArgsRow {
    pub uuid: Uuid,
    pub created_at: NaiveDateTime,
    pub args: String,
    pub checksum: String,
}

/// Row from the `dataset_versions` table.
///
/// Columns accumulated across V1, V29 (fields jsonb, namespace_name,
/// dataset_name), V41 (lifecycle_state), V69.3 (dataset_schema_version_uuid).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetVersionRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub dataset_uuid: Option<Uuid>,
    pub version: Uuid,
    pub run_uuid: Option<Uuid>,
    pub fields: Option<serde_json::Value>,
    pub namespace_name: Option<String>,
    pub dataset_name: Option<String>,
    pub lifecycle_state: Option<String>,
    pub dataset_schema_version_uuid: Option<Uuid>,
}

/// Row from the `dataset_fields` table.
///
/// Created in V4, type made nullable in V34, type widened in V37,
/// timestamps converted in V73.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetFieldRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub dataset_uuid: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
}

/// Dataset field row with aggregated tags.
///
/// Matches Java's `DatasetField` from `DatasetFieldDao.findByDatasetSchemaVersion()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetFieldWithTagsRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub dataset_uuid: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

/// Row from the `dataset_schema_versions` table (V69.1).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetSchemaVersionRow {
    pub uuid: Uuid,
    pub dataset_uuid: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Row from the `dataset_symlinks` table (V48).
///
/// Note: timestamps are `TIMESTAMP` (not converted in V73).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetSymlinkRow {
    pub dataset_uuid: Option<Uuid>,
    pub name: String,
    pub namespace_uuid: Option<Uuid>,
    #[sqlx(rename = "type")]
    pub type_: Option<String>,
    pub is_primary: Option<bool>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Row from the `job_versions` table.
///
/// Columns accumulated across V1, V2.1 (job_context_uuid), V3 (location
/// nullable), V22 (namespace_uuid, namespace_name, job_name),
/// V60 (job_context_uuid nullable). V73 timestamps to TIMESTAMPTZ.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct JobVersionRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub job_uuid: Option<Uuid>,
    pub version: Uuid,
    pub location: Option<String>,
    pub latest_run_uuid: Option<Uuid>,
    pub job_context_uuid: Option<Uuid>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub job_name: Option<String>,
}

/// Row from the `job_versions_io_mapping` table.
///
/// Columns accumulated across V1, V67.1 (job_uuid, job_symlink_target_uuid,
/// is_current_job_version, made_current_at).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct JobVersionIoMappingRow {
    pub job_version_uuid: Option<Uuid>,
    pub dataset_uuid: Option<Uuid>,
    pub io_type: String,
    pub job_uuid: Option<Uuid>,
    pub job_symlink_target_uuid: Option<Uuid>,
    pub is_current_job_version: Option<bool>,
    pub made_current_at: Option<NaiveDateTime>,
}

/// Row from the `column_lineage` table (V49).
///
/// Note: timestamps are `TIMESTAMP` (not converted in V73).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ColumnLineageRow {
    pub output_dataset_version_uuid: Option<Uuid>,
    pub output_dataset_field_uuid: Option<Uuid>,
    pub input_dataset_version_uuid: Option<Uuid>,
    pub input_dataset_field_uuid: Option<Uuid>,
    pub transformation_description: Option<String>,
    pub transformation_type: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Row from the `lineage_events` table (V17.2).
///
/// Note: `event_time` was created as `TIMESTAMP WITH TIME ZONE`.
/// `run_id` was dropped in V36. `_event_type` added in V66.2.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct LineageEventRow {
    pub event_time: Option<DateTime<Utc>>,
    pub event: Option<serde_json::Value>,
    pub event_type: Option<String>,
    pub job_name: Option<String>,
    pub job_namespace: Option<String>,
    pub producer: Option<String>,
    #[sqlx(rename = "_event_type")]
    pub _event_type: Option<String>,
}

/// Row from the `dataset_facets` table (V55.1).
///
/// `lineage_event_type` made nullable in V65.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetFacetRow {
    pub created_at: DateTime<Utc>,
    pub dataset_uuid: Option<Uuid>,
    pub dataset_version_uuid: Option<Uuid>,
    pub run_uuid: Option<Uuid>,
    pub lineage_event_time: DateTime<Utc>,
    pub lineage_event_type: Option<String>,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub name: String,
    pub facet: serde_json::Value,
}

/// Row from the `job_facets` table (V55.2).
///
/// `lineage_event_type` made nullable in V66.1.
/// `job_version_uuid` added in V66.1.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct JobFacetRow {
    pub created_at: DateTime<Utc>,
    pub job_uuid: Option<Uuid>,
    pub run_uuid: Option<Uuid>,
    pub lineage_event_time: DateTime<Utc>,
    pub lineage_event_type: Option<String>,
    pub name: String,
    pub facet: serde_json::Value,
    pub job_version_uuid: Option<Uuid>,
}

/// Row from the `run_facets` table (V55.3).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunFacetRow {
    pub created_at: DateTime<Utc>,
    pub run_uuid: Option<Uuid>,
    pub lineage_event_time: DateTime<Utc>,
    pub lineage_event_type: String,
    pub name: String,
    pub facet: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Extended (composed) rows -- NOT FromRow, used for in-memory assembly
// ---------------------------------------------------------------------------

/// Extended run row composed from a `RunRow` plus denormalized fields
/// that come from JOINs.
#[derive(Debug, Clone, Serialize)]
pub struct ExtendedRunRow {
    pub run: RunRow,
    pub args: Option<String>,
    pub external_id: Option<String>,
    pub namespace_name: String,
    pub job_name: String,
}

/// Extended dataset version row composed from a `DatasetVersionRow` plus
/// denormalized dataset identity.
#[derive(Debug, Clone, Serialize)]
pub struct ExtendedDatasetVersionRow {
    pub row: DatasetVersionRow,
    pub namespace_name: String,
    pub dataset_name: String,
}

/// Extended job version row composed from a `JobVersionRow` plus
/// denormalized job identity.
#[derive(Debug, Clone, Serialize)]
pub struct ExtendedJobVersionRow {
    pub row: JobVersionRow,
    pub namespace_name: String,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Search / Stats / Lineage rows
// ---------------------------------------------------------------------------

/// Aggregated column lineage node row from the recursive CTE.
///
/// Each row represents one output field with all its input fields
/// aggregated via `JSONB_AGG`. This eliminates N+1 queries.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ColumnLineageNodeRow {
    pub namespace_name: String,
    pub dataset_name: String,
    pub field_name: String,
    #[sqlx(rename = "field_type")]
    pub field_type: Option<String>,
    /// JSONB_AGG of arrays: `[[namespace, dataset, version_uuid, field, desc, type], ...]`
    pub input_fields: Option<serde_json::Value>,
    pub dataset_version_uuid: Option<Uuid>,
}

/// Row for search results (UNION of datasets_view and jobs_view).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct SearchResultRow {
    #[sqlx(rename = "type")]
    pub type_: String,
    pub name: String,
    pub updated_at: DateTime<Utc>,
    pub namespace_name: String,
}

/// Row for lineage event metrics (hourly or daily aggregation).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageMetricRow {
    #[serde(with = "super::iso8601")]
    pub start_interval: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub end_interval: DateTime<Utc>,
    pub fail: i64,
    pub start: i64,
    pub complete: i64,
    pub abort: i64,
}

/// Row for interval metrics (jobs, datasets, sources counts).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalMetricRow {
    #[serde(with = "super::iso8601")]
    pub start_interval: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub end_interval: DateTime<Utc>,
    pub count: i64,
}

/// Row for upstream run lineage results (enriched with run metadata and input datasets).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UpstreamRunRow {
    pub r_uuid: Uuid,
    pub dataset_uuid: Option<Uuid>,
    pub dataset_version_uuid: Option<Uuid>,
    pub dataset_namespace: Option<String>,
    pub dataset_name: Option<String>,
    pub u_r_uuid: Option<Uuid>,
    pub depth: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub state: Option<String>,
    pub job_uuid: Option<Uuid>,
    pub job_version_uuid: Option<Uuid>,
    pub job_namespace: Option<String>,
    pub job_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Enriched rows for gap queries (Priority 1 parity)
// ---------------------------------------------------------------------------

/// Enriched job row from the lineage CTE (`getLineage()`).
///
/// Matches Java's `JobData`: job fields plus input/output dataset UUID arrays.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct JobDataRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub name: String,
    pub simple_name: Option<String>,
    pub parent_job_uuid: Option<Uuid>,
    pub parent_job_name: Option<String>,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub current_job_context_uuid: Option<Uuid>,
    pub current_location: Option<String>,
    pub current_inputs: Option<serde_json::Value>,
    pub symlink_target_uuid: Option<Uuid>,
    pub current_run_uuid: Option<Uuid>,
    pub aliases: Option<Vec<String>>,
    pub input_uuids: Option<Vec<Uuid>>,
    pub output_uuids: Option<Vec<Uuid>>,
}

/// Enriched dataset row from `getDatasetData()`.
///
/// Matches Java's `DatasetData`: dataset fields plus version fields/lifecycle_state.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetDataRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub source_uuid: Option<Uuid>,
    pub source_name: Option<String>,
    pub name: String,
    pub physical_name: String,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub last_modified_at: Option<DateTime<Utc>>,
    pub fields: Option<serde_json::Value>,
    pub lifecycle_state: Option<String>,
}

/// Run row with facets, args, and version info (`getCurrentRunsWithFacets()`).
///
/// Matches Java's enriched `Run` from `LineageDao.getCurrentRunsWithFacets()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RunWithFacetsRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub job_uuid: Option<Uuid>,
    pub job_version_uuid: Option<Uuid>,
    pub parent_run_uuid: Option<Uuid>,
    pub run_args_uuid: Option<Uuid>,
    pub nominal_start_time: Option<DateTime<Utc>>,
    pub nominal_end_time: Option<DateTime<Utc>>,
    pub current_run_state: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub start_run_state_uuid: Option<Uuid>,
    pub ended_at: Option<DateTime<Utc>>,
    pub end_run_state_uuid: Option<Uuid>,
    pub job_name: Option<String>,
    pub namespace_name: Option<String>,
    pub external_id: Option<String>,
    pub location: Option<String>,
    pub transitioned_at: Option<DateTime<Utc>>,
    pub job_context_uuid: Option<Uuid>,
    pub args: Option<String>,
    pub facets: Option<serde_json::Value>,
    pub job_version: Option<Uuid>,
    pub input_versions: Option<serde_json::Value>,
    pub output_versions: Option<serde_json::Value>,
}

/// Enriched run with full facets from `BASE_FIND_RUN_SQL` + dataset_facets.
///
/// Matches Java's `Run` from `RunDao.findRunByUuid()` / `findByLatestJob()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ExtendedRunWithFacetsRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub job_uuid: Option<Uuid>,
    pub job_version_uuid: Option<Uuid>,
    pub parent_run_uuid: Option<Uuid>,
    pub run_args_uuid: Option<Uuid>,
    pub nominal_start_time: Option<DateTime<Utc>>,
    pub nominal_end_time: Option<DateTime<Utc>>,
    pub current_run_state: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub start_run_state_uuid: Option<Uuid>,
    pub ended_at: Option<DateTime<Utc>>,
    pub end_run_state_uuid: Option<Uuid>,
    pub job_name: Option<String>,
    pub namespace_name: Option<String>,
    pub external_id: Option<String>,
    pub location: Option<String>,
    pub transitioned_at: Option<DateTime<Utc>>,
    pub job_context_uuid: Option<Uuid>,
    pub args: Option<String>,
    pub facets: Option<serde_json::Value>,
    pub job_version: Option<Uuid>,
    pub input_versions: Option<serde_json::Value>,
    pub output_versions: Option<serde_json::Value>,
    pub dataset_facets: Option<serde_json::Value>,
}

/// Job with facets and tags from `JobDao.findJobByName()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct JobWithFacetsRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub name: String,
    pub simple_name: Option<String>,
    pub parent_job_uuid: Option<Uuid>,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub current_job_context_uuid: Option<Uuid>,
    pub current_location: Option<String>,
    pub current_inputs: Option<serde_json::Value>,
    pub symlink_target_uuid: Option<Uuid>,
    pub current_run_uuid: Option<Uuid>,
    pub aliases: Option<Vec<String>>,
    pub facets: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

/// Dataset with facets, tags, fields, and lifecycle state.
///
/// Matches Java's `Dataset` from `DatasetDao.findDatasetByName()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct DatasetWithFacetsRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub source_uuid: Option<Uuid>,
    pub source_name: Option<String>,
    pub name: String,
    pub physical_name: String,
    pub last_modified_at: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub is_deleted: Option<bool>,
    pub fields: Option<serde_json::Value>,
    pub lifecycle_state: Option<String>,
    pub schema_location: Option<String>,
    pub tags: Option<Vec<String>>,
    pub facets: Option<serde_json::Value>,
}

/// Enriched dataset version row from `DatasetVersionDao.findBy(UUID)`.
///
/// Matches Java's enriched `DatasetVersion` with tags, facets, schema info.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct EnrichedDatasetVersionRow {
    #[sqlx(rename = "type")]
    pub type_: Option<String>,
    pub name: Option<String>,
    pub physical_name: Option<String>,
    pub namespace_name: Option<String>,
    pub source_name: Option<String>,
    pub description: Option<String>,
    pub lifecycle_state: Option<String>,
    pub created_at: DateTime<Utc>,
    pub current_version_uuid: Uuid,
    pub version: Uuid,
    pub dataset_schema_version_uuid: Option<Uuid>,
    pub fields: Option<serde_json::Value>,
    #[sqlx(rename = "createdbyrunuuid")]
    pub created_by_run_uuid: Option<Uuid>,
    pub schema_location: Option<String>,
    pub tags: Option<Vec<String>>,
    pub facets: Option<serde_json::Value>,
}

/// Input field data associated with a run.
///
/// Matches Java's `InputFieldData` from `DatasetFieldDao.findInputFieldsDataAssociatedWithRun()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct InputFieldDataRow {
    pub namespace_name: Option<String>,
    pub dataset_name: Option<String>,
    pub field_name: String,
    pub dataset_uuid: Option<Uuid>,
    pub dataset_version_uuid: Option<Uuid>,
    pub dataset_field_uuid: Uuid,
}

// ---------------------------------------------------------------------------
// Enriched job version row (gap query: JobVersionDao enriched)
// ---------------------------------------------------------------------------

/// Enriched job version row from the `BASE_SELECT_ON_JOB_VERSIONS` CTE.
///
/// Matches Java's `JobVersion` from `JobVersionDao.findJobVersion()` /
/// `findAllJobVersions()`. Includes I/O datasets, latest run details,
/// facets, and input/output versions.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct EnrichedJobVersionRow {
    // -- job_version fields --
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub job_uuid: Option<Uuid>,
    pub version: Uuid,
    pub location: Option<String>,
    pub latest_run_uuid: Option<Uuid>,
    pub namespace_uuid: Option<Uuid>,
    pub namespace_name: Option<String>,
    pub job_name: Option<String>,
    // -- I/O datasets (JSON aggregates) --
    pub input_datasets: Option<serde_json::Value>,
    pub output_datasets: Option<serde_json::Value>,
    // -- latest run fields (prefixed with run_) --
    pub run_uuid: Option<Uuid>,
    pub run_created_at: Option<DateTime<Utc>>,
    pub run_updated_at: Option<DateTime<Utc>>,
    pub run_nominal_start_time: Option<DateTime<Utc>>,
    pub run_nominal_end_time: Option<DateTime<Utc>>,
    pub run_current_run_state: Option<String>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub run_ended_at: Option<DateTime<Utc>>,
    pub run_namespace_name: Option<String>,
    pub run_job_name: Option<String>,
    pub run_job_version: Option<Uuid>,
    pub run_location: Option<String>,
    pub run_args: Option<String>,
    pub run_facets: Option<serde_json::Value>,
    pub run_input_versions: Option<serde_json::Value>,
    pub run_output_versions: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Namespace ownership row (gap query: NamespaceDao ownership)
// ---------------------------------------------------------------------------

/// Row from the `namespace_ownerships` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct NamespaceOwnershipRow {
    pub uuid: Uuid,
    pub started_at: NaiveDateTime,
    pub ended_at: Option<NaiveDateTime>,
    pub namespace_uuid: Option<Uuid>,
    pub owner_uuid: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Search result rows (gap query: FullSearchDao)
// ---------------------------------------------------------------------------

/// Dataset row for full search results.
///
/// Matches Java's `SimpleDataset` from `FullSearchDao.searchDatasets()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct SimpleDatasetRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_name: Option<String>,
    pub name: String,
    pub physical_name: String,
    pub source_name: Option<String>,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub last_modified_at: Option<DateTime<Utc>>,
    pub is_deleted: Option<bool>,
    pub lifecycle_state: Option<String>,
    pub fields: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub facets: Option<serde_json::Value>,
    pub is_current_version: Option<bool>,
}

/// Job row for full search results.
///
/// Matches Java's `SimpleJob` from `FullSearchDao.searchJobs()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct SimpleJobRow {
    pub uuid: Uuid,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub namespace_name: Option<String>,
    pub name: String,
    pub simple_name: Option<String>,
    pub parent_job_name: Option<String>,
    pub parent_job_uuid: Option<Uuid>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub current_version_uuid: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub labels: Option<Vec<String>>,
    pub facets: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Dataset field UUID rows (gap query: DatasetFieldDao)
// ---------------------------------------------------------------------------

/// Row for field UUID with timestamp, from `DatasetFieldDao.findFieldsUuidsByJobVersion()`.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct FieldUuidTimestampRow {
    pub uuid: Uuid,
    pub created_at: DateTime<Utc>,
}
