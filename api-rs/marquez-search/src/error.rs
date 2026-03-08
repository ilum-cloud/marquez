// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

#[derive(Debug)]
pub enum SearchError {
    Transport(opensearch::Error),
    Response { status: u16, body: String },
    Serialization(serde_json::Error),
    Connection(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::Transport(e) => write!(f, "OpenSearch transport error: {e}"),
            SearchError::Response { status, body } => {
                write!(f, "OpenSearch response error (HTTP {status}): {body}")
            }
            SearchError::Serialization(e) => write!(f, "OpenSearch serialization error: {e}"),
            SearchError::Connection(msg) => write!(f, "OpenSearch connection error: {msg}"),
        }
    }
}

impl std::error::Error for SearchError {}

impl From<opensearch::Error> for SearchError {
    fn from(e: opensearch::Error) -> Self {
        SearchError::Transport(e)
    }
}

impl From<serde_json::Error> for SearchError {
    fn from(e: serde_json::Error) -> Self {
        SearchError::Serialization(e)
    }
}
