// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! OpenSearch integration for full-text search.
//!
//! Provides a `SearchClient` that indexes OpenLineage events into OpenSearch
//! and supports full-text search with highlighting for datasets and jobs.

pub mod client;
pub mod error;
pub mod models;

pub use client::SearchClient;
pub use error::SearchError;

use serde::Deserialize;

/// Configuration for the OpenSearch integration.
/// Matches the Java `SearchConfig` defaults exactly.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_scheme")]
    pub scheme: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_username")]
    pub username: String,
    #[serde(default = "default_password")]
    pub password: String,
}

fn default_scheme() -> String {
    "http".into()
}

fn default_host() -> String {
    "opensearch".into()
}

fn default_port() -> u16 {
    9200
}

fn default_username() -> String {
    "admin".into()
}

fn default_password() -> String {
    "CHANGEMEPLEASE1@#a".into()
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scheme: default_scheme(),
            host: default_host(),
            port: default_port(),
            username: default_username(),
            password: default_password(),
        }
    }
}
