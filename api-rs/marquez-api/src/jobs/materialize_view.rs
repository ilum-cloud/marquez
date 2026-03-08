use sqlx::PgPool;
use tokio::time::{interval, Duration};
use tokio_util::sync::CancellationToken;

/// Periodically refreshes the `lineage_events_by_type_hourly_view` materialized view.
///
/// Runs every 60 minutes, matching Java's `MaterializeViewRefresherJob`.
/// Uses `REFRESH MATERIALIZED VIEW` (not CONCURRENTLY) to match Java behavior.
pub async fn run(pool: PgPool, token: CancellationToken) {
    let mut tick = interval(Duration::from_secs(3600)); // 60 minutes
    tracing::info!("Materialized view refresh job started (every 60 mins)");

    loop {
        tokio::select! {
            _ = tick.tick() => {
                tracing::info!("Refreshing materialized views...");
                match sqlx::query(
                    "REFRESH MATERIALIZED VIEW lineage_events_by_type_hourly_view"
                )
                .execute(&pool)
                .await
                {
                    Ok(_) => {
                        tracing::info!("Materialized view `lineage_events_by_type_hourly_view` refreshed");
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh materialized views. Retrying on next run: {}", e);
                    }
                }
            }
            _ = token.cancelled() => {
                tracing::info!("Materialized view refresh job shutting down");
                return;
            }
        }
    }
}
