// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use axum::routing::{get, post, put};
use axum::Router;
use marquez_search::SearchClient;
use sqlx::PgPool;
use tower::Layer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::api::middleware::{NormalizeUriLayer, NormalizeUriService};
use crate::api::v1::{
    column_lineage, datasets, health, jobs, lineage, namespaces, runs, search, sources, stats, tags,
};
use crate::api::v2beta;
use crate::service;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub search_client: Option<Arc<SearchClient>>,
    pub namespace_svc: Arc<service::namespace::NamespaceService>,
    pub source_svc: Arc<service::source::SourceService>,
    pub tag_svc: Arc<service::tag::TagService>,
    pub dataset_svc: Arc<service::dataset::DatasetService>,
    pub job_svc: Arc<service::job::JobService>,
    pub run_svc: Arc<service::run::RunService>,
    pub lineage_svc: Arc<service::lineage::LineageService>,
    pub column_lineage_svc: Arc<service::column_lineage::ColumnLineageService>,
    pub openlineage_svc: Arc<service::openlineage::OpenLineageService>,
    pub search_svc: Arc<service::search::SearchService>,
    pub stats_svc: Arc<service::stats::StatsService>,
}

pub fn build_app(
    pool: PgPool,
    search_client: Option<SearchClient>,
    exclusions: &crate::config::ExclusionsConfig,
) -> NormalizeUriService<Router> {
    let search_client = search_client.map(Arc::new);

    // Resolve namespace exclusion pattern from config (matching Java's Exclusions class)
    let ns_exclusion_pattern = exclusions
        .namespaces
        .on_read
        .as_ref()
        .filter(|r| r.enabled)
        .and_then(|r| r.pattern.clone());

    let state = AppState {
        pool: pool.clone(),
        search_client: search_client.clone(),
        namespace_svc: Arc::new(
            service::namespace::NamespaceService::new(pool.clone())
                .with_exclusion_pattern(ns_exclusion_pattern),
        ),
        source_svc: Arc::new(service::source::SourceService::new(pool.clone())),
        tag_svc: Arc::new(service::tag::TagService::new(pool.clone())),
        dataset_svc: Arc::new(service::dataset::DatasetService::new(pool.clone())),
        job_svc: Arc::new(service::job::JobService::new(pool.clone())),
        run_svc: Arc::new(service::run::RunService::new(pool.clone())),
        lineage_svc: Arc::new(service::lineage::LineageService::new(pool.clone())),
        column_lineage_svc: Arc::new(service::column_lineage::ColumnLineageService::new(
            pool.clone(),
        )),
        openlineage_svc: Arc::new(service::openlineage::OpenLineageService::new(
            pool.clone(),
            search_client,
        )),
        search_svc: Arc::new(service::search::SearchService::new(pool.clone())),
        stats_svc: Arc::new(service::stats::StatsService::new(pool)),
    };

    let router = Router::new()
        // Health checks (on main port too, matching Java behavior)
        .route("/healthcheck", get(health_check_from_app_state))
        .route("/ping", get(health::ping))
        // Namespaces (4)
        .route("/api/v1/namespaces", get(namespaces::list))
        .route(
            "/api/v1/namespaces/{namespace}",
            get(namespaces::get)
                .put(namespaces::create_or_update)
                .delete(namespaces::delete),
        )
        // Sources (3)
        .route("/api/v1/sources", get(sources::list))
        .route(
            "/api/v1/sources/{source}",
            get(sources::get).put(sources::create_or_update),
        )
        // Tags (2)
        .route("/api/v1/tags", get(tags::list))
        .route("/api/v1/tags/{name}", put(tags::create_or_update))
        // Datasets (10)
        .route(
            "/api/v1/namespaces/{namespace}/datasets",
            get(datasets::list),
        )
        .route(
            "/api/v1/namespaces/{namespace}/datasets/{dataset}",
            get(datasets::get)
                .put(datasets::create_or_update)
                .delete(datasets::delete),
        )
        .route(
            "/api/v1/namespaces/{namespace}/datasets/{dataset}/versions",
            get(datasets::list_versions),
        )
        .route(
            "/api/v1/namespaces/{namespace}/datasets/{dataset}/versions/{version}",
            get(datasets::get_version),
        )
        .route(
            "/api/v1/namespaces/{namespace}/datasets/{dataset}/tags/{tag}",
            post(datasets::tag_dataset).delete(datasets::untag_dataset),
        )
        .route(
            "/api/v1/namespaces/{namespace}/datasets/{dataset}/fields/{field}/tags/{tag}",
            post(datasets::tag_field).delete(datasets::untag_field),
        )
        // Jobs (12)
        .route("/api/v1/jobs", get(jobs::list_all))
        .route("/api/v1/namespaces/{namespace}/jobs", get(jobs::list))
        .route(
            "/api/v1/namespaces/{namespace}/jobs/{job}",
            get(jobs::get)
                .put(jobs::create_or_update)
                .delete(jobs::delete),
        )
        .route(
            "/api/v1/namespaces/{namespace}/jobs/{job}/versions",
            get(jobs::list_versions),
        )
        .route(
            "/api/v1/namespaces/{namespace}/jobs/{job}/versions/{version}",
            get(jobs::get_version),
        )
        .route(
            "/api/v1/namespaces/{namespace}/jobs/{job}/runs",
            get(jobs::list_runs).post(jobs::create_run),
        )
        .route("/api/v1/jobs/runs/{id}/facets", get(jobs::get_facets))
        .route(
            "/api/v1/namespaces/{namespace}/jobs/{job}/tags/{tag}",
            post(jobs::tag_job).delete(jobs::untag_job),
        )
        // Runs (5)
        .route("/api/v1/jobs/runs/{id}", get(runs::get))
        .route("/api/v1/jobs/runs/{id}/start", post(runs::start))
        .route("/api/v1/jobs/runs/{id}/complete", post(runs::complete))
        .route("/api/v1/jobs/runs/{id}/fail", post(runs::fail))
        .route("/api/v1/jobs/runs/{id}/abort", post(runs::abort))
        // Lineage / OpenLineage (4)
        .route(
            "/api/v1/lineage",
            get(lineage::get_lineage).post(lineage::create_lineage_event),
        )
        .route("/api/v1/events/lineage", get(lineage::list_events))
        .route(
            "/api/v1/runlineage/upstream",
            get(lineage::get_upstream_runs),
        )
        // Column Lineage (1)
        .route("/api/v1/column-lineage", get(column_lineage::get))
        // Search (3)
        .route("/api/v1/search", get(search::search))
        .route("/api/v1/search/simple", get(search::simple_search))
        .route("/api/v1/search/full", get(search::full_search))
        // Stats (4)
        .route("/api/v1/stats/lineage-events", get(stats::lineage_events))
        .route("/api/v1/stats/jobs", get(stats::jobs))
        .route("/api/v1/stats/datasets", get(stats::datasets))
        .route("/api/v1/stats/sources", get(stats::sources))
        // v2beta Search (2)
        .route("/api/v2beta/search/jobs", get(v2beta::search::jobs))
        .route("/api/v2beta/search/datasets", get(v2beta::search::datasets))
        .fallback(fallback)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(sentry_tower::SentryHttpLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(Level::TRACE))
                .on_response(DefaultOnResponse::new().level(Level::TRACE)),
        )
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state);

    // NormalizeUriLayer must wrap the Router from outside (not via Router::layer())
    // because Router::layer() applies middleware after route matching.
    // Wrapping from outside ensures the URI is re-encoded before route matching.
    NormalizeUriLayer.layer(router)
}

/// Fallback handler for unmatched routes — returns JSON with the unmatched path
/// for easier debugging of proxy/routing issues.
async fn fallback(uri: axum::http::Uri) -> impl axum::response::IntoResponse {
    let body = serde_json::json!({
        "code": 404,
        "message": format!("No route matched: {}", uri.path()),
    });
    (axum::http::StatusCode::NOT_FOUND, axum::Json(body))
}

/// Health check handler that extracts pool from AppState for the main router.
async fn health_check_from_app_state(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    health::health_check(axum::extract::State(state.pool)).await
}

/// Build the admin router (health checks only, served on a separate port).
pub fn build_admin_app(pool: PgPool) -> Router {
    Router::new()
        .route("/healthcheck", get(health::health_check))
        .route("/ping", get(health::ping))
        .with_state(pool)
}
