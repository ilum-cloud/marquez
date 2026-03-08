use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::extractors::{JsonQuery, JsonQueryFullSearch, SearchParams};
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::ResultsPage;

pub async fn search(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit.unwrap_or(10).min(1000);
    let results = state
        .search_svc
        .search(
            &params.q,
            params.filter.as_deref(),
            Some(params.sort.as_deref().unwrap_or("updated_at")),
            limit,
            params.namespace.as_deref(),
            params.before.as_deref(),
            params.after.as_deref(),
        )
        .await?;
    let total_count = results.len() as i64;
    Ok(Json(ResultsPage::new("results", results, total_count)))
}

pub async fn simple_search(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let results = state
        .search_svc
        .simple_search(
            &params.q,
            params.filter.as_deref(),
            Some(params.sort.as_deref().unwrap_or("updated_at")),
            limit,
            params.namespace.as_deref(),
        )
        .await?;
    let total_count = results.len() as i64;
    Ok(Json(ResultsPage::new("results", results, total_count)))
}

pub async fn full_search(
    State(state): State<AppState>,
    JsonQueryFullSearch(params): JsonQueryFullSearch,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0).max(0);
    let include_facets = params.facets.unwrap_or(true);
    let facet_names: Vec<String> = params
        .facet_names
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let facet_names = if facet_names.is_empty() {
        None
    } else {
        Some(facet_names)
    };

    let results = state
        .search_svc
        .full_search(
            &params.q,
            params.filter.as_deref(),
            Some(params.sort.as_deref().unwrap_or("updated_at")),
            limit,
            offset,
            params.namespace.as_deref(),
            include_facets,
            facet_names.as_deref(),
        )
        .await?;
    Ok(Json(results))
}
