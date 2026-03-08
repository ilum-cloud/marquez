use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::AppError;
use crate::models::api;

pub struct TagService {
    pool: PgPool,
}

impl TagService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_or_update(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<api::Tag, AppError> {
        let row =
            db::tag::upsert(&self.pool, Uuid::new_v4(), Utc::now(), name, description).await?;
        Ok(api::Tag::from(row))
    }

    pub async fn list(&self, limit: i32, offset: i32) -> Result<Vec<api::Tag>, AppError> {
        let rows = db::tag::find_all(&self.pool, limit, offset).await?;
        Ok(rows.into_iter().map(api::Tag::from).collect())
    }
}
