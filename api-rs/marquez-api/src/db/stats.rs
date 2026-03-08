// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

//! DAO functions for dashboard metrics and statistics.
//!
//! Provides hourly (DAY) and daily (WEEK) aggregations of lineage events, jobs,
//! datasets, and sources. Uses `generate_series()` to always return a complete
//! time series (24 rows for DAY, 7 for WEEK) with zero-filled intervals.

use sqlx::PgPool;

use crate::models::db::{IntervalMetricRow, LineageMetricRow};

// ---------------------------------------------------------------------------
// Lineage event metrics
// ---------------------------------------------------------------------------

/// Get last 24 hours of lineage event metrics, aggregated hourly.
///
/// Always returns 24 rows. Uses `generate_series` for the time axis,
/// LEFT JOINs the materialized view for past hours, and queries
/// `lineage_events` directly for the current (potentially incomplete) hour.
pub async fn get_last_day_metrics(pool: &PgPool) -> Result<Vec<LineageMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, LineageMetricRow>(
        "WITH hour_series AS ( \
             SELECT generate_series( \
                        DATE_TRUNC('hour', NOW() - INTERVAL '23 hours'), \
                        DATE_TRUNC('hour', NOW()), \
                        '1 hour' \
                    ) AS start_interval) \
         SELECT hs.start_interval, \
                hs.start_interval + INTERVAL '1 hour'                       AS end_interval, \
                COALESCE(current_hour.fail, hourly_metrics.fail, 0)         AS fail, \
                COALESCE(current_hour.start, hourly_metrics.start, 0)       AS start, \
                COALESCE(current_hour.complete, hourly_metrics.complete, 0) AS complete, \
                COALESCE(current_hour.abort, hourly_metrics.abort, 0)       AS abort \
         FROM hour_series hs \
              LEFT JOIN lineage_events_by_type_hourly_view hourly_metrics \
                  ON hs.start_interval = hourly_metrics.start_interval \
              LEFT JOIN ( \
                  SELECT DATE_TRUNC('hour', NOW()) AS start_interval, \
                         DATE_TRUNC('hour', NOW()) + INTERVAL '1 hour' AS end_interval, \
                         COALESCE(SUM(CASE WHEN event_type = 'FAIL' THEN 1 ELSE 0 END), 0)     AS fail, \
                         COALESCE(SUM(CASE WHEN event_type = 'START' THEN 1 ELSE 0 END), 0)    AS start, \
                         COALESCE(SUM(CASE WHEN event_type = 'COMPLETE' THEN 1 ELSE 0 END), 0) AS complete, \
                         COALESCE(SUM(CASE WHEN event_type = 'ABORT' THEN 1 ELSE 0 END), 0)    AS abort \
                  FROM lineage_events \
                  WHERE event_time >= DATE_TRUNC('hour', NOW()) \
                    AND event_time < DATE_TRUNC('hour', NOW()) + INTERVAL '1 hour' \
              ) current_hour ON current_hour.start_interval = hs.start_interval \
         ORDER BY hs.start_interval",
    )
    .fetch_all(pool)
    .await
}

/// Get last 7 days of lineage event metrics, aggregated daily.
///
/// Always returns 7 rows. Uses the materialized view for completed days
/// and queries `lineage_events` directly for today.
pub async fn get_last_week_metrics(
    pool: &PgPool,
    timezone: &str,
) -> Result<Vec<LineageMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, LineageMetricRow>(
        "WITH date_bounds AS ( \
             SELECT DATE_TRUNC('day', NOW() AT TIME ZONE $1) - INTERVAL '6 days' AS start_date, \
                    DATE_TRUNC('day', NOW() AT TIME ZONE $1) + INTERVAL '1 day'  AS end_date), \
         day_series AS ( \
             SELECT start_date + INTERVAL '1 day' * n AS day \
             FROM date_bounds, generate_series(0, 6) AS n), \
         metrics AS ( \
             SELECT DATE_TRUNC('day', mv.start_interval AT TIME ZONE $1) AS day, \
                    SUM(mv.fail)     AS fail, \
                    SUM(mv.start)    AS start, \
                    SUM(mv.complete) AS complete, \
                    SUM(mv.abort)    AS abort \
             FROM lineage_events_by_type_hourly_view AS mv, date_bounds \
             WHERE mv.start_interval >= (start_date AT TIME ZONE $1) \
               AND mv.start_interval < (end_date AT TIME ZONE $1) \
             GROUP BY day) \
         SELECT ds.day::TIMESTAMPTZ                                AS start_interval, \
                (ds.day + INTERVAL '1 day')::TIMESTAMPTZ             AS end_interval, \
                COALESCE(today.fail, m.fail, 0)::BIGINT              AS fail, \
                COALESCE(today.start, m.start, 0)::BIGINT            AS start, \
                COALESCE(today.complete, m.complete, 0)::BIGINT      AS complete, \
                COALESCE(today.abort, m.abort, 0)::BIGINT            AS abort \
         FROM day_series ds \
              LEFT JOIN metrics m ON m.day = ds.day \
              LEFT OUTER JOIN ( \
                  WITH local_now AS (SELECT DATE_TRUNC('day', NOW() AT TIME ZONE $1) AS time) \
                  SELECT local_now.time AS start_interval, \
                         local_now.time + INTERVAL '1 day' AS end_interval, \
                         COALESCE(SUM(CASE WHEN event_type = 'FAIL' THEN 1 END), 0)     AS fail, \
                         COALESCE(SUM(CASE WHEN event_type = 'START' THEN 1 END), 0)    AS start, \
                         COALESCE(SUM(CASE WHEN event_type = 'COMPLETE' THEN 1 END), 0) AS complete, \
                         COALESCE(SUM(CASE WHEN event_type = 'ABORT' THEN 1 END), 0)    AS abort \
                  FROM lineage_events le, local_now \
                  WHERE (le.event_time AT TIME ZONE $1) >= local_now.time \
                    AND (le.event_time AT TIME ZONE $1) < local_now.time + INTERVAL '1 day' \
                  GROUP BY local_now.time \
              ) AS today ON ds.day = today.start_interval \
         ORDER BY ds.day",
    )
    .bind(timezone)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Job metrics (cumulative counts)
// ---------------------------------------------------------------------------

/// Get last 24 hours of cumulative job counts, aggregated hourly.
///
/// Always returns 24 rows with a running total that includes jobs
/// created before the 24-hour window.
pub async fn get_last_day_jobs(pool: &PgPool) -> Result<Vec<IntervalMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, IntervalMetricRow>(
        "WITH hourly_series AS ( \
             SELECT generate_series( \
                        DATE_TRUNC('hour', NOW() - INTERVAL '23 hours'), \
                        DATE_TRUNC('hour', NOW()), \
                        '1 hour' \
                    ) AS start_interval), \
         before_count AS ( \
             SELECT COUNT(*) AS job_count \
             FROM jobs \
             WHERE created_at < DATE_TRUNC('hour', NOW() - INTERVAL '23 hours')), \
         hourly_jobs AS ( \
             SELECT hs.start_interval, \
                    COUNT(j.uuid) AS jobs_in_hour \
             FROM hourly_series hs \
                  LEFT JOIN jobs j \
                      ON j.created_at >= hs.start_interval \
                     AND j.created_at < hs.start_interval + INTERVAL '1 hour' \
             GROUP BY hs.start_interval), \
         cumulative_jobs AS ( \
             SELECT start_interval, \
                    SUM(jobs_in_hour) OVER (ORDER BY start_interval) \
                        + (SELECT job_count FROM before_count) AS cumulative_job_count \
             FROM hourly_jobs) \
         SELECT start_interval, \
                start_interval + INTERVAL '1 hour' AS end_interval, \
                cumulative_job_count::BIGINT AS count \
         FROM cumulative_jobs \
         ORDER BY start_interval",
    )
    .fetch_all(pool)
    .await
}

/// Get last 7 days of cumulative job counts, aggregated daily.
pub async fn get_last_week_jobs(
    pool: &PgPool,
    timezone: &str,
) -> Result<Vec<IntervalMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, IntervalMetricRow>(
        "WITH local_now AS ( \
             SELECT (NOW() AT TIME ZONE $1) AS local_now), \
         daily_series AS ( \
             SELECT generate_series( \
                        DATE_TRUNC('day', ln.local_now - INTERVAL '6 days'), \
                        DATE_TRUNC('day', ln.local_now), \
                        '1 day' \
                    ) AS start_interval \
             FROM local_now ln), \
         before_count AS ( \
             SELECT COUNT(*) AS job_count \
             FROM jobs, local_now ln \
             WHERE (jobs.created_at AT TIME ZONE $1) < DATE_TRUNC('day', ln.local_now - INTERVAL '6 days')), \
         daily_jobs AS ( \
             SELECT ds.start_interval, \
                    COUNT(j.uuid) AS jobs_in_day \
             FROM daily_series ds \
                  LEFT JOIN jobs j \
                      ON (j.created_at AT TIME ZONE $1) >= ds.start_interval \
                     AND (j.created_at AT TIME ZONE $1) < ds.start_interval + INTERVAL '1 day' \
             GROUP BY ds.start_interval), \
         cumulative_jobs AS ( \
             SELECT start_interval, \
                    SUM(jobs_in_day) OVER (ORDER BY start_interval) \
                        + (SELECT job_count FROM before_count) AS cumulative_job_count \
             FROM daily_jobs) \
         SELECT start_interval::TIMESTAMPTZ, \
                (start_interval + INTERVAL '1 day')::TIMESTAMPTZ AS end_interval, \
                cumulative_job_count::BIGINT AS count \
         FROM cumulative_jobs \
         ORDER BY start_interval",
    )
    .bind(timezone)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Dataset metrics (cumulative counts)
// ---------------------------------------------------------------------------

/// Get last 24 hours of cumulative dataset counts, aggregated hourly.
pub async fn get_last_day_datasets(pool: &PgPool) -> Result<Vec<IntervalMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, IntervalMetricRow>(
        "WITH hourly_series AS ( \
             SELECT generate_series( \
                        DATE_TRUNC('hour', NOW() - INTERVAL '23 hours'), \
                        DATE_TRUNC('hour', NOW()), \
                        '1 hour' \
                    ) AS start_interval), \
         before_count AS ( \
             SELECT COUNT(*) AS dataset_count \
             FROM datasets \
             WHERE created_at < DATE_TRUNC('hour', NOW() - INTERVAL '23 hours')), \
         hourly_datasets AS ( \
             SELECT hs.start_interval, \
                    COUNT(d.uuid) AS datasets_in_hour \
             FROM hourly_series hs \
                  LEFT JOIN datasets d \
                      ON d.created_at >= hs.start_interval \
                     AND d.created_at < hs.start_interval + INTERVAL '1 hour' \
             GROUP BY hs.start_interval), \
         cumulative_datasets AS ( \
             SELECT start_interval, \
                    SUM(datasets_in_hour) OVER (ORDER BY start_interval) \
                        + (SELECT dataset_count FROM before_count) AS cumulative_dataset_count \
             FROM hourly_datasets) \
         SELECT start_interval, \
                start_interval + INTERVAL '1 hour' AS end_interval, \
                cumulative_dataset_count::BIGINT AS count \
         FROM cumulative_datasets \
         ORDER BY start_interval",
    )
    .fetch_all(pool)
    .await
}

/// Get last 7 days of cumulative dataset counts, aggregated daily.
pub async fn get_last_week_datasets(
    pool: &PgPool,
    timezone: &str,
) -> Result<Vec<IntervalMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, IntervalMetricRow>(
        "WITH local_now AS ( \
             SELECT (NOW() AT TIME ZONE $1) AS local_now), \
         daily_series AS ( \
             SELECT generate_series( \
                        DATE_TRUNC('day', ln.local_now - INTERVAL '6 days'), \
                        DATE_TRUNC('day', ln.local_now), \
                        '1 day' \
                    ) AS start_interval \
             FROM local_now ln), \
         before_count AS ( \
             SELECT COUNT(*) AS dataset_count \
             FROM datasets d CROSS JOIN local_now ln \
             WHERE (d.created_at AT TIME ZONE $1) < DATE_TRUNC('day', ln.local_now - INTERVAL '6 days')), \
         daily_datasets AS ( \
             SELECT ds.start_interval, \
                    COUNT(d.uuid) AS datasets_in_day \
             FROM daily_series ds \
                  LEFT JOIN datasets d \
                      ON (d.created_at AT TIME ZONE $1) >= ds.start_interval \
                     AND (d.created_at AT TIME ZONE $1) < ds.start_interval + INTERVAL '1 day' \
             GROUP BY ds.start_interval), \
         cumulative_datasets AS ( \
             SELECT start_interval, \
                    SUM(datasets_in_day) OVER (ORDER BY start_interval) \
                        + (SELECT dataset_count FROM before_count) AS cumulative_dataset_count \
             FROM daily_datasets) \
         SELECT start_interval::TIMESTAMPTZ, \
                (start_interval + INTERVAL '1 day')::TIMESTAMPTZ AS end_interval, \
                cumulative_dataset_count::BIGINT AS count \
         FROM cumulative_datasets \
         ORDER BY start_interval",
    )
    .bind(timezone)
    .fetch_all(pool)
    .await
}

// ---------------------------------------------------------------------------
// Source metrics (cumulative counts)
// ---------------------------------------------------------------------------

/// Get last 24 hours of cumulative source counts, aggregated hourly.
pub async fn get_last_day_sources(pool: &PgPool) -> Result<Vec<IntervalMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, IntervalMetricRow>(
        "WITH hourly_series AS ( \
             SELECT generate_series( \
                        DATE_TRUNC('hour', NOW() - INTERVAL '23 hours'), \
                        DATE_TRUNC('hour', NOW()), \
                        '1 hour' \
                    ) AS start_interval), \
         before_count AS ( \
             SELECT COUNT(*) AS source_count \
             FROM sources \
             WHERE created_at < DATE_TRUNC('hour', NOW() - INTERVAL '23 hours')), \
         hourly_sources AS ( \
             SELECT hs.start_interval, \
                    COUNT(s.uuid) AS sources_in_hour \
             FROM hourly_series hs \
                  LEFT JOIN sources s \
                      ON s.created_at >= hs.start_interval \
                     AND s.created_at < hs.start_interval + INTERVAL '1 hour' \
             GROUP BY hs.start_interval), \
         cumulative_sources AS ( \
             SELECT start_interval, \
                    SUM(sources_in_hour) OVER (ORDER BY start_interval) \
                        + (SELECT source_count FROM before_count) AS cumulative_source_count \
             FROM hourly_sources) \
         SELECT start_interval, \
                start_interval + INTERVAL '1 hour' AS end_interval, \
                cumulative_source_count::BIGINT AS count \
         FROM cumulative_sources \
         ORDER BY start_interval",
    )
    .fetch_all(pool)
    .await
}

/// Get last 7 days of cumulative source counts, aggregated daily.
pub async fn get_last_week_sources(
    pool: &PgPool,
    timezone: &str,
) -> Result<Vec<IntervalMetricRow>, sqlx::Error> {
    sqlx::query_as::<_, IntervalMetricRow>(
        "WITH local_now AS ( \
             SELECT (NOW() AT TIME ZONE $1) AS local_now), \
         daily_series AS ( \
             SELECT generate_series( \
                        DATE_TRUNC('day', ln.local_now - INTERVAL '6 days'), \
                        DATE_TRUNC('day', ln.local_now), \
                        '1 day' \
                    ) AS start_interval \
             FROM local_now ln), \
         before_count AS ( \
             SELECT COUNT(*) AS source_count \
             FROM sources s CROSS JOIN local_now ln \
             WHERE (s.created_at AT TIME ZONE $1) < DATE_TRUNC('day', ln.local_now - INTERVAL '6 days')), \
         daily_sources AS ( \
             SELECT ds.start_interval, \
                    COUNT(s.uuid) AS sources_in_day \
             FROM daily_series ds \
                  LEFT JOIN sources s \
                      ON (s.created_at AT TIME ZONE $1) >= ds.start_interval \
                     AND (s.created_at AT TIME ZONE $1) < ds.start_interval + INTERVAL '1 day' \
             GROUP BY ds.start_interval), \
         cumulative_sources AS ( \
             SELECT start_interval, \
                    SUM(sources_in_day) OVER (ORDER BY start_interval) \
                        + (SELECT source_count FROM before_count) AS cumulative_source_count \
             FROM daily_sources) \
         SELECT start_interval::TIMESTAMPTZ, \
                (start_interval + INTERVAL '1 day')::TIMESTAMPTZ AS end_interval, \
                cumulative_source_count::BIGINT AS count \
         FROM cumulative_sources \
         ORDER BY start_interval",
    )
    .bind(timezone)
    .fetch_all(pool)
    .await
}
