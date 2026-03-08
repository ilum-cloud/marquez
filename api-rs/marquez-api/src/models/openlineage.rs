// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Flexible datetime deserializer for OpenLineage event timestamps
// ---------------------------------------------------------------------------

mod flexible_datetime {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&date.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        // Try RFC3339 first (handles Z and timezone offsets)
        if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
            return Ok(dt.with_timezone(&Utc));
        }

        // Try parsing with fractional seconds and timezone offset
        if let Ok(dt) = DateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f%:z") {
            return Ok(dt.with_timezone(&Utc));
        }

        // Try naive datetime (no timezone) with fractional seconds - assume UTC
        if let Ok(ndt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f") {
            return Ok(ndt.and_utc());
        }

        // Try naive datetime without fractional seconds - assume UTC
        if let Ok(ndt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S") {
            return Ok(ndt.and_utc());
        }

        Err(serde::de::Error::custom(format!("invalid datetime: {}", s)))
    }
}

// ---------------------------------------------------------------------------
// OpenLineage event envelope (custom dispatch)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum OpenLineageEvent {
    LineageEvent(LineageEvent),
    DatasetEvent(DatasetEvent),
    JobEvent(JobEvent),
}

impl<'de> Deserialize<'de> for OpenLineageEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let obj = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("expected object"))?;

        if obj.contains_key("run") {
            Ok(OpenLineageEvent::LineageEvent(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ))
        } else if obj.contains_key("dataset") {
            Ok(OpenLineageEvent::DatasetEvent(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ))
        } else if obj.contains_key("job") {
            Ok(OpenLineageEvent::JobEvent(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ))
        } else {
            Err(serde::de::Error::custom(
                "expected run, dataset, or job key",
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Core event types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageEvent {
    pub event_type: Option<String>,
    #[serde(with = "flexible_datetime")]
    pub event_time: DateTime<Utc>,
    pub run: RunRef,
    pub job: JobRef,
    pub inputs: Option<Vec<InputDatasetRef>>,
    pub outputs: Option<Vec<OutputDatasetRef>>,
    pub producer: String,
    #[serde(rename = "schemaURL", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetEvent {
    #[serde(with = "flexible_datetime")]
    pub event_time: DateTime<Utc>,
    pub dataset: DatasetRef,
    pub producer: String,
    #[serde(rename = "schemaURL", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub event_type: Option<String>,
    #[serde(with = "flexible_datetime")]
    pub event_time: DateTime<Utc>,
    pub job: JobRef,
    pub inputs: Option<Vec<InputDatasetRef>>,
    pub outputs: Option<Vec<OutputDatasetRef>>,
    pub producer: String,
    #[serde(rename = "schemaURL", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Reference types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRef {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRef {
    pub namespace: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetRef {
    pub namespace: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "outputFacets", skip_serializing_if = "Option::is_none")]
    pub output_facets: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDatasetRef {
    pub namespace: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "inputFacets", skip_serializing_if = "Option::is_none")]
    pub input_facets: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDatasetRef {
    pub namespace: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "outputFacets", skip_serializing_if = "Option::is_none")]
    pub output_facets: Option<HashMap<String, serde_json::Value>>,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    // -----------------------------------------------------------------------
    // Helper to deserialize and assert variant
    // -----------------------------------------------------------------------

    fn deser_event(json_str: &str) -> OpenLineageEvent {
        serde_json::from_str(json_str).expect("failed to deserialize OpenLineageEvent")
    }

    fn assert_lineage_event(event: &OpenLineageEvent) -> &LineageEvent {
        match event {
            OpenLineageEvent::LineageEvent(e) => e,
            other => panic!(
                "expected LineageEvent, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    fn assert_dataset_event(event: &OpenLineageEvent) -> &DatasetEvent {
        match event {
            OpenLineageEvent::DatasetEvent(e) => e,
            other => panic!(
                "expected DatasetEvent, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    fn assert_job_event(event: &OpenLineageEvent) -> &JobEvent {
        match event {
            OpenLineageEvent::JobEvent(e) => e,
            other => panic!("expected JobEvent, got {:?}", std::mem::discriminant(other)),
        }
    }

    // -----------------------------------------------------------------------
    // Fixture tests: RunEvent (LineageEvent) variants
    // -----------------------------------------------------------------------

    #[test]
    fn fixture_event_required_only() {
        let json = include_str!("../../../tests/fixtures/event_required_only.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        assert_eq!(le.run.run_id, "43d7db3b-8984-4cda-b237-8c6919b36670");
        assert_eq!(le.job.namespace, "my-namespace");
        assert_eq!(le.job.name, "myjob");
        assert_eq!(le.event_type, None);
        assert!(le.schema_url.is_some());
    }

    #[test]
    fn fixture_event_simple() {
        let json = include_str!("../../../tests/fixtures/event_simple.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        assert_eq!(le.event_type.as_deref(), Some("COMPLETE"));
        assert_eq!(le.inputs.as_ref().unwrap().len(), 2);
        assert_eq!(le.outputs.as_ref().unwrap().len(), 1);
        assert_eq!(
            le.inputs.as_ref().unwrap()[0].name,
            "instance.schema.input-1"
        );
    }

    #[test]
    fn fixture_event_full() {
        let json = include_str!("../../../tests/fixtures/event_full.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        assert_eq!(le.event_type.as_deref(), Some("COMPLETE"));
        // Run facets present
        assert!(le.run.facets.is_some());
        let run_facets = le.run.facets.as_ref().unwrap();
        assert!(run_facets.contains_key("nominalTime"));
        assert!(run_facets.contains_key("parent"));
        // Job facets present
        assert!(le.job.facets.is_some());
        let job_facets = le.job.facets.as_ref().unwrap();
        assert!(job_facets.contains_key("documentation"));
        assert!(job_facets.contains_key("sql"));
        // Input facets
        let inputs = le.inputs.as_ref().unwrap();
        assert_eq!(inputs.len(), 1);
        assert!(inputs[0].facets.is_some());
        assert!(inputs[0].input_facets.is_some());
        // Output facets
        let outputs = le.outputs.as_ref().unwrap();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].facets.is_some());
        assert!(outputs[0].output_facets.is_some());
    }

    #[test]
    fn fixture_event_additional_facet() {
        let json = include_str!("../../../tests/fixtures/event_additional_facet.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        assert_eq!(le.event_type.as_deref(), Some("START"));
        // Additional run facets
        let run_facets = le.run.facets.as_ref().unwrap();
        assert!(run_facets.contains_key("additionalProp1"));
        assert!(run_facets.contains_key("additionalProp2"));
        assert!(run_facets.contains_key("additionalProp3"));
    }

    #[test]
    fn fixture_event_large() {
        let json = include_str!("../../../tests/fixtures/event_large.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        assert_eq!(le.event_type.as_deref(), Some("COMPLETE"));
        assert!(le.run.facets.is_some());
    }

    #[test]
    fn fixture_event_namespace_naming() {
        let json = include_str!("../../../tests/fixtures/event_namespace_naming.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        // Verify S3 URIs work as namespaces
        let inputs = le.inputs.as_ref().unwrap();
        assert_eq!(inputs[0].namespace, "s3://blob-source");
        let outputs = le.outputs.as_ref().unwrap();
        assert_eq!(outputs[0].namespace, "s3://blob-sink");
    }

    #[test]
    fn fixture_event_required_nanoseconds() {
        let json = include_str!("../../../tests/fixtures/event_required_nanoseconds.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        // Timestamp with microseconds: "2020-12-28T19:51:01.641499Z"
        assert_eq!(le.event_time.year(), 2020);
        assert_eq!(le.event_time.month(), 12);
        assert_eq!(le.event_time.day(), 28);
    }

    #[test]
    fn fixture_event_required_no_timezone() {
        let json = include_str!("../../../tests/fixtures/event_required_no_timezone.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        // Timestamp without timezone: "2020-12-28T19:51:01.641499" — assumed UTC
        assert_eq!(le.event_time.year(), 2020);
        assert_eq!(le.event_time.month(), 12);
        assert_eq!(le.event_time.day(), 28);
    }

    #[test]
    fn fixture_event_unicode() {
        let json = include_str!("../../../tests/fixtures/event_unicode.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        // Job name contains unicode spider emoji
        assert!(le.job.name.contains('\u{1F577}'));
    }

    #[test]
    fn fixture_event_without_schema_url() {
        let json = include_str!("../../../tests/fixtures/event_without_schema_url.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        assert!(le.schema_url.is_none());
    }

    #[test]
    fn fixture_null_nominal_end_time() {
        let json = include_str!("../../../tests/fixtures/null_nominal_end_time.json");
        let event = deser_event(json);
        let le = assert_lineage_event(&event);
        assert_eq!(le.event_type.as_deref(), Some("START"));
        // nominalEndTime is absent (not null) in run facets — facets are a HashMap so this is fine
        let run_facets = le.run.facets.as_ref().unwrap();
        let nominal = run_facets.get("nominalTime").unwrap();
        assert!(nominal.get("nominalStartTime").is_some());
    }

    // -----------------------------------------------------------------------
    // Fixture tests: DatasetEvent
    // -----------------------------------------------------------------------

    #[test]
    fn fixture_event_dataset_event() {
        let json = include_str!("../../../tests/fixtures/event_dataset_event.json");
        let event = deser_event(json);
        let de = assert_dataset_event(&event);
        assert_eq!(de.dataset.namespace, "my-dataset-namespace");
        assert_eq!(de.dataset.name, "my-dataset-name");
        assert!(de.dataset.facets.is_some());
        assert!(de.schema_url.is_some());
    }

    // -----------------------------------------------------------------------
    // Fixture tests: JobEvent
    // -----------------------------------------------------------------------

    #[test]
    fn fixture_event_job_event() {
        let json = include_str!("../../../tests/fixtures/event_job_event.json");
        let event = deser_event(json);
        let je = assert_job_event(&event);
        assert_eq!(je.event_type.as_deref(), Some("COMPLETE"));
        assert_eq!(je.job.namespace, "my-scheduler-namespace");
        assert_eq!(je.job.name, "myjob");
        assert!(je.inputs.is_some());
        assert!(je.outputs.is_some());
    }

    // -----------------------------------------------------------------------
    // Flexible datetime tests
    // -----------------------------------------------------------------------

    #[test]
    fn flexible_datetime_rfc3339_z() {
        let event_json = r#"{"eventTime":"2020-12-28T19:51:01.641Z","run":{"runId":"00000000-0000-0000-0000-000000000000"},"job":{"namespace":"ns","name":"j"},"producer":"p"}"#;
        let event: LineageEvent = serde_json::from_str(event_json).unwrap();
        assert_eq!(event.event_time.year(), 2020);
        assert_eq!(event.event_time.month(), 12);
        assert_eq!(event.event_time.day(), 28);
    }

    #[test]
    fn flexible_datetime_timezone_offset() {
        let event_json = r#"{"eventTime":"2020-12-28T19:52:00.001+10:00","run":{"runId":"00000000-0000-0000-0000-000000000000"},"job":{"namespace":"ns","name":"j"},"producer":"p"}"#;
        let event: LineageEvent = serde_json::from_str(event_json).unwrap();
        // +10:00 offset means UTC is 09:52
        assert_eq!(event.event_time.hour(), 9);
        assert_eq!(event.event_time.minute(), 52);
    }

    #[test]
    fn flexible_datetime_no_timezone() {
        let event_json = r#"{"eventTime":"2020-12-28T19:51:01.641499","run":{"runId":"00000000-0000-0000-0000-000000000000"},"job":{"namespace":"ns","name":"j"},"producer":"p"}"#;
        let event: LineageEvent = serde_json::from_str(event_json).unwrap();
        // No timezone — assumed UTC
        assert_eq!(event.event_time.hour(), 19);
        assert_eq!(event.event_time.minute(), 51);
    }

    #[test]
    fn flexible_datetime_microseconds() {
        let event_json = r#"{"eventTime":"2020-12-28T19:51:01.641499Z","run":{"runId":"00000000-0000-0000-0000-000000000000"},"job":{"namespace":"ns","name":"j"},"producer":"p"}"#;
        let event: LineageEvent = serde_json::from_str(event_json).unwrap();
        assert_eq!(event.event_time.year(), 2020);
        assert_eq!(event.event_time.second(), 1);
    }

    // -----------------------------------------------------------------------
    // Serialization roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn lineage_event_roundtrip() {
        let event = LineageEvent {
            event_type: Some("COMPLETE".into()),
            event_time: Utc::now(),
            run: RunRef {
                run_id: "00000000-0000-0000-0000-000000000000".into(),
                facets: None,
            },
            job: JobRef {
                namespace: "ns".into(),
                name: "j".into(),
                facets: None,
            },
            inputs: Some(vec![InputDatasetRef {
                namespace: "ns".into(),
                name: "input".into(),
                facets: None,
                input_facets: None,
            }]),
            outputs: None,
            producer: "test".into(),
            schema_url: Some("https://example.com".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: LineageEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_type, event.event_type);
        assert_eq!(back.run.run_id, event.run.run_id);
        assert_eq!(back.producer, "test");
    }

    #[test]
    fn dataset_event_roundtrip() {
        let event = DatasetEvent {
            event_time: Utc::now(),
            dataset: DatasetRef {
                namespace: "ns".into(),
                name: "ds".into(),
                facets: Some(HashMap::from([(
                    "schema".into(),
                    serde_json::json!({"fields": []}),
                )])),
                output_facets: None,
            },
            producer: "test".into(),
            schema_url: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: DatasetEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.dataset.namespace, "ns");
        assert!(back.dataset.facets.is_some());
    }

    #[test]
    fn job_event_roundtrip() {
        let event = JobEvent {
            event_type: Some("START".into()),
            event_time: Utc::now(),
            job: JobRef {
                namespace: "ns".into(),
                name: "j".into(),
                facets: None,
            },
            inputs: None,
            outputs: Some(vec![OutputDatasetRef {
                namespace: "ns".into(),
                name: "out".into(),
                facets: None,
                output_facets: Some(HashMap::from([(
                    "stats".into(),
                    serde_json::json!({"rowCount": 100}),
                )])),
            }]),
            producer: "test".into(),
            schema_url: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: JobEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_type, Some("START".into()));
        assert!(back.outputs.is_some());
        let outs = back.outputs.unwrap();
        assert!(outs[0].output_facets.is_some());
    }

    #[test]
    fn open_lineage_event_dispatch_lineage() {
        let json = r#"{
            "eventTime": "2020-12-28T19:51:01.641Z",
            "run": {"runId": "00000000-0000-0000-0000-000000000000"},
            "job": {"namespace": "ns", "name": "j"},
            "producer": "p"
        }"#;
        let event: OpenLineageEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, OpenLineageEvent::LineageEvent(_)));
    }

    #[test]
    fn open_lineage_event_dispatch_dataset() {
        let json = r#"{
            "eventTime": "2020-12-28T19:51:01.641Z",
            "dataset": {"namespace": "ns", "name": "ds"},
            "producer": "p"
        }"#;
        let event: OpenLineageEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, OpenLineageEvent::DatasetEvent(_)));
    }

    #[test]
    fn open_lineage_event_dispatch_job() {
        let json = r#"{
            "eventTime": "2020-12-28T19:51:01.641Z",
            "job": {"namespace": "ns", "name": "j"},
            "producer": "p"
        }"#;
        let event: OpenLineageEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, OpenLineageEvent::JobEvent(_)));
    }

    #[test]
    fn open_lineage_event_dispatch_error_no_key() {
        let json = r#"{"eventTime": "2020-12-28T19:51:01.641Z", "producer": "p"}"#;
        let result: Result<OpenLineageEvent, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn schema_url_serialized_correctly() {
        let event = LineageEvent {
            event_type: None,
            event_time: Utc::now(),
            run: RunRef {
                run_id: "id".into(),
                facets: None,
            },
            job: JobRef {
                namespace: "ns".into(),
                name: "j".into(),
                facets: None,
            },
            inputs: None,
            outputs: None,
            producer: "p".into(),
            schema_url: Some("https://openlineage.io/spec/1-0-1/OpenLineage.json".into()),
        };
        let json = serde_json::to_value(&event).unwrap();
        // Verify the key is "schemaURL" not "schema_url" or "schemaUrl"
        assert!(json.get("schemaURL").is_some());
        assert!(json.get("schema_url").is_none());
        assert!(json.get("schemaUrl").is_none());
    }

    #[test]
    fn input_facets_key_serialized_correctly() {
        let input = InputDatasetRef {
            namespace: "ns".into(),
            name: "ds".into(),
            facets: None,
            input_facets: Some(HashMap::from([(
                "quality".into(),
                serde_json::json!({"score": 0.99}),
            )])),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert!(json.get("inputFacets").is_some());
        assert!(json.get("input_facets").is_none());
    }

    #[test]
    fn output_facets_key_serialized_correctly() {
        let output = OutputDatasetRef {
            namespace: "ns".into(),
            name: "ds".into(),
            facets: None,
            output_facets: Some(HashMap::from([(
                "stats".into(),
                serde_json::json!({"rows": 42}),
            )])),
        };
        let json = serde_json::to_value(&output).unwrap();
        assert!(json.get("outputFacets").is_some());
        assert!(json.get("output_facets").is_none());
    }
}
