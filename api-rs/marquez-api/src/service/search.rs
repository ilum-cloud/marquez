use sqlx::PgPool;

use crate::db;
use crate::error::AppError;
use crate::models::api;
use crate::models::common::{
    DatasetId, DatasetName, DatasetType, JobId, JobName, JobType, NamespaceName,
};
use crate::models::db::{SimpleDatasetRow, SimpleJobRow};

pub struct SearchService {
    pool: PgPool,
}

impl SearchService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn search(
        &self,
        query: &str,
        filter: Option<&str>,
        sort: Option<&str>,
        limit: i32,
        namespace: Option<&str>,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<Vec<api::SearchResult>, AppError> {
        let rows = db::search::search(
            &self.pool, query, filter, sort, limit, namespace, before, after,
        )
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let type_ = match row.type_.as_str() {
                    "DATASET" => api::SearchResultType::Dataset,
                    _ => api::SearchResultType::Job,
                };
                let node_id = format!(
                    "{}:{}:{}",
                    row.type_.to_lowercase(),
                    row.namespace_name,
                    row.name
                );
                api::SearchResult {
                    type_,
                    name: row.name,
                    updated_at: row.updated_at,
                    namespace: row.namespace_name,
                    node_id,
                }
            })
            .collect())
    }

    /// Simple search — returns lightweight results without nodeId.
    pub async fn simple_search(
        &self,
        query: &str,
        filter: Option<&str>,
        sort: Option<&str>,
        limit: i32,
        namespace: Option<&str>,
    ) -> Result<Vec<api::SimpleSearchResult>, AppError> {
        let rows =
            db::search::simple_search(&self.pool, query, filter, sort, limit, namespace).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let type_ = match row.type_.as_str() {
                    "DATASET" => api::SearchResultType::Dataset,
                    _ => api::SearchResultType::Job,
                };
                api::SimpleSearchResult {
                    type_,
                    name: row.name,
                    namespace: row.namespace_name,
                    updated_at: row.updated_at,
                }
            })
            .collect())
    }

    /// Full search — counts datasets/jobs, distributes the limit, and returns
    /// separate lists with full detail (tags, facets, fields).
    pub async fn full_search(
        &self,
        query: &str,
        filter: Option<&str>,
        sort: Option<&str>,
        limit: i32,
        offset: i32,
        namespace: Option<&str>,
        include_facets: bool,
        facet_names: Option<&[String]>,
    ) -> Result<api::FullSearchResults, AppError> {
        let sort_str = sort.unwrap_or("UPDATED_AT");

        // Count matching datasets and jobs in parallel.
        let (dataset_count, job_count) = tokio::try_join!(
            db::search::count_datasets(&self.pool, query, namespace),
            db::search::count_jobs(&self.pool, query, namespace),
        )?;

        // If a type filter is specified, skip the other type.
        let filter_upper = filter.map(|f| f.to_uppercase());
        let skip_datasets = filter_upper.as_deref() == Some("JOB") || dataset_count == 0;
        let skip_jobs = filter_upper.as_deref() == Some("DATASET") || job_count == 0;

        let (ds_limit, job_limit) = if skip_datasets {
            (0, limit)
        } else if skip_jobs {
            (limit, 0)
        } else {
            calculate_optimal_limits(limit, dataset_count, job_count)
        };

        // Fetch datasets and jobs in parallel.
        let (dataset_rows, job_rows) = tokio::try_join!(
            async {
                if ds_limit > 0 {
                    db::search::search_datasets(
                        &self.pool,
                        query,
                        sort_str,
                        ds_limit,
                        offset,
                        namespace,
                        include_facets,
                        facet_names,
                    )
                    .await
                } else {
                    Ok(vec![])
                }
            },
            async {
                if job_limit > 0 {
                    db::search::search_jobs(
                        &self.pool,
                        query,
                        sort_str,
                        job_limit,
                        offset,
                        namespace,
                        include_facets,
                        facet_names,
                    )
                    .await
                } else {
                    Ok(vec![])
                }
            },
        )?;

        let total_count = dataset_count + job_count;

        let datasets: Vec<api::SimpleDataset> =
            dataset_rows.into_iter().map(map_dataset_row).collect();
        let jobs: Vec<api::SimpleJob> = job_rows.into_iter().map(map_job_row).collect();

        Ok(api::FullSearchResults {
            total_count,
            datasets,
            jobs,
        })
    }
}

/// Distribute `limit` between datasets and jobs.
///
/// Evenly splits the limit, then redistributes unused slots when one side
/// has fewer results than its half. For example, limit=20 with 5 datasets
/// and 100 jobs → 5 datasets + 15 jobs.
fn calculate_optimal_limits(limit: i32, dataset_count: i64, job_count: i64) -> (i32, i32) {
    let half = limit / 2;
    let ds_count = dataset_count as i32;
    let job_count = job_count as i32;

    let ds_limit = half.min(ds_count);
    let remaining = limit - ds_limit;
    let job_limit = remaining.min(job_count);
    // Give any leftover back to datasets.
    let ds_limit = (ds_limit + (remaining - job_limit)).min(ds_count);

    (ds_limit, limit - ds_limit)
}

/// Flatten a JSON array of facet objects into a single merged object.
/// Matches Java's `MapperUtils.toFacetsOrNull()` behaviour.
///
/// Input:  `[{"doc": {...}}, {"schema": {...}}]`
/// Output: `{"doc": {...}, "schema": {...}}`
fn flatten_facets_array(value: Option<serde_json::Value>) -> serde_json::Value {
    let Some(serde_json::Value::Array(arr)) = value else {
        return value.unwrap_or_else(|| serde_json::json!({}));
    };
    let mut merged = serde_json::Map::new();
    for item in arr {
        if let serde_json::Value::Object(obj) = item {
            merged.extend(obj);
        }
    }
    serde_json::Value::Object(merged)
}

fn map_dataset_row(row: SimpleDatasetRow) -> api::SimpleDataset {
    let ns = row.namespace_name.clone().unwrap_or_default();
    let type_ = row
        .type_
        .parse::<DatasetType>()
        .unwrap_or(DatasetType::DbTable);
    api::SimpleDataset {
        id: DatasetId {
            namespace: NamespaceName::new(&ns),
            name: DatasetName::new(&row.name),
        },
        type_,
        name: row.name,
        physical_name: row.physical_name,
        created_at: row.created_at,
        updated_at: row.updated_at,
        namespace: ns,
        source_name: row.source_name.unwrap_or_default(),
        description: row.description,
        current_version: row.current_version_uuid,
        last_modified_at: row.last_modified_at,
        last_lifecycle_state: row.lifecycle_state,
        is_deleted: row.is_deleted.unwrap_or(false),
        fields: row
            .fields
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        tags: row.tags.unwrap_or_default(),
        facets: flatten_facets_array(row.facets),
        is_current_version: row.is_current_version,
    }
}

fn map_job_row(row: SimpleJobRow) -> api::SimpleJob {
    let ns = row.namespace_name.clone().unwrap_or_default();
    let type_ = row.type_.parse::<JobType>().unwrap_or(JobType::Batch);
    api::SimpleJob {
        id: JobId {
            namespace: NamespaceName::new(&ns),
            name: JobName::new(&row.name),
        },
        type_,
        name: row.name,
        simple_name: row.simple_name,
        parent_job_name: row.parent_job_name,
        parent_job_uuid: row.parent_job_uuid,
        created_at: row.created_at,
        updated_at: row.updated_at,
        namespace: ns,
        description: row.description,
        current_version: row.current_version_uuid,
        location: row.location,
        tags: row.tags.unwrap_or_default(),
        labels: row.labels.unwrap_or_default(),
        facets: flatten_facets_array(row.facets),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimal_limits_both_empty() {
        let (ds, jobs) = calculate_optimal_limits(20, 0, 0);
        assert_eq!(ds, 0);
        assert_eq!(jobs, 20);
    }

    #[test]
    fn optimal_limits_only_datasets() {
        let (ds, jobs) = calculate_optimal_limits(20, 50, 0);
        assert_eq!(ds, 20);
        assert_eq!(jobs, 0);
    }

    #[test]
    fn optimal_limits_only_jobs() {
        let (ds, jobs) = calculate_optimal_limits(20, 0, 50);
        assert_eq!(ds, 0);
        assert_eq!(jobs, 20);
    }

    #[test]
    fn optimal_limits_even_split() {
        let (ds, jobs) = calculate_optimal_limits(20, 100, 100);
        assert_eq!(ds, 10);
        assert_eq!(jobs, 10);
    }

    #[test]
    fn optimal_limits_uneven_availability() {
        // limit=20, only 5 datasets available → 5 datasets, 15 jobs
        let (ds, jobs) = calculate_optimal_limits(20, 5, 100);
        assert_eq!(ds, 5);
        assert_eq!(jobs, 15);
    }

    #[test]
    fn optimal_limits_uneven_jobs_scarce() {
        // limit=20, only 3 jobs available → 17 datasets, 3 jobs
        let (ds, jobs) = calculate_optimal_limits(20, 100, 3);
        assert_eq!(ds, 17);
        assert_eq!(jobs, 3);
    }

    #[test]
    fn optimal_limits_odd_total() {
        // limit=21 → half=10, datasets get 10, jobs get 11
        let (ds, jobs) = calculate_optimal_limits(21, 100, 100);
        assert_eq!(ds, 10);
        assert_eq!(jobs, 11);
    }

    #[test]
    fn optimal_limits_both_scarce() {
        // limit=20, 3 datasets, 4 jobs → 3 datasets, 4 jobs (only 7 total, fewer than limit)
        let (ds, jobs) = calculate_optimal_limits(20, 3, 4);
        assert_eq!(ds, 3);
        assert_eq!(jobs, 17);
        // jobs capped at available in real usage, but limit distribution gives 17 to jobs
        // because the DB query itself enforces the cap
    }
}
