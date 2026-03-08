// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use figment::providers::{Env, Format, Yaml};
use figment::Figment;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use marquez_api::config::MarquezConfig;
use marquez_api::db::migration::MigrationRunner;
use marquez_api::jobs;

#[derive(Parser)]
#[command(name = "marquez", about = "Marquez metadata API server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to configuration file
    #[arg(short, long, default_value = "marquez.yml")]
    config: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the API server (default)
    Serve {
        /// Path to configuration file
        #[arg(short, long, default_value = "marquez.yml")]
        config: PathBuf,
    },
    /// Run database migrations
    DbMigrate {
        /// Path to configuration file
        #[arg(short, long, default_value = "marquez.yml")]
        config: PathBuf,
    },
    /// Run database retention cleanup (one-shot)
    DbRetention {
        /// Path to configuration file
        #[arg(short, long, default_value = "marquez.yml")]
        config: PathBuf,
        /// Number of rows to delete per batch
        #[arg(long, default_value = "1000")]
        number_of_rows_per_batch: i64,
        /// Number of days to retain data
        #[arg(long, default_value = "7")]
        retention_days: i32,
        /// Perform a dry run (log what would be deleted)
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() {
    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    if std::env::var("MARQUEZ_LOG_JSON").is_ok() {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    let cli = Cli::parse();

    match cli.command {
        None => serve(cli.config).await,
        Some(Commands::Serve { config }) => serve(config).await,
        Some(Commands::DbMigrate { config }) => db_migrate(config).await,
        Some(Commands::DbRetention {
            config,
            number_of_rows_per_batch,
            retention_days,
            dry_run,
        }) => db_retention(config, number_of_rows_per_batch, retention_days, dry_run).await,
    }
}

fn load_config(path: &PathBuf) -> MarquezConfig {
    Figment::new()
        .merge(Yaml::file(path))
        .merge(Env::prefixed("MARQUEZ_").split("__"))
        .extract()
        .expect("Failed to load configuration")
}

async fn connect_db(config: &MarquezConfig) -> sqlx::PgPool {
    let db_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        config.db.user, config.db.password, config.db.host, config.db.port, config.db.name
    );

    let max_retries = 5;
    let mut delay = std::time::Duration::from_secs(1);

    for attempt in 1..=max_retries {
        match PgPoolOptions::new()
            .max_connections(config.db.max_pool_size)
            .connect(&db_url)
            .await
        {
            Ok(pool) => {
                tracing::info!(
                    "Connected to database at {}:{}/{}",
                    config.db.host,
                    config.db.port,
                    config.db.name
                );
                return pool;
            }
            Err(e) => {
                if attempt == max_retries {
                    tracing::error!(
                        "Failed to connect to database after {} attempts: {}",
                        max_retries,
                        e
                    );
                    std::process::exit(1);
                }
                tracing::warn!(
                    "Database connection attempt {}/{} failed: {}. Retrying in {:?}...",
                    attempt,
                    max_retries,
                    e,
                    delay
                );
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
        }
    }
    unreachable!()
}

async fn run_migrations(pool: &sqlx::PgPool) {
    // Try ./migrations first (Docker layout), then ../migrations (local dev from marquez-api/)
    let migrations_dir = ["./migrations", "../migrations"]
        .iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists());

    let Some(migrations_dir) = migrations_dir else {
        tracing::warn!("Migrations directory not found (tried ./migrations and ../migrations)");
        return;
    };

    let runner = MigrationRunner::new(&migrations_dir);
    match runner.run_all(pool).await {
        Ok(()) => tracing::info!("Database migrations completed from {:?}", migrations_dir),
        Err(e) => tracing::warn!("Migration skipped (DB may already be migrated): {}", e),
    }
}

async fn seed_tags(pool: &sqlx::PgPool, tags: &[marquez_api::config::TagConfig]) {
    use chrono::Utc;
    use uuid::Uuid;

    for tag in tags {
        if let Err(e) = marquez_api::db::tag::upsert(
            pool,
            Uuid::new_v4(),
            Utc::now(),
            &tag.name,
            tag.description.as_deref(),
        )
        .await
        {
            tracing::warn!("Failed to seed tag '{}': {}", tag.name, e);
        }
    }
    tracing::info!("Seeded {} tags from config", tags.len());
}

async fn seed_default_namespace(pool: &sqlx::PgPool) {
    use chrono::Utc;
    use uuid::Uuid;

    if let Err(e) = marquez_api::db::namespace::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        "default",
        "anonymous",
        Some(
            "The default global namespace for dataset, job, and run metadata \
             not belonging to a user-specified namespace.",
        ),
    )
    .await
    {
        tracing::warn!("Failed to seed default namespace: {}", e);
    }

    // Ensure the owner row exists too (matches Java's NamespaceService.init)
    if let Err(e) =
        marquez_api::db::namespace::upsert_owner(pool, Uuid::new_v4(), Utc::now(), "anonymous")
            .await
    {
        tracing::warn!("Failed to seed anonymous owner: {}", e);
    }
}

async fn serve(config_path: PathBuf) {
    let config = load_config(&config_path);

    // Initialize Sentry before anything else (guard must live for app lifetime)
    let _sentry_guard = marquez_tracing::init_sentry(&config.sentry);

    let pool = connect_db(&config).await;

    // Conditional migration
    if config.migrate_on_startup {
        run_migrations(&pool).await;
    }

    // Seed default namespace (matches Java NamespaceService.init)
    seed_default_namespace(&pool).await;

    // Seed configured tags
    if !config.tags.is_empty() {
        seed_tags(&pool, &config.tags).await;
    }

    let cancel_token = CancellationToken::new();

    // Spawn materialized view refresh job
    let mv_token = cancel_token.clone();
    let mv_pool = pool.clone();
    tokio::spawn(async move {
        jobs::materialize_view::run(mv_pool, mv_token).await;
    });

    // Spawn DB retention job (only if configured)
    if let Some(ref retention_config) = config.db_retention {
        let ret_token = cancel_token.clone();
        let ret_pool = pool.clone();
        let ret_cfg = jobs::db_retention::RetentionConfig {
            frequency_mins: retention_config.frequency_mins,
            batch_size: retention_config.number_of_rows_per_batch,
            retention_days: retention_config.retention_days,
        };
        tokio::spawn(async move {
            jobs::db_retention::run(ret_pool, ret_cfg, ret_token).await;
        });
    }

    // Create OpenSearch client (if enabled).
    // OpenSearch may need 15-30s to fully start in Docker — be patient.
    let search_client = match marquez_search::SearchClient::new(&config.search) {
        Ok(Some(client)) => {
            if client.ping_with_retry(10).await {
                client.ensure_indices().await;
                Some(client)
            } else {
                tracing::warn!("OpenSearch not reachable — search will use PostgreSQL only");
                None
            }
        }
        Ok(None) => {
            tracing::info!("OpenSearch search disabled");
            None
        }
        Err(e) => {
            tracing::warn!("Failed to create OpenSearch client: {e}");
            None
        }
    };

    // Build admin app and bind to admin port
    let admin_app = marquez_api::app::build_admin_app(pool.clone());
    let admin_addr = format!("{}:{}", config.server.host, config.server.admin_port);
    let admin_listener = TcpListener::bind(&admin_addr)
        .await
        .expect("Failed to bind admin port");
    tracing::info!("Admin server listening on {}", admin_addr);
    tokio::spawn(async move {
        axum::serve(admin_listener, admin_app)
            .await
            .expect("Admin server failed");
    });

    // Build main API app
    let app = marquez_api::app::build_app(pool, search_client, &config.exclusions);

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("Starting Marquez API server on {}", bind_addr);

    let listener = TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel_token))
        .await
        .expect("Server failed");
}

async fn db_migrate(config_path: PathBuf) {
    let config = load_config(&config_path);
    let pool = connect_db(&config).await;
    run_migrations(&pool).await;
    tracing::info!("Migrations complete. Exiting.");
}

async fn db_retention(
    config_path: PathBuf,
    number_of_rows_per_batch: i64,
    retention_days: i32,
    dry_run: bool,
) {
    let config = load_config(&config_path);
    let pool = connect_db(&config).await;

    if dry_run {
        tracing::info!(
            "DRY RUN: would delete rows older than {} days (batch size: {})",
            retention_days,
            number_of_rows_per_batch
        );
        return;
    }

    let ret_cfg = jobs::db_retention::RetentionConfig {
        frequency_mins: 0, // unused for one-shot
        batch_size: number_of_rows_per_batch,
        retention_days,
    };

    match jobs::db_retention::run_retention(&pool, &ret_cfg).await {
        Ok(()) => tracing::info!("Retention cleanup complete."),
        Err(e) => {
            tracing::error!("Retention cleanup failed: {}", e);
            std::process::exit(1);
        }
    }
}

async fn shutdown_signal(token: CancellationToken) {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");

        tokio::select! {
            _ = ctrl_c => tracing::info!("Received SIGINT"),
            _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("failed to listen for ctrl_c");
        tracing::info!("Received SIGINT");
    }

    tracing::info!("Shutting down gracefully...");
    token.cancel();
}
