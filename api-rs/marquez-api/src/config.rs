// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use marquez_search::SearchConfig;
use serde::Deserialize;

fn default_true() -> bool {
    true
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_app_port() -> u16 {
    8080
}

fn default_admin_port() -> u16 {
    8081
}

fn default_db_port() -> u16 {
    5432
}

fn default_db_host() -> String {
    "localhost".to_string()
}

fn default_db_name() -> String {
    "marquez".to_string()
}

fn default_db_user() -> String {
    "marquez".to_string()
}

fn default_db_password() -> String {
    "marquez".to_string()
}

fn default_pool_size() -> u32 {
    10
}

fn default_retention_frequency() -> u64 {
    15
}

fn default_retention_batch_size() -> i64 {
    1000
}

fn default_retention_days() -> i32 {
    7
}

#[derive(Debug, Deserialize)]
pub struct MarquezConfig {
    #[serde(default)]
    pub db: DbConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default = "default_true")]
    pub migrate_on_startup: bool,
    #[serde(default)]
    pub db_retention: Option<DbRetentionConfig>,
    #[serde(default)]
    pub tags: Vec<TagConfig>,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub sentry: marquez_tracing::SentryConfig,
    #[serde(default)]
    pub exclusions: ExclusionsConfig,
}

/// Namespace exclusion configuration matching Java's `ExclusionsConfig`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ExclusionsConfig {
    #[serde(default)]
    pub namespaces: NamespaceExclusions,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct NamespaceExclusions {
    #[serde(default, rename = "onRead")]
    pub on_read: Option<ExclusionRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExclusionRule {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_app_port")]
    pub port: u16,
    #[serde(default = "default_admin_port")]
    pub admin_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_app_port(),
            admin_port: default_admin_port(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DbConfig {
    #[serde(default = "default_db_host")]
    pub host: String,
    #[serde(default = "default_db_port")]
    pub port: u16,
    #[serde(default = "default_db_name")]
    pub name: String,
    #[serde(default = "default_db_user")]
    pub user: String,
    #[serde(default = "default_db_password")]
    pub password: String,
    #[serde(default = "default_pool_size")]
    pub max_pool_size: u32,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            host: default_db_host(),
            port: default_db_port(),
            name: default_db_name(),
            user: default_db_user(),
            password: default_db_password(),
            max_pool_size: default_pool_size(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DbRetentionConfig {
    #[serde(default = "default_retention_frequency")]
    pub frequency_mins: u64,
    #[serde(default = "default_retention_batch_size")]
    pub number_of_rows_per_batch: i64,
    #[serde(default = "default_retention_days")]
    pub retention_days: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TagConfig {
    pub name: String,
    pub description: Option<String>,
}
