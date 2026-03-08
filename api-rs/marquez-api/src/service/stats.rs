use sqlx::PgPool;

use crate::db;
use crate::error::AppError;
use crate::models::db::{IntervalMetricRow, LineageMetricRow};

pub struct StatsService {
    pool: PgPool,
}

impl StatsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_last_day_metrics(&self) -> Result<Vec<LineageMetricRow>, AppError> {
        Ok(db::stats::get_last_day_metrics(&self.pool).await?)
    }

    pub async fn get_last_week_metrics(
        &self,
        timezone: &str,
    ) -> Result<Vec<LineageMetricRow>, AppError> {
        Ok(db::stats::get_last_week_metrics(&self.pool, timezone).await?)
    }

    pub async fn get_last_day_jobs(&self) -> Result<Vec<IntervalMetricRow>, AppError> {
        Ok(db::stats::get_last_day_jobs(&self.pool).await?)
    }

    pub async fn get_last_week_jobs(
        &self,
        timezone: &str,
    ) -> Result<Vec<IntervalMetricRow>, AppError> {
        Ok(db::stats::get_last_week_jobs(&self.pool, timezone).await?)
    }

    pub async fn get_last_day_datasets(&self) -> Result<Vec<IntervalMetricRow>, AppError> {
        Ok(db::stats::get_last_day_datasets(&self.pool).await?)
    }

    pub async fn get_last_week_datasets(
        &self,
        timezone: &str,
    ) -> Result<Vec<IntervalMetricRow>, AppError> {
        Ok(db::stats::get_last_week_datasets(&self.pool, timezone).await?)
    }

    pub async fn get_last_day_sources(&self) -> Result<Vec<IntervalMetricRow>, AppError> {
        Ok(db::stats::get_last_day_sources(&self.pool).await?)
    }

    pub async fn get_last_week_sources(
        &self,
        timezone: &str,
    ) -> Result<Vec<IntervalMetricRow>, AppError> {
        Ok(db::stats::get_last_week_sources(&self.pool, timezone).await?)
    }
}
