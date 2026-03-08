use std::sync::Arc;

use chrono::{DateTime, Utc};
use marquez_search::SearchClient;
use sqlx::PgPool;

use crate::db;
use crate::error::AppError;
use crate::models::db::LineageEventRow;
use crate::models::openlineage::{DatasetEvent, JobEvent, LineageEvent};

pub struct OpenLineageService {
    pool: PgPool,
    search_client: Option<Arc<SearchClient>>,
}

impl OpenLineageService {
    pub fn new(pool: PgPool, search_client: Option<Arc<SearchClient>>) -> Self {
        Self {
            pool,
            search_client,
        }
    }

    /// Process a lineage event (run event).
    ///
    /// Stores the raw event and updates the Marquez model.
    /// The heavy lifting is in `db::openlineage::update_marquez_model`.
    pub async fn create_lineage_event(&self, event: &LineageEvent) -> Result<(), AppError> {
        db::openlineage::update_marquez_model(&self.pool, event).await?;

        if let Some(ref client) = self.search_client {
            let client = client.clone();
            let event = event.clone();
            tokio::spawn(async move {
                index_lineage_event(&client, &event).await;
            });
        }

        Ok(())
    }

    /// Process a lineage event asynchronously (fire-and-forget).
    /// Spawns the processing in a background task with error logging.
    pub fn create_lineage_event_async(&self, event: LineageEvent) {
        let pool = self.pool.clone();
        let search_client = self.search_client.clone();
        tokio::spawn(async move {
            if let Err(e) = db::openlineage::update_marquez_model(&pool, &event).await {
                tracing::error!("Failed to process lineage event: {}", e);
            }

            if let Some(ref client) = search_client {
                index_lineage_event(client, &event).await;
            }
        });
    }

    /// Process a dataset event: update model and store raw event.
    pub async fn create_dataset_event(&self, event: &DatasetEvent) -> Result<(), AppError> {
        db::openlineage::update_marquez_model_dataset_event(&self.pool, event).await?;
        Ok(())
    }

    /// Process a job event: update model and store raw event.
    pub async fn create_job_event(&self, event: &JobEvent) -> Result<(), AppError> {
        db::openlineage::update_marquez_model_job_event(&self.pool, event).await?;
        Ok(())
    }

    /// List events with cursor-based pagination.
    pub async fn list_events(
        &self,
        before: DateTime<Utc>,
        after: DateTime<Utc>,
        limit: i32,
        offset: i32,
        sort_direction: &str,
    ) -> Result<Vec<LineageEventRow>, AppError> {
        let rows = match sort_direction.to_uppercase().as_str() {
            "ASC" => {
                db::openlineage::get_all_events_asc(&self.pool, before, after, limit, offset)
                    .await?
            }
            _ => {
                db::openlineage::get_all_events_desc(&self.pool, before, after, limit, offset)
                    .await?
            }
        };
        Ok(rows)
    }

    /// Get total event count within a time range.
    pub async fn get_total_count(
        &self,
        before: DateTime<Utc>,
        after: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        Ok(db::openlineage::get_total_count(&self.pool, before, after).await?)
    }
}

/// Index datasets and job from a lineage event into OpenSearch.
/// Errors are logged but never propagated (graceful degradation).
async fn index_lineage_event(client: &SearchClient, event: &LineageEvent) {
    let run_id = resolve_run_id(&event.run.run_id);
    let event_type = event.event_type.as_deref().unwrap_or("UNKNOWN").to_string();

    // Index input datasets
    if let Some(ref inputs) = event.inputs {
        for ds in inputs {
            let doc = marquez_search::models::DatasetDocument {
                run_id: run_id.clone(),
                event_type: event_type.clone(),
                name: ds.name.clone(),
                namespace: ds.namespace.clone(),
                facets: ds
                    .facets
                    .as_ref()
                    .map(|f| serde_json::to_value(f).unwrap_or_default())
                    .unwrap_or_default(),
                input_facets: ds
                    .input_facets
                    .as_ref()
                    .map(|f| serde_json::to_value(f).unwrap_or_default())
                    .unwrap_or_default(),
                output_facets: serde_json::Value::Null,
            };
            if let Err(e) = client.index_dataset(&doc).await {
                tracing::warn!(
                    "Failed to index input dataset {}:{}: {e}",
                    ds.namespace,
                    ds.name
                );
            }
        }
    }

    // Index output datasets
    if let Some(ref outputs) = event.outputs {
        for ds in outputs {
            let doc = marquez_search::models::DatasetDocument {
                run_id: run_id.clone(),
                event_type: event_type.clone(),
                name: ds.name.clone(),
                namespace: ds.namespace.clone(),
                facets: ds
                    .facets
                    .as_ref()
                    .map(|f| serde_json::to_value(f).unwrap_or_default())
                    .unwrap_or_default(),
                input_facets: serde_json::Value::Null,
                output_facets: ds
                    .output_facets
                    .as_ref()
                    .map(|f| serde_json::to_value(f).unwrap_or_default())
                    .unwrap_or_default(),
            };
            if let Err(e) = client.index_dataset(&doc).await {
                tracing::warn!(
                    "Failed to index output dataset {}:{}: {e}",
                    ds.namespace,
                    ds.name
                );
            }
        }
    }

    // Index job — detect streaming type from jobType.processingType facet
    let job_type = event
        .job
        .facets
        .as_ref()
        .and_then(|f| f.get("jobType"))
        .and_then(|jt| jt.get("processingType"))
        .and_then(|pt| pt.as_str())
        .map(|pt| {
            if pt.eq_ignore_ascii_case("STREAMING") {
                "STREAM"
            } else {
                "BATCH"
            }
        })
        .unwrap_or("BATCH")
        .to_string();

    let doc = marquez_search::models::JobDocument {
        run_id: run_id.clone(),
        event_type,
        name: event.job.name.clone(),
        type_: job_type,
        namespace: event.job.namespace.clone(),
        facets: event
            .job
            .facets
            .as_ref()
            .map(|f| serde_json::to_value(f).unwrap_or_default())
            .unwrap_or_default(),
        run_facets: event
            .run
            .facets
            .as_ref()
            .map(|f| serde_json::to_value(f).unwrap_or_default())
            .unwrap_or_default(),
    };
    if let Err(e) = client.index_job(&doc).await {
        tracing::warn!(
            "Failed to index job {}:{}: {e}",
            event.job.namespace,
            event.job.name
        );
    }
}

/// Resolve a run ID string to a UUID string.
/// If parsing as UUID fails, derive one deterministically using raw MD5
/// (matching Java's `UUID.nameUUIDFromBytes()`).
fn resolve_run_id(raw: &str) -> String {
    match uuid::Uuid::parse_str(raw) {
        Ok(id) => id.to_string(),
        Err(_) => crate::db::openlineage::uuid_name_from_bytes(raw.as_bytes()).to_string(),
    }
}
