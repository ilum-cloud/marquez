use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;

use crate::api::extractors::{JsonQuery, PaginationParams, ValidJson};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::{NamespaceMeta, NamespacesResponse};

pub async fn list(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let namespaces = state.namespace_svc.list(limit, offset).await?;
    Ok(Json(NamespacesResponse { namespaces }))
}

pub async fn get(
    State(state): State<AppState>,
    Path(namespace): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let ns = state.namespace_svc.get(&namespace).await?;
    Ok(Json(ns))
}

pub async fn create_or_update(
    State(state): State<AppState>,
    Path(namespace): Path<String>,
    ValidJson(body): ValidJson<NamespaceMeta>,
) -> Result<impl IntoResponse, AppError> {
    let ns = state
        .namespace_svc
        .create_or_update(&namespace, &body.owner_name, body.description.as_deref())
        .await?;
    Ok(Json(ns))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(namespace): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let ns = state.namespace_svc.get(&namespace).await?;
    state.namespace_svc.delete(&namespace).await?;
    Ok(Json(ns))
}
