// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Search error: {0}")]
    Search(String),
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl From<marquez_search::SearchError> for AppError {
    fn from(e: marquez_search::SearchError) -> Self {
        AppError::Search(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::UnprocessableEntity(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Internal(_) | AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Search(_) => StatusCode::BAD_GATEWAY,
            AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        };

        // Capture server errors to Sentry
        if status.is_server_error() {
            sentry::capture_message(&self.to_string(), sentry::Level::Error);
        }

        let msg = self.to_string();
        let body = serde_json::json!({
            "code": status.as_u16(),
            "message": msg,
            "errors": [msg],
        });
        (status, axum::Json(body)).into_response()
    }
}
