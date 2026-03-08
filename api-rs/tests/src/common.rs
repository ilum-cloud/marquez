use marquez_api::db::migration::MigrationRunner;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::PathBuf;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;

/// Shared testcontainer, initialized exactly once across all tests.
/// Only the container handle and connection URL are shared — each test
/// creates its own PgPool so there are no cross-runtime issues.
static SHARED_CONTAINER: OnceCell<SharedContainer> = OnceCell::const_new();

/// Shared external Postgres URL (CI), initialized exactly once.
static EXT_MIGRATED_URL: OnceCell<String> = OnceCell::const_new();

struct SharedContainer {
    _container: testcontainers::ContainerAsync<Postgres>,
    url: String,
}

pub struct TestDb {
    pub pool: PgPool,
}

impl TestDb {
    pub async fn new() -> Self {
        let url = if let Some(ext_url) = external_postgres_url() {
            // External Postgres (CI or manual) — run migrations once, then reuse URL
            EXT_MIGRATED_URL
                .get_or_init(|| async {
                    let pool = PgPoolOptions::new()
                        .max_connections(2)
                        .connect(&ext_url)
                        .await
                        .expect("Failed to connect to external test db");
                    run_migrations(&pool).await;
                    pool.close().await;
                    ext_url
                })
                .await
                .clone()
        } else {
            // Testcontainer — shared singleton, started once
            let shared = SHARED_CONTAINER
                .get_or_init(|| async {
                    let container = Postgres::default()
                        .with_tag("16")
                        .start()
                        .await
                        .expect("Failed to start postgres container");

                    let host = container.get_host().await.expect("get host");
                    let port = container.get_host_port_ipv4(5432).await.expect("get port");
                    let url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

                    // Run migrations once using a temporary pool
                    let pool = PgPoolOptions::new()
                        .max_connections(2)
                        .connect(&url)
                        .await
                        .expect("Failed to connect to test db");
                    run_migrations(&pool).await;
                    pool.close().await;

                    SharedContainer {
                        _container: container,
                        url,
                    }
                })
                .await;
            shared.url.clone()
        };

        // Each test gets its own pool (avoids cross-runtime issues with #[tokio::test])
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .expect("Failed to connect to test db");

        Self { pool }
    }

    fn migrations_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("migrations")
    }
}

/// Check for external Postgres connection via TEST_POSTGRES_* env vars (used in CI).
fn external_postgres_url() -> Option<String> {
    let host = std::env::var("TEST_POSTGRES_HOST").ok()?;
    let port = std::env::var("TEST_POSTGRES_PORT").ok()?;
    let user = std::env::var("TEST_POSTGRES_USER").ok()?;
    let password = std::env::var("TEST_POSTGRES_PASSWORD").ok()?;
    let db = std::env::var("TEST_POSTGRES_DB").ok()?;
    Some(format!(
        "postgres://{}:{}@{}:{}/{}",
        user, password, host, port, db
    ))
}

/// Run all Flyway migrations against the given pool.
async fn run_migrations(pool: &PgPool) {
    let migrations_dir = TestDb::migrations_dir();
    let runner = MigrationRunner::new(&migrations_dir);
    runner
        .run_all(pool)
        .await
        .expect("Failed to run migrations");
}
