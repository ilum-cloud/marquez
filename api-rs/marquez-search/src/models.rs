// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Document indexed for a dataset in OpenSearch.
/// Field names match the Java `SearchService.buildDatasetIndexRequest` exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetDocument {
    pub run_id: String,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub name: String,
    pub namespace: String,
    #[serde(default)]
    pub facets: serde_json::Value,
    #[serde(rename = "inputFacets", default)]
    pub input_facets: serde_json::Value,
    #[serde(rename = "outputFacets", default)]
    pub output_facets: serde_json::Value,
}

/// Document indexed for a job in OpenSearch.
/// Field names match the Java `SearchService.buildJobIndexRequest` exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDocument {
    pub run_id: String,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub namespace: String,
    #[serde(default)]
    pub facets: serde_json::Value,
    #[serde(rename = "runFacets", default)]
    pub run_facets: serde_json::Value,
}

/// A single search hit returned from OpenSearch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub source: serde_json::Value,
    pub highlights: HashMap<String, Vec<String>>,
    pub score: f64,
}

/// Aggregated search response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_document_serialization() {
        let doc = DatasetDocument {
            run_id: "abc-123".into(),
            event_type: "COMPLETE".into(),
            name: "my_dataset".into(),
            namespace: "prod".into(),
            facets: serde_json::json!({"schema": {"fields": []}}),
            input_facets: serde_json::json!({}),
            output_facets: serde_json::json!({}),
        };
        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["eventType"], "COMPLETE");
        assert_eq!(json["run_id"], "abc-123");
        assert_eq!(json["inputFacets"], serde_json::json!({}));
        assert_eq!(json["outputFacets"], serde_json::json!({}));
    }

    #[test]
    fn job_document_serialization() {
        let doc = JobDocument {
            run_id: "abc-123".into(),
            event_type: "START".into(),
            name: "my_job".into(),
            type_: "BATCH".into(),
            namespace: "prod".into(),
            facets: serde_json::json!({}),
            run_facets: serde_json::json!({}),
        };
        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["type"], "BATCH");
        assert_eq!(json["runFacets"], serde_json::json!({}));
    }

    #[test]
    fn dataset_doc_id_format() {
        let ns = "prod";
        let name = "my_dataset";
        let id = format!("DATASET:{ns}:{name}");
        assert_eq!(id, "DATASET:prod:my_dataset");
    }

    #[test]
    fn job_doc_id_format() {
        let ns = "prod";
        let name = "my_job";
        let id = format!("JOB:{ns}:{name}");
        assert_eq!(id, "JOB:prod:my_job");
    }
}
