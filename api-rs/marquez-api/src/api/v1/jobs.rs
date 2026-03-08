use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::api::extractors::{
    FacetParams, JsonQuery, JsonQueryJobList, PaginationParams, ValidJson,
};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::{JobMeta, ResultsPage, RunMeta};

pub async fn list_all(
    State(state): State<AppState>,
    JsonQueryJobList(params): JsonQueryJobList,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let jobs = state
        .job_svc
        .list_all(limit, offset, &params.last_run_states)
        .await?;
    let total_count = state.job_svc.count_all().await?;
    Ok(Json(ResultsPage::new("jobs", jobs, total_count)))
}

pub async fn list(
    State(state): State<AppState>,
    Path(namespace): Path<String>,
    JsonQueryJobList(params): JsonQueryJobList,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let jobs = state
        .job_svc
        .list(&namespace, limit, offset, &params.last_run_states)
        .await?;
    let total_count = state.job_svc.count(&namespace).await?;
    Ok(Json(ResultsPage::new("jobs", jobs, total_count)))
}

pub async fn get(
    State(state): State<AppState>,
    Path((namespace, job)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let j = state.job_svc.get(&namespace, &job).await?;
    Ok(Json(j))
}

pub async fn create_or_update(
    State(state): State<AppState>,
    Path((namespace, job)): Path<(String, String)>,
    ValidJson(body): ValidJson<JobMeta>,
) -> Result<impl IntoResponse, AppError> {
    let j = state
        .job_svc
        .create_or_update(
            &namespace,
            &job,
            &body.type_,
            body.description.as_deref(),
            body.location.as_deref(),
        )
        .await?;
    Ok(Json(j))
}

pub async fn delete(
    State(state): State<AppState>,
    Path((namespace, job)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let j = state.job_svc.get(&namespace, &job).await?;
    state.job_svc.delete(&namespace, &job).await?;
    Ok(Json(j))
}

pub async fn list_versions(
    State(state): State<AppState>,
    Path((namespace, job)): Path<(String, String)>,
    JsonQuery(params): JsonQuery<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let versions = state
        .job_svc
        .list_versions(&namespace, &job, limit, offset)
        .await?;
    Ok(Json(serde_json::json!({ "versions": versions })))
}

pub async fn get_version(
    State(state): State<AppState>,
    Path((namespace, job, version)): Path<(String, String, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let v = state.job_svc.get_version(&namespace, &job, version).await?;
    Ok(Json(v))
}

pub async fn list_runs(
    State(state): State<AppState>,
    Path((namespace, job)): Path<(String, String)>,
    JsonQuery(params): JsonQuery<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit();
    let offset = params.offset();
    let runs = state
        .run_svc
        .list_by_job(&namespace, &job, limit, offset)
        .await?;
    let total_count = state.run_svc.count_by_job(&namespace, &job).await?;
    Ok(Json(ResultsPage::new("runs", runs, total_count)))
}

pub async fn create_run(
    State(state): State<AppState>,
    Path((namespace, job)): Path<(String, String)>,
    ValidJson(body): ValidJson<RunMeta>,
) -> Result<impl IntoResponse, AppError> {
    let run = state
        .run_svc
        .create_run(
            &namespace,
            &job,
            body.args.as_ref(),
            body.nominal_start_time,
            body.nominal_end_time,
        )
        .await?;
    let location = format!("/api/v1/jobs/runs/{}", run.id);
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, HeaderValue::from_str(&location).unwrap())],
        Json(run),
    ))
}

pub async fn get_facets(
    State(state): State<AppState>,
    crate::api::extractors::JsonPath(id): crate::api::extractors::JsonPath<Uuid>,
    JsonQuery(params): JsonQuery<FacetParams>,
) -> Result<impl IntoResponse, AppError> {
    let facets = match params.type_.to_uppercase().as_str() {
        "RUN" => crate::db::facets::find_run_facets_by_run(state.run_svc.pool(), id).await?,
        "JOB" => crate::db::facets::find_job_facets_by_run(state.run_svc.pool(), id).await?,
        "DATASET" => return Ok(Json(serde_json::Value::Null)),
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid facet type: {}",
                params.type_
            )));
        }
    };
    Ok(Json(serde_json::json!({
        "runId": id,
        "facets": facets.unwrap_or(serde_json::json!({}))
    })))
}

pub async fn tag_job(
    State(state): State<AppState>,
    Path((namespace, job, tag)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let j = state.job_svc.tag_job(&namespace, &job, &tag).await?;
    Ok(Json(j))
}

pub async fn untag_job(
    State(state): State<AppState>,
    Path((namespace, job, tag)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let j = state.job_svc.untag_job(&namespace, &job, &tag).await?;
    Ok(Json(j))
}
