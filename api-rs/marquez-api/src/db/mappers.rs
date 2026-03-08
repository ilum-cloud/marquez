// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! Conversions from database row types to API response types.

use crate::models::api;
use crate::models::common::{
    DatasetId, DatasetName, DatasetType, DatasetVersionId, JobId, JobName, JobType, JobVersionId,
    NamespaceName, RunId, RunState,
};
use crate::models::db;

impl From<db::NamespaceRow> for api::Namespace {
    fn from(row: db::NamespaceRow) -> Self {
        Self {
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            owner_name: row.current_owner_name.unwrap_or_default(),
            description: row.description,
            is_hidden: row.is_hidden.unwrap_or(false),
        }
    }
}

impl From<db::SourceRow> for api::Source {
    fn from(row: db::SourceRow) -> Self {
        Self {
            type_: row.type_,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            connection_url: row.connection_url,
            description: row.description,
        }
    }
}

impl From<db::TagRow> for api::Tag {
    fn from(row: db::TagRow) -> Self {
        Self {
            name: row.name,
            description: row.description,
        }
    }
}

/// Basic mapper from `DatasetRow` to `api::Dataset`.
///
/// Fields that require JOINs (tags, fields, facets) are left empty and
/// must be populated by the service layer.
impl From<db::DatasetRow> for api::Dataset {
    fn from(row: db::DatasetRow) -> Self {
        Self {
            id: DatasetId {
                namespace: NamespaceName::new(row.namespace_name.clone().unwrap_or_default()),
                name: DatasetName::new(&row.name),
            },
            type_: row.type_.parse().unwrap_or(DatasetType::DbTable),
            name: row.name,
            physical_name: row.physical_name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            namespace: row.namespace_name.unwrap_or_default(),
            source_name: row.source_name.unwrap_or_default(),
            fields: vec![],
            tags: vec![],
            last_modified_at: row.last_modified_at,
            last_lifecycle_state: None,
            description: row.description,
            current_version: row.current_version_uuid,
            facets: serde_json::json!({}),
            is_deleted: row.is_deleted.unwrap_or(false),
            column_lineage: serde_json::Value::Null,
        }
    }
}

/// Basic mapper from `JobRow` to `api::Job`.
///
/// Fields that require JOINs (inputs, outputs, latest_run, facets, tags) are
/// left empty and must be populated by the service layer.
impl From<db::JobRow> for api::Job {
    fn from(row: db::JobRow) -> Self {
        Self {
            id: JobId {
                namespace: NamespaceName::new(row.namespace_name.clone().unwrap_or_default()),
                name: JobName::new(&row.name),
            },
            type_: row.type_.parse().unwrap_or(JobType::Batch),
            name: row.name,
            simple_name: row.simple_name,
            parent_job_name: None, // populated by service
            parent_job_uuid: row.parent_job_uuid,
            created_at: row.created_at,
            updated_at: row.updated_at,
            namespace: row.namespace_name.unwrap_or_default(),
            inputs: vec![],
            outputs: vec![],
            location: row.current_location,
            description: row.description,
            latest_run: None,
            facets: serde_json::json!({}),
            current_version: row.current_version_uuid,
            tags: vec![],
            labels: vec![],
            latest_runs: vec![],
        }
    }
}

/// Basic mapper from `RunRow` to `api::Run`.
///
/// Fields that require JOINs (args, facets) are left empty and must be
/// populated by the service layer.
impl From<db::RunRow> for api::Run {
    fn from(row: db::RunRow) -> Self {
        let duration_ms = match (row.started_at, row.ended_at) {
            (Some(start), Some(end)) => {
                let duration = end - start;
                Some(duration.num_milliseconds())
            }
            _ => None,
        };
        Self {
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
            args: serde_json::json!({}),
            job_version: None,
            input_dataset_versions: serde_json::json!([]),
            output_dataset_versions: serde_json::json!([]),
            facets: serde_json::json!({}),
        }
    }
}

/// Basic mapper from `DatasetFieldRow` to a `common::Field`.
///
/// Tags are left empty and must be populated by the service layer.
impl From<db::DatasetFieldRow> for crate::models::common::Field {
    fn from(row: db::DatasetFieldRow) -> Self {
        Self {
            name: crate::models::common::FieldName::new(row.name),
            type_: row.type_,
            tags: vec![],
            description: row.description,
        }
    }
}

/// Mapper from `DatasetFieldWithTagsRow` to a `common::Field`.
///
/// Includes tags from the `dataset_fields_tag_mapping` join.
impl From<db::DatasetFieldWithTagsRow> for crate::models::common::Field {
    fn from(row: db::DatasetFieldWithTagsRow) -> Self {
        Self {
            name: crate::models::common::FieldName::new(row.name),
            type_: row.type_,
            tags: row
                .tags
                .into_iter()
                .map(crate::models::common::TagName::new)
                .collect(),
            description: row.description,
        }
    }
}

/// Basic mapper from `DatasetVersionRow` to `api::DatasetVersion`.
///
/// Fields that require JOINs (fields, tags, run, facets) are left empty and
/// must be populated by the service layer. Source name and physical name are
/// not available on the row, so defaults are used.
impl From<db::DatasetVersionRow> for api::DatasetVersion {
    fn from(row: db::DatasetVersionRow) -> Self {
        Self {
            id: DatasetVersionId {
                namespace: NamespaceName::new(row.namespace_name.clone().unwrap_or_default()),
                name: DatasetName::new(row.dataset_name.as_deref().unwrap_or_default()),
                version: row.version,
            },
            type_: DatasetType::DbTable, // default, service layer can override
            name: row.dataset_name.clone().unwrap_or_default(),
            physical_name: row.dataset_name.unwrap_or_default(), // same as name by default
            created_at: row.created_at,
            version: row.version,
            namespace: row.namespace_name.unwrap_or_default(),
            source_name: String::new(), // populated by service
            fields: vec![],
            tags: vec![],
            last_modified_at: None,
            description: None,
            current_schema_version: row.dataset_schema_version_uuid,
            lifecycle_state: row.lifecycle_state,
            run: None, // populated by service if needed
            facets: serde_json::json!({}),
        }
    }
}

/// Basic mapper from `JobVersionRow` to `api::JobVersion`.
///
/// Fields that require JOINs (inputs, outputs, latest_run) are left empty
/// and must be populated by the service layer.
impl From<db::JobVersionRow> for api::JobVersion {
    fn from(row: db::JobVersionRow) -> Self {
        Self {
            id: JobVersionId {
                namespace: NamespaceName::new(row.namespace_name.clone().unwrap_or_default()),
                name: JobName::new(row.job_name.as_deref().unwrap_or_default()),
                version: row.version,
            },
            type_: JobType::Batch, // default, service layer can override
            name: row.job_name.unwrap_or_default(),
            created_at: row.created_at,
            version: row.version,
            namespace: row.namespace_name.unwrap_or_default(),
            inputs: vec![],  // populated by service
            outputs: vec![], // populated by service
            location: row.location,
            latest_run: None, // populated by service if needed
        }
    }
}
