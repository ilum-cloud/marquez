// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::common::{
    DatasetId, DatasetType, DatasetVersionId, Field, JobId, JobType, JobVersionId, NodeType, RunId,
    RunState,
};

fn default_empty_object() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

fn default_empty_array() -> serde_json::Value {
    serde_json::Value::Array(Default::default())
}

fn default_null_value() -> serde_json::Value {
    serde_json::Value::Null
}

// ---------------------------------------------------------------------------
// Core API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub name: String,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    pub owner_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    pub connection_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub id: DatasetId,
    #[serde(rename = "type")]
    pub type_: DatasetType,
    pub name: String,
    pub physical_name: String,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    pub namespace: String,
    pub source_name: String,
    pub fields: Vec<Field>,
    pub tags: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::iso8601::serialize_option",
        deserialize_with = "super::iso8601::deserialize_option"
    )]
    pub last_modified_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_lifecycle_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<Uuid>,
    #[serde(default = "default_empty_object")]
    pub facets: serde_json::Value,
    #[serde(rename = "deleted")]
    pub is_deleted: bool,
    #[serde(default = "default_null_value")]
    pub column_lineage: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: JobId,
    #[serde(rename = "type")]
    pub type_: JobType,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simple_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_job_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_job_uuid: Option<Uuid>,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    pub namespace: String,
    pub inputs: Vec<DatasetId>,
    pub outputs: Vec<DatasetId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub latest_run: Option<Box<Run>>,
    #[serde(default = "default_empty_object")]
    pub facets: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<Uuid>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub latest_runs: Vec<Run>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: RunId,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::iso8601::serialize_option",
        deserialize_with = "super::iso8601::deserialize_option"
    )]
    pub nominal_start_time: Option<DateTime<Utc>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::iso8601::serialize_option",
        deserialize_with = "super::iso8601::deserialize_option"
    )]
    pub nominal_end_time: Option<DateTime<Utc>>,
    pub state: RunState,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::iso8601::serialize_option",
        deserialize_with = "super::iso8601::deserialize_option"
    )]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::iso8601::serialize_option",
        deserialize_with = "super::iso8601::deserialize_option"
    )]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(default = "default_empty_object")]
    pub args: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_version: Option<JobVersionLink>,
    #[serde(default = "default_empty_array")]
    pub input_dataset_versions: serde_json::Value,
    #[serde(default = "default_empty_array")]
    pub output_dataset_versions: serde_json::Value,
    #[serde(default = "default_empty_object")]
    pub facets: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobVersionLink {
    pub name: String,
    pub namespace: String,
    pub version: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetVersion {
    pub id: DatasetVersionId,
    #[serde(rename = "type")]
    pub type_: DatasetType,
    pub name: String,
    pub physical_name: String,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    pub version: Uuid,
    pub namespace: String,
    pub source_name: String,
    pub fields: Vec<Field>,
    pub tags: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::iso8601::serialize_option",
        deserialize_with = "super::iso8601::deserialize_option"
    )]
    pub last_modified_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_schema_version: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_state: Option<String>,
    #[serde(rename = "createdByRun", skip_serializing_if = "Option::is_none")]
    pub run: Option<Box<Run>>,
    #[serde(default = "default_empty_object")]
    pub facets: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobVersion {
    pub id: JobVersionId,
    #[serde(rename = "type")]
    pub type_: JobType,
    pub name: String,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    pub version: Uuid,
    pub namespace: String,
    pub inputs: Vec<DatasetId>,
    pub outputs: Vec<DatasetId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_run: Option<Box<Run>>,
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SearchResultType {
    #[serde(rename = "DATASET")]
    Dataset,
    #[serde(rename = "JOB")]
    Job,
}

impl fmt::Display for SearchResultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dataset => f.write_str("DATASET"),
            Self::Job => f.write_str("JOB"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    #[serde(rename = "type")]
    pub type_: SearchResultType,
    pub name: String,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    pub namespace: String,
    pub node_id: String,
}

/// Simple search result — type, name, namespace, updatedAt (no nodeId).
///
/// Used by `/api/v1/search/simple`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleSearchResult {
    #[serde(rename = "type")]
    pub type_: SearchResultType,
    pub name: String,
    pub namespace: String,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
}

/// Dataset object for full search results.
///
/// Matches Java's `SimpleDataset` from `FullSearchDao.searchDatasets()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleDataset {
    pub id: DatasetId,
    #[serde(rename = "type")]
    pub type_: DatasetType,
    pub name: String,
    pub physical_name: String,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    pub namespace: String,
    pub source_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<Uuid>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "super::iso8601::serialize_option",
        deserialize_with = "super::iso8601::deserialize_option"
    )]
    pub last_modified_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_lifecycle_state: Option<String>,
    pub is_deleted: bool,
    pub fields: Vec<serde_json::Value>,
    pub tags: Vec<String>,
    #[serde(default = "default_empty_object")]
    pub facets: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_current_version: Option<bool>,
}

/// Job object for full search results.
///
/// Matches Java's `SimpleJob` from `FullSearchDao.searchJobs()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleJob {
    pub id: JobId,
    #[serde(rename = "type")]
    pub type_: JobType,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simple_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_job_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_job_uuid: Option<Uuid>,
    #[serde(with = "super::iso8601")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub updated_at: DateTime<Utc>,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub tags: Vec<String>,
    pub labels: Vec<String>,
    #[serde(default = "default_empty_object")]
    pub facets: serde_json::Value,
}

/// Full search results with separate datasets and jobs lists.
///
/// Used by `/api/v1/search/full`. Cannot use `ResultsPage` since it needs
/// two separate lists.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullSearchResults {
    pub total_count: i64,
    pub datasets: Vec<SimpleDataset>,
    pub jobs: Vec<SimpleJob>,
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// Paginated response with a dynamic key name for the items array.
///
/// Mirrors Java's `ResultsPage` which uses `@JsonAnyGetter` to serialize
/// the items under an entity-specific key (e.g., `"jobs"`, `"datasets"`).
///
/// Example: `ResultsPage::new("jobs", jobs, 42)` serializes to:
/// ```json
/// { "totalCount": 42, "jobs": [...] }
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsPage {
    pub total_count: i64,
    #[serde(flatten)]
    items: HashMap<String, serde_json::Value>,
}

impl ResultsPage {
    pub fn new<T: Serialize>(key: &str, items: Vec<T>, total_count: i64) -> Self {
        Self {
            total_count,
            items: HashMap::from([(
                key.to_string(),
                serde_json::to_value(items).unwrap_or_default(),
            )]),
        }
    }
}

// ---------------------------------------------------------------------------
// Lineage graph types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    /// Returns `true` if this node ID contains a version component.
    ///
    /// Java's `NodeId.hasVersion()` checks for `#` separator followed by
    /// a version UUID.
    pub fn has_version(&self) -> bool {
        self.0.contains('#')
    }

    /// Parse a node ID into `expected` parts.
    ///
    /// Mirrors Java's `NodeId.parts()` — first tries a simple split on `:`.
    /// If too many parts (namespace contains URI colons like `hive://host:9083`),
    /// falls back to splitting only on `:` that is NOT followed by `//` (URI
    /// scheme separator) or an ASCII digit (port number).
    pub fn parse_parts(&self, expected: usize) -> Result<Vec<&str>, crate::error::AppError> {
        let simple: Vec<&str> = self.0.split(':').collect();
        if simple.len() == expected {
            return Ok(simple);
        }
        if simple.len() < expected {
            return Err(crate::error::AppError::BadRequest(format!(
                "Invalid node ID: {}",
                self.0
            )));
        }
        // Too many parts — find delimiter positions that are NOT inside URIs.
        // A `:` is a real delimiter if the character after it is neither `/` (from `://`)
        // nor an ASCII digit (from `:9083`).
        let bytes = self.0.as_bytes();
        let mut delimiters: Vec<usize> = Vec::new();
        for (i, &b) in bytes.iter().enumerate() {
            if b == b':' {
                let next = bytes.get(i + 1).copied();
                let is_uri_scheme = next == Some(b'/');
                let is_port = next.is_some_and(|c| c.is_ascii_digit());
                if !is_uri_scheme && !is_port {
                    delimiters.push(i);
                }
            }
        }
        if delimiters.len() < expected - 1 {
            return Err(crate::error::AppError::BadRequest(format!(
                "Invalid node ID: {}",
                self.0
            )));
        }
        let mut parts = Vec::with_capacity(expected);
        let mut prev = 0;
        for &d in delimiters.iter().take(expected - 1) {
            parts.push(&self.0[prev..d]);
            prev = d + 1;
        }
        parts.push(&self.0[prev..]);
        Ok(parts)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub origin: NodeId,
    pub destination: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: NodeId,
    #[serde(rename = "type")]
    pub type_: NodeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub in_edges: Vec<Edge>,
    pub out_edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lineage {
    pub graph: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnLineageInputField {
    pub namespace: String,
    pub dataset: String,
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transformation_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transformation_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnLineageNode {
    pub name: String,
    pub input_fields: Vec<ColumnLineageInputField>,
    #[serde(default)]
    pub output_fields: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transformation_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transformation_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Request body types (Meta)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceMeta {
    pub owner_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMeta {
    #[serde(rename = "type")]
    pub type_: String,
    pub connection_url: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagMeta {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetMeta {
    #[serde(rename = "type")]
    pub type_: String,
    pub physical_name: String,
    pub source_name: String,
    #[serde(default)]
    pub fields: Vec<crate::models::common::Field>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub run_id: Option<uuid::Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobMeta {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub inputs: Vec<crate::models::common::DatasetId>,
    #[serde(default)]
    pub outputs: Vec<crate::models::common::DatasetId>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub run_id: Option<uuid::Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMeta {
    pub id: Option<uuid::Uuid>,
    pub nominal_start_time: Option<DateTime<Utc>>,
    pub nominal_end_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Response wrapper types (matching Java response envelopes)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct NamespacesResponse {
    pub namespaces: Vec<Namespace>,
}

#[derive(Debug, Serialize)]
pub struct SourcesResponse {
    pub sources: Vec<Source>,
}

#[derive(Debug, Serialize)]
pub struct TagsResponse {
    pub tags: Vec<Tag>,
}

// ---------------------------------------------------------------------------
// Lineage event response
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageEventResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(with = "super::iso8601")]
    pub event_time: DateTime<Utc>,
    pub run: serde_json::Value,
    pub job: serde_json::Value,
    #[serde(default = "default_empty_array")]
    pub inputs: serde_json::Value,
    #[serde(default = "default_empty_array")]
    pub outputs: serde_json::Value,
    pub producer: String,
}

// ---------------------------------------------------------------------------
// Stats response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageMetricResponse {
    #[serde(with = "super::iso8601")]
    pub start: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub end: DateTime<Utc>,
    pub fail: i64,
    pub complete: i64,
    pub abort: i64,
    pub start_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalMetricResponse {
    #[serde(with = "super::iso8601")]
    pub start: DateTime<Utc>,
    #[serde(with = "super::iso8601")]
    pub end: DateTime<Utc>,
    pub count: i64,
}

// ---------------------------------------------------------------------------
// v2beta search response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct V2betaSearchResponse {
    pub hits: Vec<SearchResult>,
    pub highlights: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Upstream run lineage types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct UpstreamRunLineage {
    pub runs: Vec<UpstreamRun>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamRun {
    pub job: JobSummary,
    pub run: RunSummary,
    pub inputs: Vec<DatasetSummary>,
}

#[derive(Debug, Serialize)]
pub struct JobSummary {
    pub namespace: String,
    pub name: String,
    pub version: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct RunSummary {
    pub id: Uuid,
    pub start: Option<String>,
    pub end: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSummary {
    pub namespace: String,
    pub name: String,
    pub version: Option<Uuid>,
    pub produced_by_run_id: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::common::*;
    use chrono::TimeZone;

    fn sample_datetime() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap()
    }

    #[test]
    fn namespace_json_roundtrip() {
        let ns = Namespace {
            name: "my-namespace".into(),
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            owner_name: "owner".into(),
            description: Some("A namespace".into()),
            is_hidden: false,
        };
        let json = serde_json::to_value(&ns).unwrap();
        assert_eq!(json["name"], "my-namespace");
        assert_eq!(json["ownerName"], "owner");
        assert_eq!(json["isHidden"], false);
        assert!(json.get("owner_name").is_none(), "should use camelCase");

        // Dates must use Z suffix (not +00:00) to match Java API
        let created = json["createdAt"].as_str().unwrap();
        assert!(created.ends_with('Z'), "expected Z suffix, got: {created}");
        let updated = json["updatedAt"].as_str().unwrap();
        assert!(updated.ends_with('Z'), "expected Z suffix, got: {updated}");

        let back: Namespace = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, "my-namespace");
        assert_eq!(back.owner_name, "owner");
    }

    #[test]
    fn source_json_roundtrip() {
        let src = Source {
            type_: "POSTGRESQL".into(),
            name: "my-source".into(),
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            connection_url: "jdbc:postgresql://localhost:5432/mydb".into(),
            description: None,
        };
        let json = serde_json::to_value(&src).unwrap();
        assert_eq!(json["type"], "POSTGRESQL");
        assert_eq!(
            json["connectionUrl"],
            "jdbc:postgresql://localhost:5432/mydb"
        );
        assert!(json.get("type_").is_none(), "should rename to 'type'");

        let back: Source = serde_json::from_value(json).unwrap();
        assert_eq!(back.type_, "POSTGRESQL");
    }

    #[test]
    fn tag_json_roundtrip() {
        let tag = Tag {
            name: "pii".into(),
            description: Some("Personally identifiable information".into()),
        };
        let json = serde_json::to_value(&tag).unwrap();
        assert_eq!(json["name"], "pii");

        let back: Tag = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, "pii");
        assert_eq!(
            back.description.unwrap(),
            "Personally identifiable information"
        );
    }

    #[test]
    fn dataset_json_roundtrip() {
        let ds = Dataset {
            id: DatasetId {
                namespace: NamespaceName::new("ns"),
                name: DatasetName::new("my-dataset"),
            },
            type_: DatasetType::DbTable,
            name: "my-dataset".into(),
            physical_name: "public.my_dataset".into(),
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            namespace: "ns".into(),
            source_name: "my-source".into(),
            fields: vec![],
            tags: vec!["pii".into()],
            last_modified_at: None,
            last_lifecycle_state: None,
            description: Some("A dataset".into()),
            current_version: None,
            facets: serde_json::json!({}),
            is_deleted: false,
            column_lineage: serde_json::Value::Null,
        };
        let json = serde_json::to_value(&ds).unwrap();
        assert_eq!(json["type"], "DB_TABLE");
        assert_eq!(json["physicalName"], "public.my_dataset");
        assert_eq!(json["sourceName"], "my-source");
        assert_eq!(json["deleted"], false);
        assert!(
            json.get("isDeleted").is_none(),
            "should serialize as 'deleted'"
        );
        assert_eq!(json["facets"], serde_json::json!({}));

        let back: Dataset = serde_json::from_value(json).unwrap();
        assert_eq!(back.type_, DatasetType::DbTable);
        assert_eq!(back.physical_name, "public.my_dataset");
    }

    #[test]
    fn job_json_roundtrip() {
        let job = Job {
            id: JobId {
                namespace: NamespaceName::new("ns"),
                name: JobName::new("my-job"),
            },
            type_: JobType::Batch,
            name: "my-job".into(),
            simple_name: Some("my-job".into()),
            parent_job_name: None,
            parent_job_uuid: None,
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            namespace: "ns".into(),
            inputs: vec![],
            outputs: vec![],
            location: Some("https://github.com/example".into()),
            description: None,
            latest_run: None,
            facets: serde_json::json!({}),
            current_version: None,
            tags: vec!["etl".into()],
            labels: vec![],
            latest_runs: vec![],
        };
        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["type"], "BATCH");
        assert_eq!(json["simpleName"], "my-job");
        assert!(
            json.get("parentJobName").is_none(),
            "null fields should be omitted"
        );
        assert_eq!(json["facets"], serde_json::json!({}));

        let back: Job = serde_json::from_value(json).unwrap();
        assert_eq!(back.type_, JobType::Batch);
        assert_eq!(back.simple_name, Some("my-job".into()));
    }

    #[test]
    fn run_json_roundtrip() {
        let run = Run {
            id: RunId::new(uuid::Uuid::nil()),
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            nominal_start_time: Some(sample_datetime()),
            nominal_end_time: None,
            state: RunState::Running,
            started_at: Some(sample_datetime()),
            ended_at: None,
            duration_ms: Some(12345),
            args: serde_json::json!({}),
            job_version: None,
            input_dataset_versions: serde_json::json!([]),
            output_dataset_versions: serde_json::json!([]),
            facets: serde_json::json!({}),
        };
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["state"], "RUNNING");
        assert_eq!(json["durationMs"], 12345);
        assert!(
            json.get("nominalEndTime").is_none(),
            "null fields should be omitted"
        );
        assert_eq!(json["args"], serde_json::json!({}));
        assert_eq!(json["facets"], serde_json::json!({}));

        // All date fields must use Z suffix
        let created = json["createdAt"].as_str().unwrap();
        assert!(created.ends_with('Z'), "expected Z suffix, got: {created}");
        let started = json["nominalStartTime"].as_str().unwrap();
        assert!(started.ends_with('Z'), "expected Z suffix, got: {started}");

        let back: Run = serde_json::from_value(json).unwrap();
        assert_eq!(back.state, RunState::Running);
        assert_eq!(back.duration_ms, Some(12345));
    }

    #[test]
    fn dataset_version_json_roundtrip() {
        let dv = DatasetVersion {
            id: DatasetVersionId {
                namespace: NamespaceName::new("ns"),
                name: DatasetName::new("ds"),
                version: Uuid::nil(),
            },
            type_: DatasetType::Stream,
            name: "ds".into(),
            physical_name: "ds_phys".into(),
            created_at: sample_datetime(),
            version: Uuid::nil(),
            namespace: "ns".into(),
            source_name: "src".into(),
            fields: vec![],
            tags: vec![],
            last_modified_at: None,
            description: None,
            current_schema_version: None,
            lifecycle_state: Some("ACTIVE".into()),
            run: None,
            facets: serde_json::json!({}),
        };
        let json = serde_json::to_value(&dv).unwrap();
        assert_eq!(json["type"], "STREAM");
        assert_eq!(json["lifecycleState"], "ACTIVE");
        assert_eq!(json["facets"], serde_json::json!({}));
        // run=None is omitted (matches Java NON_NULL), and uses "createdByRun" key
        assert!(
            json.get("createdByRun").is_none(),
            "null run should be omitted"
        );
        assert!(json.get("run").is_none(), "should rename to 'createdByRun'");

        let back: DatasetVersion = serde_json::from_value(json).unwrap();
        assert_eq!(back.type_, DatasetType::Stream);
    }

    #[test]
    fn job_version_json_roundtrip() {
        let jv = JobVersion {
            id: JobVersionId {
                namespace: NamespaceName::new("ns"),
                name: JobName::new("job"),
                version: Uuid::nil(),
            },
            type_: JobType::Service,
            name: "job".into(),
            created_at: sample_datetime(),
            version: Uuid::nil(),
            namespace: "ns".into(),
            inputs: vec![],
            outputs: vec![],
            location: None,
            latest_run: None,
        };
        let json = serde_json::to_value(&jv).unwrap();
        assert_eq!(json["type"], "SERVICE");

        let back: JobVersion = serde_json::from_value(json).unwrap();
        assert_eq!(back.type_, JobType::Service);
    }

    #[test]
    fn search_result_json_roundtrip() {
        let sr = SearchResult {
            type_: SearchResultType::Dataset,
            name: "my-dataset".into(),
            updated_at: sample_datetime(),
            namespace: "ns".into(),
            node_id: "dataset:ns:my-dataset".into(),
        };
        let json = serde_json::to_value(&sr).unwrap();
        assert_eq!(json["type"], "DATASET");
        assert_eq!(json["nodeId"], "dataset:ns:my-dataset");

        let back: SearchResult = serde_json::from_value(json).unwrap();
        assert_eq!(back.type_, SearchResultType::Dataset);
    }

    #[test]
    fn search_result_type_job() {
        let sr = SearchResult {
            type_: SearchResultType::Job,
            name: "my-job".into(),
            updated_at: sample_datetime(),
            namespace: "ns".into(),
            node_id: "job:ns:my-job".into(),
        };
        let json = serde_json::to_value(&sr).unwrap();
        assert_eq!(json["type"], "JOB");

        let back: SearchResult = serde_json::from_value(json).unwrap();
        assert_eq!(back.type_, SearchResultType::Job);
    }

    #[test]
    fn results_page_json_dynamic_key() {
        let page = ResultsPage::new(
            "tags",
            vec![
                Tag {
                    name: "a".into(),
                    description: None,
                },
                Tag {
                    name: "b".into(),
                    description: Some("tag b".into()),
                },
            ],
            42,
        );
        let json = serde_json::to_value(&page).unwrap();
        assert_eq!(json["totalCount"], 42);
        // Items are under the dynamic "tags" key, not "results"
        assert_eq!(json["tags"].as_array().unwrap().len(), 2);
        assert!(json.get("results").is_none());
    }

    #[test]
    fn results_page_jobs_key() {
        let page = ResultsPage::new("jobs", Vec::<Tag>::new(), 0);
        let json = serde_json::to_value(&page).unwrap();
        assert_eq!(json["totalCount"], 0);
        assert!(json["jobs"].is_array());
        assert!(json.get("results").is_none());
    }

    #[test]
    fn node_id_display_and_from() {
        let id = NodeId::new("dataset:ns:ds");
        assert_eq!(id.to_string(), "dataset:ns:ds");
        assert_eq!(id.value(), "dataset:ns:ds");

        let id2: NodeId = "job:ns:j".into();
        assert_eq!(id2.value(), "job:ns:j");

        let id3: NodeId = String::from("run:abc").into();
        assert_eq!(id3.value(), "run:abc");
    }

    #[test]
    fn lineage_graph_json_roundtrip() {
        let lineage = Lineage {
            graph: vec![Node {
                id: NodeId::new("dataset:ns:ds"),
                type_: NodeType::Dataset,
                data: Some(serde_json::json!({"key": "value"})),
                in_edges: vec![Edge {
                    origin: NodeId::new("job:ns:j"),
                    destination: NodeId::new("dataset:ns:ds"),
                }],
                out_edges: vec![],
            }],
        };
        let json = serde_json::to_value(&lineage).unwrap();
        let graph = json["graph"].as_array().unwrap();
        assert_eq!(graph.len(), 1);
        assert_eq!(graph[0]["type"], "DATASET");
        assert_eq!(graph[0]["inEdges"][0]["origin"], "job:ns:j");

        let back: Lineage = serde_json::from_value(json).unwrap();
        assert_eq!(back.graph.len(), 1);
        assert_eq!(back.graph[0].type_, NodeType::Dataset);
    }

    #[test]
    fn column_lineage_node_json_roundtrip() {
        let node = ColumnLineageNode {
            name: "output_col".into(),
            input_fields: vec![ColumnLineageInputField {
                namespace: "ns".into(),
                dataset: "ds".into(),
                field: "input_col".into(),
                transformation_description: Some("identity".into()),
                transformation_type: Some("IDENTITY".into()),
            }],
            output_fields: vec![],
            transformation_description: Some("passthrough".into()),
            transformation_type: Some("IDENTITY".into()),
        };
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["name"], "output_col");
        assert_eq!(json["inputFields"][0]["namespace"], "ns");
        assert_eq!(
            json["inputFields"][0]["transformationDescription"],
            "identity"
        );
        assert_eq!(json["transformationType"], "IDENTITY");

        let back: ColumnLineageNode = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, "output_col");
        assert_eq!(back.input_fields.len(), 1);
    }

    #[test]
    fn job_with_boxed_latest_run() {
        let run = Run {
            id: RunId::new(uuid::Uuid::nil()),
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            nominal_start_time: None,
            nominal_end_time: None,
            state: RunState::Completed,
            started_at: None,
            ended_at: None,
            duration_ms: None,
            args: serde_json::json!({}),
            job_version: None,
            input_dataset_versions: serde_json::json!([]),
            output_dataset_versions: serde_json::json!([]),
            facets: serde_json::json!({}),
        };
        let job = Job {
            id: JobId {
                namespace: NamespaceName::new("ns"),
                name: JobName::new("j"),
            },
            type_: JobType::Batch,
            name: "j".into(),
            simple_name: None,
            parent_job_name: None,
            parent_job_uuid: None,
            created_at: sample_datetime(),
            updated_at: sample_datetime(),
            namespace: "ns".into(),
            inputs: vec![],
            outputs: vec![],
            location: None,
            description: None,
            latest_run: Some(Box::new(run)),
            facets: serde_json::json!({}),
            current_version: None,
            tags: vec![],
            labels: vec![],
            latest_runs: vec![],
        };
        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["latestRun"]["state"], "COMPLETED");

        let back: Job = serde_json::from_value(json).unwrap();
        assert!(back.latest_run.is_some());
        assert_eq!(back.latest_run.unwrap().state, RunState::Completed);
    }

    // ---- NodeId::parse_parts tests ----

    #[test]
    fn parse_parts_simple_dataset() {
        let id = NodeId::new("dataset:ns:name");
        let parts = id.parse_parts(3).unwrap();
        assert_eq!(parts, vec!["dataset", "ns", "name"]);
    }

    #[test]
    fn parse_parts_uri_namespace_hive() {
        let id = NodeId::new("dataset:hive://ilum-hive-metastore:9083:default.campaigns");
        let parts = id.parse_parts(3).unwrap();
        assert_eq!(
            parts,
            vec![
                "dataset",
                "hive://ilum-hive-metastore:9083",
                "default.campaigns"
            ]
        );
    }

    #[test]
    fn parse_parts_s3_namespace() {
        let id = NodeId::new("dataset:s3://bucket/path:dataset-name");
        let parts = id.parse_parts(3).unwrap();
        assert_eq!(parts, vec!["dataset", "s3://bucket/path", "dataset-name"]);
    }

    #[test]
    fn parse_parts_job_with_uri() {
        let id = NodeId::new("job:hive://host:9083:my-job");
        let parts = id.parse_parts(3).unwrap();
        assert_eq!(parts, vec!["job", "hive://host:9083", "my-job"]);
    }

    #[test]
    fn parse_parts_run_two_parts() {
        let id = NodeId::new("run:some-uuid");
        let parts = id.parse_parts(2).unwrap();
        assert_eq!(parts, vec!["run", "some-uuid"]);
    }

    #[test]
    fn parse_parts_too_few_parts() {
        let id = NodeId::new("dataset");
        assert!(id.parse_parts(3).is_err());
    }

    #[test]
    fn parse_parts_dataset_field_four_parts() {
        let id = NodeId::new("datasetField:hive://host:9083:ds:field");
        let parts = id.parse_parts(4).unwrap();
        assert_eq!(
            parts,
            vec!["datasetField", "hive://host:9083", "ds", "field"]
        );
    }
}
