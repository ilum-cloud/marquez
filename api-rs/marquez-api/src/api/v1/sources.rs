use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;

use crate::api::extractors::{JsonQuery, PaginationParams, ValidJson};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::{SourceMeta, SourcesResponse};

pub async fn list(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let sources = state.source_svc.list(limit, offset).await?;
    Ok(Json(SourcesResponse { sources }))
}

pub async fn get(
    State(state): State<AppState>,
    Path(source): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let src = state.source_svc.get(&source).await?;
    Ok(Json(src))
}

pub async fn create_or_update(
    State(state): State<AppState>,
    Path(source): Path<String>,
    ValidJson(body): ValidJson<SourceMeta>,
) -> Result<impl IntoResponse, AppError> {
    let src = state
        .source_svc
        .create_or_update(
            &body.type_,
            &source,
            &body.connection_url,
            body.description.as_deref(),
        )
        .await?;
    Ok(Json(src))
}
