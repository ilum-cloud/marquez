#![allow(clippy::too_many_arguments, unused_imports, dead_code)]

pub mod api_tests;
pub mod common;
pub mod db_tests;
pub mod fixtures;
pub mod generators;
pub mod service_tests;
pub mod smoke_tests;

use common::TestDb;
use tokio::net::TcpListener;

pub struct TestApp {
    pub db: TestDb,
    pub base_url: String,
    pub client: reqwest::Client,
    _server_handle: tokio::task::JoinHandle<()>,
}

impl TestApp {
    pub async fn new() -> Self {
        let db = TestDb::new().await;
        let app = marquez_api::app::build_app(db.pool.clone(), None, &Default::default());

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind to random port");
        let addr = listener.local_addr().expect("get local addr");
        let base_url = format!("http://{}", addr);

        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server failed");
        });

        let client = reqwest::Client::new();

        Self {
            db,
            base_url,
            client,
            _server_handle: server_handle,
        }
    }
}
