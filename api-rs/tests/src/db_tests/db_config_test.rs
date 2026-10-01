// SPDX-License-Identifier: Apache-2.0

use marquez_api::config::DbConfig;
use sqlx::postgres::PgPoolOptions;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

/// Credentials with URL-reserved characters must reach Postgres verbatim.
/// Formatting them into a `postgres://` URL failed with "invalid port number"
/// (`/`, `?`, `#`) or silently altered the password (`%41` decoded to `A`).
#[tokio::test]
async fn test_connect_options_with_url_reserved_characters() {
    let container = Postgres::default()
        .with_tag("16")
        .start()
        .await
        .expect("Failed to start postgres container");
    let host = container.get_host().await.expect("get host").to_string();
    let port = container.get_host_port_ipv4(5432).await.expect("get port");

    let admin = DbConfig {
        host: host.clone(),
        port,
        name: "postgres".to_string(),
        user: "postgres".to_string(),
        password: "postgres".to_string(),
        max_pool_size: 1,
    };
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(admin.connect_options())
        .await
        .expect("Failed to connect as postgres");

    let user = "mq#user/1";
    let password = "Mq/9x?Kt#2@:%41 end";
    let database = "mq?db#1";
    sqlx::query(&format!(
        "CREATE ROLE \"{user}\" LOGIN PASSWORD '{password}'"
    ))
    .execute(&admin_pool)
    .await
    .expect("create role");
    sqlx::query(&format!("CREATE DATABASE \"{database}\" OWNER \"{user}\""))
        .execute(&admin_pool)
        .await
        .expect("create database");

    let config = DbConfig {
        host,
        port,
        name: database.to_string(),
        user: user.to_string(),
        password: password.to_string(),
        max_pool_size: 1,
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(config.connect_options())
        .await
        .expect("Failed to connect with URL-reserved characters in credentials");

    let (current_user, current_db): (String, String) =
        sqlx::query_as("SELECT current_user::text, current_database()::text")
            .fetch_one(&pool)
            .await
            .expect("query");
    assert_eq!(current_user, user);
    assert_eq!(current_db, database);
}
