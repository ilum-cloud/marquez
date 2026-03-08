use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;

use crate::api::extractors::{JsonQuery, PaginationParams};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::{TagMeta, TagsResponse};

pub async fn list(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let tags = state.tag_svc.list(limit, offset).await?;
    Ok(Json(TagsResponse { tags }))
}

pub async fn create_or_update(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<TagMeta>,
) -> Result<impl IntoResponse, AppError> {
    let tag = state
        .tag_svc
        .create_or_update(&name, body.description.as_deref())
        .await?;
    Ok(Json(tag))
}
