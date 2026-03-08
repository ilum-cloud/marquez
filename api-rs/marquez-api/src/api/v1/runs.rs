use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::api::extractors::{JsonPath, JsonQuery, RunTransitionParams};
use crate::app::AppState;
use crate::error::AppError;

fn parse_at(at: Option<&str>) -> DateTime<Utc> {
    at.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

pub async fn get(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let run = state.run_svc.get(id).await?;
    Ok(Json(run))
}

pub async fn start(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonQuery(params): JsonQuery<RunTransitionParams>,
) -> Result<impl IntoResponse, AppError> {
    let at = parse_at(params.at.as_deref());
    let run = state.run_svc.start(id, at).await?;
    Ok(Json(run))
}

pub async fn complete(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonQuery(params): JsonQuery<RunTransitionParams>,
) -> Result<impl IntoResponse, AppError> {
    let at = parse_at(params.at.as_deref());
    let run = state.run_svc.complete(id, at).await?;
    Ok(Json(run))
}

pub async fn fail(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonQuery(params): JsonQuery<RunTransitionParams>,
) -> Result<impl IntoResponse, AppError> {
    let at = parse_at(params.at.as_deref());
    let run = state.run_svc.fail(id, at).await?;
    Ok(Json(run))
}

pub async fn abort(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonQuery(params): JsonQuery<RunTransitionParams>,
) -> Result<impl IntoResponse, AppError> {
    let at = parse_at(params.at.as_deref());
    let run = state.run_svc.abort(id, at).await?;
    Ok(Json(run))
}
