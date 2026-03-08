use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};

use crate::api::extractors::{EventsParams, JsonQuery, LineageParams, UpstreamRunParams};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::{NodeId, ResultsPage};
use crate::models::iso8601;
use crate::models::openlineage::{DatasetEvent, JobEvent, LineageEvent};

pub async fn get_lineage(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<LineageParams>,
) -> Result<impl IntoResponse, AppError> {
    let node_id = NodeId::new(params.node_id);
    let depth = params.depth.unwrap_or(20);
    let lineage = state.lineage_svc.get_lineage(&node_id, depth).await?;
    Ok(Json(lineage))
}

pub async fn create_lineage_event(
    State(state): State<AppState>,
    crate::api::extractors::ValidJson(body): crate::api::extractors::ValidJson<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    // Determine event type: run event (has "run" key), dataset event (has "dataset" key),
    // or job event (has "job" key without "run")
    if body.get("run").is_some() {
        // Run event — parse as LineageEvent
        let event: LineageEvent = serde_json::from_value(body)
            .map_err(|e| AppError::BadRequest(format!("Invalid lineage event: {}", e)))?;
        state.openlineage_svc.create_lineage_event(&event).await?;
    } else if body.get("dataset").is_some() {
        // Dataset event — parse as DatasetEvent
        let event: DatasetEvent = serde_json::from_value(body)
            .map_err(|e| AppError::BadRequest(format!("Invalid dataset event: {}", e)))?;
        state.openlineage_svc.create_dataset_event(&event).await?;
    } else if body.get("job").is_some() {
        // Job event — parse as JobEvent
        let event: JobEvent = serde_json::from_value(body)
            .map_err(|e| AppError::BadRequest(format!("Invalid job event: {}", e)))?;
        state.openlineage_svc.create_job_event(&event).await?;
    } else {
        return Err(AppError::UnprocessableEntity(
            "Unrecognizable OpenLineage event: missing 'run', 'dataset', or 'job' key".into(),
        ));
    }

    Ok(StatusCode::CREATED.into_response())
}

pub async fn list_events(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<EventsParams>,
) -> Result<impl IntoResponse, AppError> {
    // Java defaults: before=2030-01-01, after=1970-01-01 (all events)
    let before = params
        .before
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| "2030-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap());
    let after = params
        .after
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap());
    let limit = params.limit();
    let offset = params.offset();
    let sort_direction = params.sort_direction.as_deref().unwrap_or("DESC");

    let events = state
        .openlineage_svc
        .list_events(before, after, limit, offset, sort_direction)
        .await?;
    let total_count = state.openlineage_svc.get_total_count(before, after).await?;

    // Convert events to JSON response — the event column contains the full OL event.
    // Java re-serializes through model classes which always include job.facets
    // and run.facets as null, so we ensure those keys exist.
    let results: Vec<serde_json::Value> = events
        .into_iter()
        .map(|e| {
            let mut event = if let Some(ev) = e.event {
                ev
            } else {
                serde_json::json!({
                    "eventType": e.event_type,
                    "eventTime": e.event_time.as_ref().map(iso8601::fmt),
                    "producer": e.producer,
                })
            };
            // Ensure job.facets and run.facets exist (null when absent)
            if let Some(job) = event.get_mut("job") {
                if let Some(obj) = job.as_object_mut() {
                    obj.entry("facets").or_insert(serde_json::Value::Null);
                }
            }
            if let Some(run) = event.get_mut("run") {
                if let Some(obj) = run.as_object_mut() {
                    obj.entry("facets").or_insert(serde_json::Value::Null);
                }
            }
            event
        })
        .collect();

    Ok(Json(ResultsPage::new("events", results, total_count)))
}

pub async fn get_upstream_runs(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<UpstreamRunParams>,
) -> Result<impl IntoResponse, AppError> {
    let depth = params.depth.unwrap_or(20);
    let result = state
        .lineage_svc
        .get_upstream_runs(params.run_id, depth)
        .await?;
    Ok(Json(result))
}
