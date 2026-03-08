use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;

use crate::api::extractors::SearchParams;
use crate::app::AppState;
use crate::error::AppError;
use crate::models::api::{SearchResult, SearchResultType, V2betaSearchResponse};

pub async fn jobs(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit.unwrap_or(10);

    // v2beta search requires OpenSearch — return 503 if not configured
    if state.search_client.is_none() {
        return Err(AppError::ServiceUnavailable(
            "Search is not configured".to_string(),
        ));
    }

    // Try OpenSearch first if available
    if let Some(ref client) = state.search_client {
        match client.search_jobs(&params.q, limit as i64).await {
            Ok(response) if !response.hits.is_empty() => {
                let hits: Vec<SearchResult> = response
                    .hits
                    .iter()
                    .map(|hit| search_hit_to_result(hit, "JOB"))
                    .collect();
                let highlights: Vec<serde_json::Value> = response
                    .hits
                    .iter()
                    .map(|hit| serde_json::to_value(&hit.highlights).unwrap_or_default())
                    .collect();
                return Ok(Json(V2betaSearchResponse { hits, highlights }));
            }
            Ok(_) => {
                // Empty results from OpenSearch — fall through to PG
            }
            Err(e) => {
                tracing::warn!("OpenSearch job search failed, falling back to PG: {e}");
            }
        }
    }

    // Fallback to PostgreSQL ILIKE search
    let results = state
        .search_svc
        .search(
            &params.q,
            Some("JOB"),
            params.sort.as_deref(),
            limit,
            params.namespace.as_deref(),
            params.before.as_deref(),
            params.after.as_deref(),
        )
        .await?;
    // Provide one empty highlight object per hit so the frontend can safely
    // call Object.entries(highlights[i]) without hitting null/undefined.
    let highlights = results.iter().map(|_| serde_json::json!({})).collect();
    Ok(Json(V2betaSearchResponse {
        hits: results,
        highlights,
    }))
}

pub async fn datasets(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit.unwrap_or(10);

    // v2beta search requires OpenSearch — return 503 if not configured
    if state.search_client.is_none() {
        return Err(AppError::ServiceUnavailable(
            "Search is not configured".to_string(),
        ));
    }

    // Try OpenSearch first if available
    if let Some(ref client) = state.search_client {
        match client.search_datasets(&params.q, limit as i64).await {
            Ok(response) if !response.hits.is_empty() => {
                let hits: Vec<SearchResult> = response
                    .hits
                    .iter()
                    .map(|hit| search_hit_to_result(hit, "DATASET"))
                    .collect();
                let highlights: Vec<serde_json::Value> = response
                    .hits
                    .iter()
                    .map(|hit| serde_json::to_value(&hit.highlights).unwrap_or_default())
                    .collect();
                return Ok(Json(V2betaSearchResponse { hits, highlights }));
            }
            Ok(_) => {
                // Empty results from OpenSearch — fall through to PG
            }
            Err(e) => {
                tracing::warn!("OpenSearch dataset search failed, falling back to PG: {e}");
            }
        }
    }

    // Fallback to PostgreSQL ILIKE search
    let results = state
        .search_svc
        .search(
            &params.q,
            Some("DATASET"),
            params.sort.as_deref(),
            limit,
            params.namespace.as_deref(),
            params.before.as_deref(),
            params.after.as_deref(),
        )
        .await?;
    // Provide one empty highlight object per hit so the frontend can safely
    // call Object.entries(highlights[i]) without hitting null/undefined.
    let highlights = results.iter().map(|_| serde_json::json!({})).collect();
    Ok(Json(V2betaSearchResponse {
        hits: results,
        highlights,
    }))
}

/// Convert a search hit from OpenSearch into a `SearchResult`.
fn search_hit_to_result(hit: &marquez_search::models::SearchHit, type_str: &str) -> SearchResult {
    let name = hit.source["name"].as_str().unwrap_or("").to_string();
    let namespace = hit.source["namespace"].as_str().unwrap_or("").to_string();
    let type_ = match type_str {
        "DATASET" => SearchResultType::Dataset,
        _ => SearchResultType::Job,
    };
    let node_id = format!("{}:{}:{}", type_str.to_lowercase(), namespace, name);

    SearchResult {
        type_,
        name,
        updated_at: chrono::Utc::now(), // OpenSearch doesn't return updated_at
        namespace,
        node_id,
    }
}
