use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::api::extractors::{JsonQuery, PaginationParams, ValidJson};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::{DatasetMeta, ResultsPage};

pub async fn list(
    State(state): State<AppState>,
    Path(namespace): Path<String>,
    JsonQuery(params): JsonQuery<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let datasets = state.dataset_svc.list(&namespace, limit, offset).await?;
    let total_count = state.dataset_svc.count(&namespace).await?;
    Ok(Json(ResultsPage::new("datasets", datasets, total_count)))
}

pub async fn get(
    State(state): State<AppState>,
    Path((namespace, dataset)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let ds = state.dataset_svc.get(&namespace, &dataset).await?;
    Ok(Json(ds))
}

pub async fn create_or_update(
    State(state): State<AppState>,
    Path((namespace, dataset)): Path<(String, String)>,
    ValidJson(body): ValidJson<DatasetMeta>,
) -> Result<impl IntoResponse, AppError> {
    let ds = state
        .dataset_svc
        .create_or_update(
            &namespace,
            &dataset,
            &body.type_,
            &body.source_name,
            &body.physical_name,
            body.description.as_deref(),
            &body.fields,
            &body.tags,
        )
        .await?;
    Ok(Json(ds))
}

pub async fn delete(
    State(state): State<AppState>,
    Path((namespace, dataset)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let ds = state.dataset_svc.get(&namespace, &dataset).await?;
    state.dataset_svc.delete(&namespace, &dataset).await?;
    Ok(Json(ds))
}

pub async fn list_versions(
    State(state): State<AppState>,
    Path((namespace, dataset)): Path<(String, String)>,
    JsonQuery(params): JsonQuery<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let versions = state
        .dataset_svc
        .list_versions(&namespace, &dataset, limit, offset)
        .await?;
    let total_count = state
        .dataset_svc
        .count_versions(&namespace, &dataset)
        .await?;
    Ok(Json(ResultsPage::new("versions", versions, total_count)))
}

pub async fn get_version(
    State(state): State<AppState>,
    Path((namespace, dataset, version)): Path<(String, String, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let v = state
        .dataset_svc
        .get_version(&namespace, &dataset, version)
        .await?;
    Ok(Json(v))
}

pub async fn tag_dataset(
    State(state): State<AppState>,
    Path((namespace, dataset, tag)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let ds = state
        .dataset_svc
        .tag_dataset(&namespace, &dataset, &tag)
        .await?;
    Ok(Json(ds))
}

pub async fn untag_dataset(
    State(state): State<AppState>,
    Path((namespace, dataset, tag)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let ds = state
        .dataset_svc
        .delete_tag(&namespace, &dataset, &tag)
        .await?;
    Ok(Json(ds))
}

pub async fn tag_field(
    State(state): State<AppState>,
    Path((namespace, dataset, field, tag)): Path<(String, String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let tag = tag.to_uppercase();
    let ds = state
        .dataset_svc
        .tag_field(&namespace, &dataset, &field, &tag)
        .await?;
    Ok(Json(ds))
}

pub async fn untag_field(
    State(state): State<AppState>,
    Path((namespace, dataset, field, tag)): Path<(String, String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let tag = tag.to_uppercase();
    let ds = state
        .dataset_svc
        .untag_field(&namespace, &dataset, &field, &tag)
        .await?;
    Ok(Json(ds))
}
