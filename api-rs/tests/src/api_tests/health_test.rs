#[cfg(test)]
mod tests {
    use crate::TestApp;

    #[tokio::test]
    async fn healthcheck_returns_200() {
        let app = TestApp::new().await;

        let resp = app
            .client
            .get(&format!("{}/healthcheck", app.base_url))
            .send()
            .await
            .expect("GET /healthcheck failed");

        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse response");
        assert_eq!(body["status"], "healthy");
    }

    #[tokio::test]
    async fn ping_returns_pong() {
        let app = TestApp::new().await;

        let resp = app
            .client
            .get(&format!("{}/ping", app.base_url))
            .send()
            .await
            .expect("GET /ping failed");

        assert_eq!(resp.status().as_u16(), 200);

        let body = resp.text().await.expect("read body");
        assert_eq!(body, "pong");
    }

    #[tokio::test]
    async fn healthcheck_response_has_status_field() {
        let app = TestApp::new().await;

        let resp = app
            .client
            .get(&format!("{}/healthcheck", app.base_url))
            .send()
            .await
            .expect("GET /healthcheck failed");

        let body: serde_json::Value = resp.json().await.expect("parse response");
        assert!(
            body.get("status").is_some(),
            "healthcheck response should contain a 'status' field"
        );
    }

    #[tokio::test]
    async fn cli_help_flag_works() {
        // Validate that the clap CLI is correctly configured by checking --help exits cleanly.
        // Build the binary path relative to the test crate's manifest dir.
        let bin_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target")
            .join("debug")
            .join("marquez-api");

        if !bin_path.exists() {
            // Skip if binary not built (e.g., running only lib tests without prior build)
            eprintln!(
                "Skipping cli_help_flag_works: binary not found at {:?}",
                bin_path
            );
            return;
        }

        let output = std::process::Command::new(&bin_path)
            .arg("--help")
            .output()
            .expect("Failed to run binary with --help");

        assert!(output.status.success(), "Expected --help to succeed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Marquez metadata API server"),
            "Help text should contain app description"
        );
    }
}
