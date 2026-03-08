use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api;

pub struct SourceService {
    pool: PgPool,
}

impl SourceService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_or_update(
        &self,
        type_: &str,
        name: &str,
        connection_url: &str,
        description: Option<&str>,
    ) -> Result<api::Source, AppError> {
        let row = db::source::upsert(
            &self.pool,
            Uuid::new_v4(),
            type_,
            Utc::now(),
            name,
            connection_url,
            description,
        )
        .await?;
        Ok(api::Source::from(row))
    }

    pub async fn get(&self, name: &str) -> Result<api::Source, AppError> {
        let row = db::source::find_by_name(&self.pool, name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Source '{}' not found", name)))?;
        Ok(api::Source::from(row))
    }

    pub async fn list(&self, limit: i32, offset: i32) -> Result<Vec<api::Source>, AppError> {
        let rows = db::source::find_all(&self.pool, limit, offset).await?;
        Ok(rows.into_iter().map(api::Source::from).collect())
    }
}
