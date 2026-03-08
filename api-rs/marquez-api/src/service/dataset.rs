use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api;
use crate::models::common;

pub struct DatasetService {
    pool: PgPool,
}

impl DatasetService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create or update a dataset.
    pub async fn create_or_update(
        &self,
        ns_name: &str,
        ds_name: &str,
        type_: &str,
        src_name: &str,
        physical_name: &str,
        description: Option<&str>,
        fields: &[common::Field],
        tags: &[String],
    ) -> Result<api::Dataset, AppError> {
        // Look up namespace and source
        let ns = db::namespace::find_by_name(&self.pool, ns_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Namespace '{}' not found", ns_name)))?;
        let src = db::source::find_by_name(&self.pool, src_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Source '{}' not found", src_name)))?;

        let uuid = Uuid::new_v4();
        let now = Utc::now();

        let row = db::dataset::upsert(
            &self.pool,
            uuid,
            type_,
            now,
            ns.uuid,
            ns_name,
            src.uuid,
            src_name,
            ds_name,
            physical_name,
            description,
            false,
        )
        .await?;

        // Create primary symlink so datasets_view works
        db::dataset_version::upsert_symlink(
            &self.pool, row.uuid, ds_name, ns.uuid, None, true, now,
        )
        .await?;

        // Upsert fields from the request body (matches Java PUT behaviour)
        let mut field_uuids = Vec::new();
        for field in fields {
            let field_row = db::dataset_field::upsert(
                &self.pool,
                Uuid::new_v4(),
                now,
                field.name.value(),
                field.type_.as_deref(),
                field.description.as_deref(),
                row.uuid,
            )
            .await?;
            field_uuids.push(field_row.uuid);

            // Upsert tags for this field
            for tag_name in &field.tags {
                let tag_row =
                    db::tag::upsert(&self.pool, Uuid::new_v4(), now, tag_name.value(), None)
                        .await?;
                db::dataset_field::update_tags(&self.pool, field_row.uuid, tag_row.uuid, now)
                    .await?;
            }
        }

        // Upsert dataset-level tags from the request body
        for tag_name in tags {
            let tag_row = db::tag::upsert(&self.pool, Uuid::new_v4(), now, tag_name, None).await?;
            db::dataset::update_tag_mapping(&self.pool, row.uuid, tag_row.uuid, now).await?;
        }

        // Create a dataset version row (matches Java PUT behavior)
        let version_uuid = Uuid::new_v4();
        let dv_uuid = Uuid::new_v4();

        let fields_json = if fields.is_empty() {
            None
        } else {
            Some(serde_json::to_value(fields).unwrap_or(serde_json::json!([])))
        };

        db::dataset_version::upsert(
            &self.pool,
            dv_uuid,
            now,
            row.uuid,
            version_uuid,
            None, // schema_version_uuid
            None, // run_uuid (no run for PUT-created datasets)
            fields_json,
            ns_name,
            ds_name,
            None, // lifecycle_state
        )
        .await?;

        // Map fields to the new dataset version
        if !field_uuids.is_empty() {
            db::dataset_field::update_field_mapping(&self.pool, dv_uuid, &field_uuids).await?;
        }

        // Update dataset's current_version_uuid
        db::dataset::update_version(&self.pool, row.uuid, dv_uuid, now).await?;

        // Re-fetch the row to get the updated current_version_uuid
        let row = db::dataset::find_dataset_as_row(&self.pool, ns_name, ds_name)
            .await?
            .unwrap_or(row);

        self.enrich_dataset(row).await
    }

    /// Get a dataset by namespace and name, enriched with fields and tags.
    pub async fn get(&self, ns_name: &str, ds_name: &str) -> Result<api::Dataset, AppError> {
        let row = db::dataset::find_dataset_as_row(&self.pool, ns_name, ds_name)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Dataset '{}/{}' not found", ns_name, ds_name))
            })?;
        self.enrich_dataset(row).await
    }

    /// Get a raw dataset row (for use by other services).
    pub async fn get_by(
        &self,
        ns_name: &str,
        ds_name: &str,
    ) -> Result<crate::models::db::DatasetRow, AppError> {
        db::dataset::find_dataset_as_row(&self.pool, ns_name, ds_name)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Dataset '{}/{}' not found", ns_name, ds_name))
            })
    }

    /// List datasets in a namespace, enriched.
    ///
    /// Uses batch queries (6-7 total) instead of per-row enrichment (~600
    /// queries for 100 datasets).
    pub async fn list(
        &self,
        ns_name: &str,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<api::Dataset>, AppError> {
        // Check namespace exists (matches Java's throwIfNotExists)
        if !db::namespace::exists(&self.pool, ns_name).await? {
            return Err(AppError::NotFound(format!(
                "Namespace '{}' not found",
                ns_name
            )));
        }
        let rows = db::dataset::find_all(&self.pool, ns_name, limit, offset).await?;
        if rows.is_empty() {
            return Ok(vec![]);
        }

        // Collect UUIDs for batch queries
        let ds_uuids: Vec<Uuid> = rows.iter().map(|r| r.uuid).collect();
        let version_uuids: Vec<Uuid> = rows.iter().filter_map(|r| r.current_version_uuid).collect();
        let source_uuids: Vec<Uuid> = rows.iter().filter_map(|r| r.source_uuid).collect();

        // Batch queries (5 queries total, not 5*N)
        let fields_map =
            db::dataset_field::find_by_version_uuids_with_tags(&self.pool, &version_uuids).await?;
        let lifecycle_map =
            db::dataset::find_lifecycle_states_batch(&self.pool, &version_uuids).await?;
        let source_map = db::dataset::find_source_names_batch(&self.pool, &source_uuids).await?;
        let facets_map = db::dataset::find_facets_batch(&self.pool, &version_uuids).await?;
        let tags_map = db::dataset::find_tags_batch(&self.pool, &ds_uuids).await?;
        let col_lineage_map = self.get_column_lineage_batch(&version_uuids).await?;

        // Assemble results from HashMaps
        let mut datasets = Vec::with_capacity(rows.len());
        for row in rows {
            let mut dataset = api::Dataset::from(row.clone());

            // Fields from batch
            if let Some(field_rows) = fields_map.get(&row.uuid) {
                dataset.fields = field_rows
                    .iter()
                    .map(|f| common::Field::from(f.clone()))
                    .collect();
            }

            if let Some(version_uuid) = row.current_version_uuid {
                // Lifecycle state from batch
                if let Some(lifecycle) = lifecycle_map.get(&version_uuid) {
                    dataset.last_lifecycle_state = lifecycle.clone();
                }

                // Facets from batch — merge array into flat object
                if let Some(facets_arr) = facets_map.get(&version_uuid) {
                    if let Some(arr) = facets_arr.as_array() {
                        let mut merged = serde_json::Map::new();
                        for facet in arr {
                            if let Some(obj) = facet.as_object() {
                                for (k, v) in obj {
                                    merged.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        if !merged.is_empty() {
                            dataset.facets = serde_json::Value::Object(merged);
                        }
                    }
                }
            }

            // Source name from batch
            if let Some(src_uuid) = row.source_uuid {
                if let Some(src_name) = source_map.get(&src_uuid) {
                    dataset.source_name = src_name.clone();
                }
            }

            // Tags from batch
            if let Some(tags) = tags_map.get(&row.uuid) {
                dataset.tags = tags.clone();
            }

            // Column lineage from batch
            if let Some(cl) = col_lineage_map.get(&row.uuid) {
                dataset.column_lineage = cl.clone();
            }

            datasets.push(dataset);
        }
        Ok(datasets)
    }

    /// Count datasets in a namespace.
    pub async fn count(&self, ns_name: &str) -> Result<i64, AppError> {
        Ok(db::dataset::count(&self.pool, ns_name).await?)
    }

    /// Soft-delete a dataset.
    pub async fn delete(&self, ns_name: &str, ds_name: &str) -> Result<(), AppError> {
        db::dataset::delete(&self.pool, ns_name, ds_name).await?;
        Ok(())
    }

    /// Tag a dataset.
    pub async fn tag_dataset(
        &self,
        ns_name: &str,
        ds_name: &str,
        tag_name: &str,
    ) -> Result<api::Dataset, AppError> {
        let ds_row = self.get_by(ns_name, ds_name).await?;
        let tag_row = db::tag::find_by_name(&self.pool, tag_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tag '{}' not found", tag_name)))?;

        db::dataset::update_tag_mapping(&self.pool, ds_row.uuid, tag_row.uuid, Utc::now()).await?;
        self.get(ns_name, ds_name).await
    }

    /// Delete a tag from a dataset.
    pub async fn delete_tag(
        &self,
        ns_name: &str,
        ds_name: &str,
        tag_name: &str,
    ) -> Result<api::Dataset, AppError> {
        db::dataset::delete_dataset_tag(&self.pool, ns_name, ds_name, tag_name).await?;
        self.get(ns_name, ds_name).await
    }

    /// Tag a specific field on a dataset.
    pub async fn tag_field(
        &self,
        ns_name: &str,
        ds_name: &str,
        field_name: &str,
        tag_name: &str,
    ) -> Result<api::Dataset, AppError> {
        let ds_row = self.get_by(ns_name, ds_name).await?;
        let field_uuid = db::dataset_field::find_uuid(&self.pool, ds_row.uuid, field_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Field '{}' not found", field_name)))?;
        // Auto-create tag if it doesn't exist (matches Java behavior)
        let tag_row =
            db::tag::upsert(&self.pool, Uuid::new_v4(), Utc::now(), tag_name, None).await?;

        db::dataset_field::update_tags(&self.pool, field_uuid, tag_row.uuid, Utc::now()).await?;
        // Also update dataset_versions fields JSONB (matches Java behavior)
        db::dataset_field::update_dataset_version_field_tag(
            &self.pool, ns_name, ds_name, field_name, tag_name,
        )
        .await?;
        self.get(ns_name, ds_name).await
    }

    /// Remove a tag from a specific field on a dataset.
    pub async fn untag_field(
        &self,
        ns_name: &str,
        ds_name: &str,
        field_name: &str,
        tag_name: &str,
    ) -> Result<api::Dataset, AppError> {
        let ds_row = self.get_by(ns_name, ds_name).await?;
        let field_uuid = db::dataset_field::find_uuid(&self.pool, ds_row.uuid, field_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Field '{}' not found", field_name)))?;
        let tag_row = db::tag::find_by_name(&self.pool, tag_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tag '{}' not found", tag_name)))?;

        db::dataset_field::delete_tag(&self.pool, field_uuid, tag_row.uuid).await?;
        // Also remove from dataset_versions fields JSONB (matches Java behavior)
        db::dataset_field::delete_dataset_version_field_tag(
            &self.pool, ns_name, ds_name, field_name, tag_name,
        )
        .await?;
        self.get(ns_name, ds_name).await
    }

    /// Get a specific dataset version.
    pub async fn get_version(
        &self,
        ns_name: &str,
        ds_name: &str,
        version: Uuid,
    ) -> Result<api::DatasetVersion, AppError> {
        let row = db::dataset_version::find_by_uuid(&self.pool, version)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Dataset version '{}' not found", version))
            })?;
        self.enrich_dataset_version(row, ns_name, ds_name).await
    }

    /// List dataset versions (enriched).
    ///
    /// Batch-fetches fields and runs to avoid N+1 queries.
    pub async fn list_versions(
        &self,
        ns_name: &str,
        ds_name: &str,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<api::DatasetVersion>, AppError> {
        let enriched_rows =
            db::dataset_version::find_all(&self.pool, ns_name, ds_name, limit, offset).await?;

        if enriched_rows.is_empty() {
            return Ok(vec![]);
        }

        // Batch-fetch fields by schema version UUIDs (avoids N+1).
        let schema_version_uuids: Vec<Uuid> = enriched_rows
            .iter()
            .filter_map(|r| r.dataset_schema_version_uuid)
            .collect();
        let fields_by_schema = db::dataset_field::find_by_dataset_schema_versions_batch(
            &self.pool,
            &schema_version_uuids,
        )
        .await?;

        // Batch-fetch runs by run UUIDs (avoids N+1).
        let run_uuids: Vec<Uuid> = enriched_rows
            .iter()
            .filter_map(|r| r.created_by_run_uuid)
            .collect();
        let run_rows = db::run::find_runs_by_uuids_with_facets(&self.pool, &run_uuids).await?;
        let runs_by_uuid: std::collections::HashMap<Uuid, _> =
            run_rows.into_iter().map(|r| (r.uuid, r)).collect();

        let mut versions = Vec::with_capacity(enriched_rows.len());
        for row in enriched_rows {
            versions.push(self.enrich_dataset_version_from_enriched_batch(
                row,
                &fields_by_schema,
                &runs_by_uuid,
            ));
        }
        Ok(versions)
    }

    /// Count dataset versions.
    pub async fn count_versions(&self, ns_name: &str, ds_name: &str) -> Result<i64, AppError> {
        Ok(db::dataset_version::count(&self.pool, ns_name, ds_name).await?)
    }

    // ---- Private enrichment helpers ----

    /// Enrich a DatasetRow into an api::Dataset with fields, tags, facets,
    /// lifecycle state, and column lineage.
    async fn enrich_dataset(
        &self,
        row: crate::models::db::DatasetRow,
    ) -> Result<api::Dataset, AppError> {
        let mut dataset = api::Dataset::from(row.clone());

        // Enrich with fields — query through dataset_versions_field_mapping
        // for the current version to show only current-version fields (not stale ones).
        let field_rows = if let Some(version_uuid) = row.current_version_uuid {
            db::dataset_field::find_by_version_with_tags(&self.pool, version_uuid).await?
        } else {
            vec![]
        };
        dataset.fields = field_rows.into_iter().map(common::Field::from).collect();

        if let Some(version_uuid) = row.current_version_uuid {
            // Get lifecycle state from the current version
            let lifecycle: Option<(Option<String>,)> =
                sqlx::query_as("SELECT lifecycle_state FROM dataset_versions WHERE uuid = $1")
                    .bind(version_uuid)
                    .fetch_optional(&self.pool)
                    .await?;
            dataset.last_lifecycle_state = lifecycle.and_then(|(s,)| s);

            // Get source name from the dataset's source
            if let Some(src_uuid) = row.source_uuid {
                let src_row: Option<(String,)> =
                    sqlx::query_as("SELECT name FROM sources WHERE uuid = $1")
                        .bind(src_uuid)
                        .fetch_optional(&self.pool)
                        .await?;
                if let Some((src_name,)) = src_row {
                    dataset.source_name = src_name;
                }
            }

            // Get facets from dataset_facets — match Java's JSONB_AGG + MapperUtils
            // merge approach: aggregate all facets ordered by lineage_event_time ASC,
            // then flatten with last-wins semantics.
            let facet_rows: Vec<(serde_json::Value,)> = sqlx::query_as(
                "SELECT df.facet FROM dataset_facets df \
                 WHERE df.dataset_version_uuid = $1 \
                   AND (LOWER(df.type) IN ('dataset', 'unknown', 'input')) \
                   AND df.facet IS NOT NULL \
                 ORDER BY df.lineage_event_time ASC",
            )
            .bind(version_uuid)
            .fetch_all(&self.pool)
            .await?;
            if !facet_rows.is_empty() {
                let mut merged = serde_json::Map::new();
                for (facet,) in &facet_rows {
                    if let Some(obj) = facet.as_object() {
                        for (k, v) in obj {
                            merged.insert(k.clone(), v.clone());
                        }
                    }
                }
                if !merged.is_empty() {
                    dataset.facets = serde_json::Value::Object(merged);
                }
            }

            // Get column lineage for this dataset version
            let col_lineage = self.get_column_lineage(version_uuid).await?;
            dataset.column_lineage = col_lineage;
        }

        // Tags are fetched via datasets_tag_mapping
        let tag_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT t.name FROM tags t \
             INNER JOIN datasets_tag_mapping dtm ON dtm.tag_uuid = t.uuid \
             WHERE dtm.dataset_uuid = $1 \
             ORDER BY t.name",
        )
        .bind(row.uuid)
        .fetch_all(&self.pool)
        .await?;
        dataset.tags = tag_rows.into_iter().map(|(name,)| name).collect();

        Ok(dataset)
    }

    /// Get column lineage for a dataset version.
    /// Returns null if no column lineage exists, or an array of column lineage nodes.
    #[allow(clippy::type_complexity)]
    async fn get_column_lineage(
        &self,
        dataset_version_uuid: Uuid,
    ) -> Result<serde_json::Value, AppError> {
        let rows: Vec<(
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT DISTINCT \
                df.name AS field_name, \
                cl.transformation_description, \
                cl.transformation_type, \
                idf.name AS input_field_name, \
                id.name AS input_dataset_name, \
                id.namespace_name AS input_namespace \
             FROM dataset_versions_field_mapping fm \
             JOIN dataset_fields df ON df.uuid = fm.dataset_field_uuid \
             JOIN column_lineage cl ON cl.output_dataset_field_uuid = df.uuid \
             LEFT JOIN dataset_fields idf ON idf.uuid = cl.input_dataset_field_uuid \
             LEFT JOIN datasets id ON id.uuid = idf.dataset_uuid \
             WHERE fm.dataset_version_uuid = $1 \
             ORDER BY df.name",
        )
        .bind(dataset_version_uuid)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(serde_json::Value::Null);
        }

        // Group by field_name
        let mut grouped: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();
        let mut descriptions: std::collections::HashMap<String, (Option<String>, Option<String>)> =
            std::collections::HashMap::new();

        for (field_name, desc, trans_type, input_field, input_dataset, input_ns) in &rows {
            descriptions
                .entry(field_name.clone())
                .or_insert_with(|| (desc.clone(), trans_type.clone()));
            if let Some(input_field_name) = input_field {
                let entry = grouped.entry(field_name.clone()).or_default();
                entry.push(serde_json::json!({
                    "namespace": input_ns.as_deref().unwrap_or_default(),
                    "dataset": input_dataset.as_deref().unwrap_or_default(),
                    "field": input_field_name,
                    "transformationDescription": descriptions[field_name].0,
                    "transformationType": descriptions[field_name].1,
                }));
            }
        }

        // Collect all unique field names
        let mut all_fields: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (field_name, _, _, _, _, _) in &rows {
            all_fields.insert(field_name.clone());
        }

        let result: Vec<serde_json::Value> = all_fields
            .iter()
            .map(|field_name| {
                let (desc, trans_type) = descriptions
                    .get(field_name)
                    .cloned()
                    .unwrap_or((None, None));
                let inputs = grouped.get(field_name).cloned().unwrap_or_default();
                serde_json::json!({
                    "name": field_name,
                    "inputFields": inputs,
                    "outputFields": [],
                    "transformationDescription": desc,
                    "transformationType": trans_type,
                })
            })
            .collect();

        Ok(serde_json::json!(result))
    }

    /// Batch-fetch column lineage for multiple dataset versions at once.
    ///
    /// Returns a map from dataset_uuid → column lineage JSON (same structure
    /// as `get_column_lineage()`). Used by `list()` to avoid per-dataset queries.
    #[allow(clippy::type_complexity)]
    async fn get_column_lineage_batch(
        &self,
        dataset_version_uuids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, serde_json::Value>, AppError> {
        let rows: Vec<(
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT DISTINCT \
                df.dataset_uuid, \
                df.name AS field_name, \
                cl.transformation_description, \
                cl.transformation_type, \
                idf.name AS input_field_name, \
                id.name AS input_dataset_name, \
                id.namespace_name AS input_namespace \
             FROM dataset_versions_field_mapping fm \
             JOIN dataset_fields df ON df.uuid = fm.dataset_field_uuid \
             JOIN column_lineage cl ON cl.output_dataset_field_uuid = df.uuid \
             LEFT JOIN dataset_fields idf ON idf.uuid = cl.input_dataset_field_uuid \
             LEFT JOIN datasets id ON id.uuid = idf.dataset_uuid \
             WHERE fm.dataset_version_uuid = ANY($1) \
             ORDER BY df.dataset_uuid, df.name",
        )
        .bind(dataset_version_uuids)
        .fetch_all(&self.pool)
        .await?;

        // Group rows by dataset_uuid, then build the same JSON structure as get_column_lineage()
        let mut by_dataset: std::collections::HashMap<
            Uuid,
            Vec<(
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            )>,
        > = std::collections::HashMap::new();
        for (ds_uuid, field_name, desc, trans_type, input_field, input_dataset, input_ns) in rows {
            by_dataset.entry(ds_uuid).or_default().push((
                field_name,
                desc,
                trans_type,
                input_field,
                input_dataset,
                input_ns,
            ));
        }

        let mut result_map = std::collections::HashMap::new();
        for (ds_uuid, ds_rows) in by_dataset {
            let mut grouped: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
                std::collections::BTreeMap::new();
            let mut descriptions: std::collections::HashMap<
                String,
                (Option<String>, Option<String>),
            > = std::collections::HashMap::new();

            for (field_name, desc, trans_type, input_field, input_dataset, input_ns) in &ds_rows {
                descriptions
                    .entry(field_name.clone())
                    .or_insert_with(|| (desc.clone(), trans_type.clone()));
                if let Some(input_field_name) = input_field {
                    let entry = grouped.entry(field_name.clone()).or_default();
                    entry.push(serde_json::json!({
                        "namespace": input_ns.as_deref().unwrap_or_default(),
                        "dataset": input_dataset.as_deref().unwrap_or_default(),
                        "field": input_field_name,
                        "transformationDescription": descriptions[field_name].0,
                        "transformationType": descriptions[field_name].1,
                    }));
                }
            }

            let mut all_fields: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            for (field_name, _, _, _, _, _) in &ds_rows {
                all_fields.insert(field_name.clone());
            }

            let lineage: Vec<serde_json::Value> = all_fields
                .iter()
                .map(|field_name| {
                    let (desc, trans_type) = descriptions
                        .get(field_name)
                        .cloned()
                        .unwrap_or((None, None));
                    let inputs = grouped.get(field_name).cloned().unwrap_or_default();
                    serde_json::json!({
                        "name": field_name,
                        "inputFields": inputs,
                        "outputFields": [],
                        "transformationDescription": desc,
                        "transformationType": trans_type,
                    })
                })
                .collect();

            result_map.insert(ds_uuid, serde_json::json!(lineage));
        }

        Ok(result_map)
    }

    /// Enrich a DatasetVersionRow into an api::DatasetVersion.
    async fn enrich_dataset_version(
        &self,
        row: crate::models::db::DatasetVersionRow,
        _ns_name: &str,
        _ds_name: &str,
    ) -> Result<api::DatasetVersion, AppError> {
        let mut version = api::DatasetVersion::from(row.clone());

        // Enrich with fields — prefer schema version mapping (includes tags),
        // fall back to version field mapping for older data.
        if let Some(schema_version_uuid) = row.dataset_schema_version_uuid {
            let field_rows =
                db::dataset_field::find_by_dataset_schema_version(&self.pool, schema_version_uuid)
                    .await?;
            version.fields = field_rows.into_iter().map(common::Field::from).collect();
        } else {
            let field_rows =
                db::dataset_field::find_by_dataset_version(&self.pool, row.uuid).await?;
            version.fields = field_rows.into_iter().map(common::Field::from).collect();
        }

        // Enrich with source name from the dataset
        if let Some(ds_uuid) = row.dataset_uuid {
            let src_row: Option<(String,)> = sqlx::query_as(
                "SELECT s.name FROM sources s \
                 JOIN datasets d ON d.source_uuid = s.uuid \
                 WHERE d.uuid = $1",
            )
            .bind(ds_uuid)
            .fetch_optional(&self.pool)
            .await?;
            if let Some((src_name,)) = src_row {
                version.source_name = src_name;
            }

            // Get the dataset type and physical_name
            let ds_info: Option<(String, String)> =
                sqlx::query_as("SELECT type, physical_name FROM datasets WHERE uuid = $1")
                    .bind(ds_uuid)
                    .fetch_optional(&self.pool)
                    .await?;
            if let Some((type_str, physical_name)) = ds_info {
                version.type_ = type_str.parse().unwrap_or(common::DatasetType::DbTable);
                version.physical_name = physical_name;
            }
        }

        // Enrich with createdByRun (single optimized query)
        if let Some(run_uuid) = row.run_uuid {
            if let Some(enriched) =
                db::run::find_run_by_uuid_with_facets(&self.pool, run_uuid).await?
            {
                version.run = Some(Box::new(crate::service::run::map_extended_run_to_api(
                    &enriched,
                )));
            }
        }

        // Enrich with facets from dataset_facets — match Java's JSONB_AGG + merge
        let facet_rows: Vec<(serde_json::Value,)> = sqlx::query_as(
            "SELECT df.facet FROM dataset_facets df \
             WHERE df.dataset_version_uuid = $1 \
               AND (LOWER(df.type) IN ('dataset', 'unknown', 'input')) \
               AND df.facet IS NOT NULL \
             ORDER BY df.lineage_event_time ASC",
        )
        .bind(row.uuid)
        .fetch_all(&self.pool)
        .await?;
        if !facet_rows.is_empty() {
            let mut merged = serde_json::Map::new();
            for (facet,) in &facet_rows {
                if let Some(obj) = facet.as_object() {
                    for (k, v) in obj {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
            if !merged.is_empty() {
                version.facets = serde_json::Value::Object(merged);
            }
        }

        // Enrich with tags
        if let Some(ds_uuid) = row.dataset_uuid {
            let tag_rows: Vec<(String,)> = sqlx::query_as(
                "SELECT t.name FROM tags t \
                 INNER JOIN datasets_tag_mapping dtm ON dtm.tag_uuid = t.uuid \
                 WHERE dtm.dataset_uuid = $1 \
                 ORDER BY t.name",
            )
            .bind(ds_uuid)
            .fetch_all(&self.pool)
            .await?;
            version.tags = tag_rows.into_iter().map(|(name,)| name).collect();
        }

        Ok(version)
    }

    /// Build an api::DatasetVersion from an EnrichedDatasetVersionRow using
    /// pre-fetched fields and runs (batch variant — no per-item DB calls).
    fn enrich_dataset_version_from_enriched_batch(
        &self,
        row: crate::models::db::EnrichedDatasetVersionRow,
        fields_by_schema: &std::collections::HashMap<
            Uuid,
            Vec<crate::models::db::DatasetFieldWithTagsRow>,
        >,
        runs_by_uuid: &std::collections::HashMap<Uuid, crate::models::db::ExtendedRunWithFacetsRow>,
    ) -> api::DatasetVersion {
        let ns = row.namespace_name.clone().unwrap_or_default();
        let name = row.name.clone().unwrap_or_default();

        let mut version = api::DatasetVersion {
            id: crate::models::common::DatasetVersionId {
                namespace: crate::models::common::NamespaceName::new(&ns),
                name: crate::models::common::DatasetName::new(&name),
                version: row.version,
            },
            type_: row
                .type_
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(crate::models::common::DatasetType::DbTable),
            name: name.clone(),
            physical_name: row.physical_name.clone().unwrap_or_default(),
            created_at: row.created_at,
            version: row.version,
            namespace: ns,
            source_name: row.source_name.clone().unwrap_or_default(),
            fields: vec![],
            tags: row.tags.unwrap_or_default(),
            last_modified_at: None,
            description: row.description.clone(),
            current_schema_version: row.dataset_schema_version_uuid,
            lifecycle_state: row.lifecycle_state.clone(),
            run: None,
            facets: serde_json::json!({}),
        };

        // Merge facets from the JSONB_AGG
        if let Some(facets_arr) = &row.facets {
            if let Some(arr) = facets_arr.as_array() {
                let mut merged = serde_json::Map::new();
                for facet in arr {
                    if let Some(obj) = facet.as_object() {
                        for (k, v) in obj {
                            merged.insert(k.clone(), v.clone());
                        }
                    }
                }
                if !merged.is_empty() {
                    version.facets = serde_json::Value::Object(merged);
                }
            }
        }

        // Enrich with fields from pre-fetched batch
        if let Some(schema_version_uuid) = row.dataset_schema_version_uuid {
            if let Some(field_rows) = fields_by_schema.get(&schema_version_uuid) {
                version.fields = field_rows
                    .iter()
                    .cloned()
                    .map(common::Field::from)
                    .collect();
            }
        }

        // Enrich with createdByRun from pre-fetched batch
        if let Some(run_uuid) = row.created_by_run_uuid {
            if let Some(enriched) = runs_by_uuid.get(&run_uuid) {
                version.run = Some(Box::new(crate::service::run::map_extended_run_to_api(
                    enriched,
                )));
            }
        }

        version
    }
}
