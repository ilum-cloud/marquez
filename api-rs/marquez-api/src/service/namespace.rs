use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api;

pub struct NamespaceService {
    pool: PgPool,
    /// Regex pattern for excluding namespaces on read (from config).
    exclusion_pattern: Option<String>,
}

impl NamespaceService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            exclusion_pattern: None,
        }
    }

    pub fn with_exclusion_pattern(mut self, pattern: Option<String>) -> Self {
        self.exclusion_pattern = pattern;
        self
    }

    pub async fn create_or_update(
        &self,
        name: &str,
        owner: &str,
        description: Option<&str>,
    ) -> Result<api::Namespace, AppError> {
        let row = db::namespace::upsert(
            &self.pool,
            Uuid::new_v4(),
            Utc::now(),
            name,
            owner,
            description,
        )
        .await?;
        db::namespace::upsert_owner(&self.pool, Uuid::new_v4(), Utc::now(), owner).await?;
        Ok(api::Namespace::from(row))
    }

    pub async fn get(&self, name: &str) -> Result<api::Namespace, AppError> {
        let row = db::namespace::find_by_name(&self.pool, name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Namespace '{}' not found", name)))?;
        Ok(api::Namespace::from(row))
    }

    pub async fn list(&self, limit: i32, offset: i32) -> Result<Vec<api::Namespace>, AppError> {
        let rows = if let Some(ref pattern) = self.exclusion_pattern {
            db::namespace::find_all_with_exclusion(&self.pool, pattern, limit, offset).await?
        } else {
            db::namespace::find_all(&self.pool, limit, offset).await?
        };
        Ok(rows.into_iter().map(api::Namespace::from).collect())
    }

    pub async fn delete(&self, name: &str) -> Result<(), AppError> {
        db::dataset::delete_by_namespace_name(&self.pool, name).await?;
        db::job::delete_by_namespace_name(&self.pool, name).await?;
        db::namespace::delete(&self.pool, name).await?;
        Ok(())
    }
}
