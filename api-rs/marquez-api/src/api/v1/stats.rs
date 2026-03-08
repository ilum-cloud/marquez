use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::extractors::{JsonQuery, StatsParams};
use crate::app::AppState;
use crate::error::AppError;

pub async fn lineage_events(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<StatsParams>,
) -> Result<impl IntoResponse, AppError> {
    let rows = match params.period.to_uppercase().as_str() {
        "DAY" => state.stats_svc.get_last_day_metrics().await?,
        "WEEK" => {
            let tz = params.timezone.as_deref().ok_or_else(|| {
                AppError::BadRequest("timezone is required for WEEK period".to_string())
            })?;
            state.stats_svc.get_last_week_metrics(tz).await?
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid period: {}. Must be DAY or WEEK",
                params.period
            )));
        }
    };
    Ok(Json(rows))
}

pub async fn jobs(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<StatsParams>,
) -> Result<impl IntoResponse, AppError> {
    let rows = match params.period.to_uppercase().as_str() {
        "DAY" => state.stats_svc.get_last_day_jobs().await?,
        "WEEK" => {
            let tz = params.timezone.as_deref().ok_or_else(|| {
                AppError::BadRequest("timezone is required for WEEK period".to_string())
            })?;
            state.stats_svc.get_last_week_jobs(tz).await?
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid period: {}. Must be DAY or WEEK",
                params.period
            )));
        }
    };
    Ok(Json(rows))
}

pub async fn datasets(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<StatsParams>,
) -> Result<impl IntoResponse, AppError> {
    let rows = match params.period.to_uppercase().as_str() {
        "DAY" => state.stats_svc.get_last_day_datasets().await?,
        "WEEK" => {
            let tz = params.timezone.as_deref().ok_or_else(|| {
                AppError::BadRequest("timezone is required for WEEK period".to_string())
            })?;
            state.stats_svc.get_last_week_datasets(tz).await?
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid period: {}. Must be DAY or WEEK",
                params.period
            )));
        }
    };
    Ok(Json(rows))
}

pub async fn sources(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<StatsParams>,
) -> Result<impl IntoResponse, AppError> {
    let rows = match params.period.to_uppercase().as_str() {
        "DAY" => state.stats_svc.get_last_day_sources().await?,
        "WEEK" => {
            let tz = params.timezone.as_deref().ok_or_else(|| {
                AppError::BadRequest("timezone is required for WEEK period".to_string())
            })?;
            state.stats_svc.get_last_week_sources(tz).await?
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid period: {}. Must be DAY or WEEK",
                params.period
            )));
        }
    };
    Ok(Json(rows))
}
