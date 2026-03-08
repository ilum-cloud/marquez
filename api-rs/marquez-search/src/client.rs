// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use opensearch::auth::Credentials;
use opensearch::http::transport::{SingleNodeConnectionPool, TransportBuilder};
use opensearch::indices::{IndicesCreateParts, IndicesExistsParts};
use opensearch::{IndexParts, OpenSearch, SearchParts};
use url::Url;

use crate::error::SearchError;
use crate::models::{SearchHit, SearchResponse};
use crate::SearchConfig;

/// Fields to search for datasets — matches Java `SearchDao.DATASET_FIELDS`.
const DATASET_FIELDS: &[&str] = &[
    "run_id",
    "name",
    "namespace",
    "facets.schema.fields.name",
    "facets.schema.fields.type",
    "facets.columnLineage.fields.*.inputFields.name",
    "facets.columnLineage.fields.*.inputFields.namespace",
    "facets.columnLineage.fields.*.inputFields.field",
    "facets.columnLineage.fields.*.transformationDescription",
    "facets.columnLineage.fields.*.transformationType",
];

/// Fields to search for jobs — matches Java `SearchDao.JOB_FIELDS`.
const JOB_FIELDS: &[&str] = &[
    "facets.sql.query",
    "facets.sourceCode.sourceCode",
    "facets.sourceCode.language",
    "runFacets.processing_engine.name",
    "run_id",
    "name",
    "namespace",
    "type",
];

/// Wrapper around the OpenSearch client providing Marquez-specific operations.
#[derive(Clone)]
pub struct SearchClient {
    client: OpenSearch,
}

impl SearchClient {
    /// Create a new search client from configuration.
    ///
    /// Returns `Ok(None)` if search is disabled.
    /// Returns `Err` if search is enabled but the client cannot be created.
    pub fn new(config: &SearchConfig) -> Result<Option<Self>, SearchError> {
        if !config.enabled {
            return Ok(None);
        }

        let url_str = format!("{}://{}:{}", config.scheme, config.host, config.port);
        let url = Url::parse(&url_str).map_err(|e| {
            SearchError::Connection(format!("Invalid OpenSearch URL '{url_str}': {e}"))
        })?;

        let conn_pool = SingleNodeConnectionPool::new(url);
        let credentials = Credentials::Basic(config.username.clone(), config.password.clone());

        let transport = TransportBuilder::new(conn_pool)
            .auth(credentials)
            .build()
            .map_err(|e| SearchError::Connection(format!("Failed to build transport: {e}")))?;

        Ok(Some(Self {
            client: OpenSearch::new(transport),
        }))
    }

    /// Ping OpenSearch to check connectivity. Returns `true` if reachable.
    pub async fn ping(&self) -> bool {
        match self.client.ping().send().await {
            Ok(resp) => {
                let ok = resp.status_code().is_success();
                if ok {
                    tracing::info!("OpenSearch Active: true");
                } else {
                    tracing::warn!(
                        "OpenSearch ping returned status {}",
                        resp.status_code().as_u16()
                    );
                }
                ok
            }
            Err(e) => {
                tracing::warn!("OpenSearch ping failed: {e}");
                false
            }
        }
    }

    /// Ping with retry — tries up to `max_retries` times with a 3-second delay.
    /// Returns `true` if any attempt succeeds.
    pub async fn ping_with_retry(&self, max_retries: u32) -> bool {
        for attempt in 1..=max_retries {
            if self.ping().await {
                return true;
            }
            if attempt < max_retries {
                tracing::info!(
                    "OpenSearch ping attempt {}/{} failed, retrying in 3s...",
                    attempt,
                    max_retries
                );
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
        tracing::warn!(
            "OpenSearch ping failed after {} attempts — search will use PostgreSQL only",
            max_retries
        );
        false
    }

    /// Ensure the `jobs` and `datasets` indices exist, creating them if needed.
    /// Logs results but never fails — search degrades gracefully.
    pub async fn ensure_indices(&self) {
        for index in &["jobs", "datasets"] {
            match self
                .client
                .indices()
                .exists(IndicesExistsParts::Index(&[index]))
                .send()
                .await
            {
                Ok(resp) if resp.status_code().is_success() => {
                    tracing::info!("Index already exists: {index}");
                }
                Ok(_) => {
                    // Index doesn't exist — create it
                    match self
                        .client
                        .indices()
                        .create(IndicesCreateParts::Index(index))
                        .send()
                        .await
                    {
                        Ok(resp) if resp.status_code().is_success() => {
                            tracing::info!("Created index: {index}");
                        }
                        Ok(resp) => {
                            let status = resp.status_code().as_u16();
                            let body = resp.text().await.unwrap_or_else(|_| "<unreadable>".into());
                            tracing::warn!(
                                "Failed to create index {index} (HTTP {status}): {body}"
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create index {index}: {e}");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to check index {index}: {e}");
                }
            }
        }
    }

    /// Index a dataset document.
    pub async fn index_dataset(
        &self,
        doc: &crate::models::DatasetDocument,
    ) -> Result<(), SearchError> {
        let id = format!("DATASET:{}:{}", doc.namespace, doc.name);
        let body = serde_json::to_value(doc)?;

        let response = self
            .client
            .index(IndexParts::IndexId("datasets", &id))
            .body(body)
            .send()
            .await?;

        let status = response.status_code();
        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".into());
            return Err(SearchError::Response {
                status: status.as_u16(),
                body: body_text,
            });
        }

        tracing::debug!("Indexed dataset {id}");
        Ok(())
    }

    /// Index a job document.
    pub async fn index_job(&self, doc: &crate::models::JobDocument) -> Result<(), SearchError> {
        let id = format!("JOB:{}:{}", doc.namespace, doc.name);
        let body = serde_json::to_value(doc)?;

        let response = self
            .client
            .index(IndexParts::IndexId("jobs", &id))
            .body(body)
            .send()
            .await?;

        let status = response.status_code();
        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".into());
            return Err(SearchError::Response {
                status: status.as_u16(),
                body: body_text,
            });
        }

        tracing::debug!("Indexed job {id}");
        Ok(())
    }

    /// Search datasets using multi_match phrase_prefix with highlighting.
    pub async fn search_datasets(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<SearchResponse, SearchError> {
        self.search_index("datasets", query, DATASET_FIELDS, limit)
            .await
    }

    /// Search jobs using multi_match phrase_prefix with highlighting.
    pub async fn search_jobs(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<SearchResponse, SearchError> {
        self.search_index("jobs", query, JOB_FIELDS, limit).await
    }

    /// Execute a multi_match phrase_prefix search with highlighting on the given index.
    async fn search_index(
        &self,
        index: &str,
        query: &str,
        fields: &[&str],
        limit: i64,
    ) -> Result<SearchResponse, SearchError> {
        // Build highlight fields object — one entry per field, matching Java's Plain highlighter
        let highlight_fields: serde_json::Value = fields
            .iter()
            .map(|f| (f.to_string(), serde_json::json!({})))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        let body = serde_json::json!({
            "size": limit,
            "query": {
                "multi_match": {
                    "query": query,
                    "type": "phrase_prefix",
                    "fields": fields,
                }
            },
            "highlight": {
                "type": "plain",
                "fields": highlight_fields,
            }
        });

        let response = self
            .client
            .search(SearchParts::Index(&[index]))
            .body(body)
            .send()
            .await?;

        let status = response.status_code();

        // Index doesn't exist yet — no data has been indexed, return empty results
        if status.as_u16() == 404 {
            return Ok(SearchResponse {
                hits: vec![],
                total: 0,
            });
        }

        let response_body: serde_json::Value = response.json().await?;

        if !status.is_success() {
            return Err(SearchError::Response {
                status: status.as_u16(),
                body: response_body.to_string(),
            });
        }

        parse_search_response(&response_body)
    }
}

/// Parse the OpenSearch JSON response into our `SearchResponse` type.
fn parse_search_response(body: &serde_json::Value) -> Result<SearchResponse, SearchError> {
    let total = body["hits"]["total"]["value"].as_u64().unwrap_or(0);

    let hits = body["hits"]["hits"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|hit| {
                    let id = hit["_id"].as_str().unwrap_or("").to_string();
                    let source = hit["_source"].clone();
                    let score = hit["_score"].as_f64().unwrap_or(0.0);

                    let highlights = hit["highlight"]
                        .as_object()
                        .map(|obj| {
                            obj.iter()
                                .map(|(k, v)| {
                                    let fragments = v
                                        .as_array()
                                        .map(|arr| {
                                            arr.iter()
                                                .filter_map(|s| s.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    (k.clone(), fragments)
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    SearchHit {
                        id,
                        source,
                        highlights,
                        score,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(SearchResponse { hits, total })
}
