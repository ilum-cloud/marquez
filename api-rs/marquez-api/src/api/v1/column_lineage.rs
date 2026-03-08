use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;

use crate::api::extractors::{ColumnLineageParams, JsonQuery};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::NodeId;

pub async fn get(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<ColumnLineageParams>,
) -> Result<impl IntoResponse, AppError> {
    let node_id = NodeId::new(params.node_id);
    let depth = params.depth.unwrap_or(20);
    let with_downstream = params.with_downstream.unwrap_or(false);

    // Java rejects withDownstream=true + versioned nodeId with 400,
    // returning {error, message, type} format.
    if with_downstream && node_id.has_version() {
        let body = serde_json::json!({
            "error": "Bad Request",
            "message": "withDownstream=true is not supported with versioned nodeId",
            "type": "bad_request",
        });
        return Ok((StatusCode::BAD_REQUEST, Json(body)).into_response());
    }

    let created_at_until = match params.created_at_until {
        Some(ref s) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        None => Utc::now(),
    };

    let lineage = state
        .column_lineage_svc
        .get_lineage(&node_id, depth, with_downstream, created_at_until)
        .await?;
    // Lineage struct serializes as { "graph": [...] } matching Java API
    Ok(Json(lineage).into_response())
}
