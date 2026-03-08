#[cfg(test)]
mod tests {
    use crate::generators;
    use crate::TestApp;
    use serde_json::json;

    /// Percent-encode a value for use as a single URL path segment.
    fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }

    #[tokio::test]
    async fn create_and_get_namespace() {
        let app = TestApp::new().await;
        let name = generators::new_namespace_name();
        let owner = generators::new_owner_name();

        // PUT to create the namespace
        let put_resp = app
            .client
            .put(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .json(&json!({ "ownerName": owner }))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(put_resp.status().as_u16(), 200);

        let put_body: serde_json::Value = put_resp.json().await.expect("parse PUT response");
        assert_eq!(put_body["name"], name);
        assert_eq!(put_body["ownerName"], owner);
        assert_eq!(put_body["isHidden"], false);
        assert!(put_body.get("createdAt").is_some());
        assert!(put_body.get("updatedAt").is_some());

        // GET to retrieve the namespace
        let get_resp = app
            .client
            .get(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .send()
            .await
            .expect("GET request failed");
        assert_eq!(get_resp.status().as_u16(), 200);

        let get_body: serde_json::Value = get_resp.json().await.expect("parse GET response");
        assert_eq!(get_body["name"], name);
        assert_eq!(get_body["ownerName"], owner);
        assert_eq!(get_body["isHidden"], false);
    }

    #[tokio::test]
    async fn create_namespace_with_description() {
        let app = TestApp::new().await;
        let name = generators::new_namespace_name();
        let owner = generators::new_owner_name();
        let description = generators::new_description();

        let resp = app
            .client
            .put(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .json(&json!({
                "ownerName": owner,
                "description": description
            }))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse response");
        assert_eq!(body["name"], name);
        assert_eq!(body["ownerName"], owner);
        assert_eq!(body["description"], description);
    }

    #[tokio::test]
    async fn list_namespaces() {
        let app = TestApp::new().await;
        let name1 = generators::new_namespace_name();
        let name2 = generators::new_namespace_name();
        let owner = generators::new_owner_name();

        // Create two namespaces
        app.client
            .put(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name1)
            ))
            .json(&json!({ "ownerName": owner }))
            .send()
            .await
            .expect("PUT ns1 failed");

        app.client
            .put(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name2)
            ))
            .json(&json!({ "ownerName": owner }))
            .send()
            .await
            .expect("PUT ns2 failed");

        // List namespaces
        let resp = app
            .client
            .get(&format!("{}/api/v1/namespaces", app.base_url))
            .send()
            .await
            .expect("GET list failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse list response");
        let namespaces = body["namespaces"].as_array().expect("namespaces is array");
        assert!(namespaces.len() >= 2);

        let names: Vec<&str> = namespaces
            .iter()
            .map(|ns| ns["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&name1.as_str()));
        assert!(names.contains(&name2.as_str()));
    }

    #[tokio::test]
    async fn list_namespaces_with_pagination() {
        let app = TestApp::new().await;
        let owner = generators::new_owner_name();

        let mut created_names = Vec::new();
        for _ in 0..3 {
            let name = generators::new_namespace_name();
            app.client
                .put(&format!(
                    "{}/api/v1/namespaces/{}",
                    app.base_url,
                    encode(&name)
                ))
                .json(&json!({ "ownerName": owner }))
                .send()
                .await
                .expect("PUT failed");
            created_names.push(name);
        }

        // Fetch page 1 with limit=2, offset=0
        let resp = app
            .client
            .get(&format!(
                "{}/api/v1/namespaces?limit=2&offset=0",
                app.base_url
            ))
            .send()
            .await
            .expect("GET page 1 failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse page 1");
        let namespaces = body["namespaces"].as_array().expect("namespaces is array");
        assert_eq!(namespaces.len(), 2);

        // Fetch page 2 with limit=2, offset=2
        let resp2 = app
            .client
            .get(&format!(
                "{}/api/v1/namespaces?limit=2&offset=2",
                app.base_url
            ))
            .send()
            .await
            .expect("GET page 2 failed");
        assert_eq!(resp2.status().as_u16(), 200);

        let body2: serde_json::Value = resp2.json().await.expect("parse page 2");
        let namespaces2 = body2["namespaces"].as_array().expect("namespaces is array");
        assert!(
            namespaces2.len() >= 1,
            "expected at least 1 result on page 2"
        );
    }

    #[tokio::test]
    async fn delete_namespace() {
        let app = TestApp::new().await;
        let name = generators::new_namespace_name();
        let owner = generators::new_owner_name();

        // Create the namespace
        app.client
            .put(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .json(&json!({ "ownerName": owner }))
            .send()
            .await
            .expect("PUT failed");

        // DELETE the namespace
        let del_resp = app
            .client
            .delete(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .send()
            .await
            .expect("DELETE request failed");
        assert_eq!(del_resp.status().as_u16(), 200);

        let del_body: serde_json::Value = del_resp.json().await.expect("parse DELETE response");
        assert_eq!(del_body["name"], name);
        assert_eq!(del_body["ownerName"], owner);

        // GET should still return the namespace, but with isHidden = true (soft-delete)
        let get_resp = app
            .client
            .get(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .send()
            .await
            .expect("GET request failed");
        assert_eq!(get_resp.status().as_u16(), 200);

        let get_body: serde_json::Value = get_resp.json().await.expect("parse GET response");
        assert_eq!(get_body["name"], name);
        assert_eq!(
            get_body["isHidden"], true,
            "namespace should be soft-deleted"
        );
    }

    #[tokio::test]
    async fn get_namespace_not_found() {
        let app = TestApp::new().await;
        let name = "nonexistent-namespace-that-does-not-exist";

        let resp = app
            .client
            .get(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(name)
            ))
            .send()
            .await
            .expect("GET request failed");
        assert_eq!(resp.status().as_u16(), 404);
    }

    #[tokio::test]
    async fn create_namespace_idempotent() {
        let app = TestApp::new().await;
        let name = generators::new_namespace_name();
        let owner = generators::new_owner_name();

        let url = format!("{}/api/v1/namespaces/{}", app.base_url, encode(&name));

        // First PUT
        let resp1 = app
            .client
            .put(&url)
            .json(&json!({ "ownerName": owner }))
            .send()
            .await
            .expect("PUT 1 failed");
        assert_eq!(resp1.status().as_u16(), 200);

        let body1: serde_json::Value = resp1.json().await.expect("parse response 1");

        // Second PUT with same data
        let resp2 = app
            .client
            .put(&url)
            .json(&json!({ "ownerName": owner }))
            .send()
            .await
            .expect("PUT 2 failed");
        assert_eq!(resp2.status().as_u16(), 200);

        let body2: serde_json::Value = resp2.json().await.expect("parse response 2");

        assert_eq!(body1["name"], body2["name"]);
        assert_eq!(body1["ownerName"], body2["ownerName"]);
    }

    #[tokio::test]
    async fn create_and_get_namespace_with_special_characters() {
        let app = TestApp::new().await;
        let name = format!(
            "hive://ilum-hive-metastore-{}",
            rand::random_range(0..u32::MAX)
        );
        let owner = generators::new_owner_name();

        // PUT to create the namespace (client percent-encodes the name)
        let put_resp = app
            .client
            .put(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .json(&serde_json::json!({ "ownerName": owner }))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(put_resp.status().as_u16(), 200);

        let put_body: serde_json::Value = put_resp.json().await.expect("parse PUT response");
        assert_eq!(put_body["name"], name);
        assert_eq!(put_body["ownerName"], owner);

        // GET to retrieve the namespace
        let get_resp = app
            .client
            .get(&format!(
                "{}/api/v1/namespaces/{}",
                app.base_url,
                encode(&name)
            ))
            .send()
            .await
            .expect("GET request failed");
        assert_eq!(get_resp.status().as_u16(), 200);

        let get_body: serde_json::Value = get_resp.json().await.expect("parse GET response");
        assert_eq!(get_body["name"], name);
    }

    /// Simulates a reverse proxy that has decoded %2F and %3A in the path.
    /// The middleware must re-encode these before routing.
    ///
    /// Uses `tower::ServiceExt::oneshot` to send a request with a raw decoded
    /// path directly to the app, bypassing reqwest's URL normalization.
    #[tokio::test]
    async fn namespace_with_proxy_decoded_path() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let test_db = crate::common::TestDb::new().await;
        let pool = test_db.pool.clone();

        let name = format!(
            "hive://ilum-hive-metastore-{}",
            rand::random_range(0..u32::MAX)
        );

        // Create the namespace directly in the DB via the service layer
        let ns_svc = marquez_api::service::namespace::NamespaceService::new(pool.clone());
        ns_svc
            .create_or_update(&name, "test-owner", None)
            .await
            .expect("create namespace");

        // Build a fresh app router for oneshot use
        let app = marquez_api::app::build_app(pool, None, &Default::default());

        // Construct a request with the decoded path (as a proxy would forward it)
        let decoded_path = format!("/api/v1/namespaces/{}/datasets", name);
        let req = Request::builder()
            .uri(&decoded_path)
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();

        // The middleware should re-encode the path so routing works.
        // We expect 200 (empty list) rather than 404 (no route).
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "Expected 200 for proxy-decoded path '{}', got {}",
            decoded_path,
            resp.status()
        );
    }

    #[tokio::test]
    async fn create_namespace_preserves_owner_on_reput() {
        let app = TestApp::new().await;
        let name = generators::new_namespace_name();
        let owner1 = generators::new_owner_name();
        let owner2 = generators::new_owner_name();

        let url = format!("{}/api/v1/namespaces/{}", app.base_url, encode(&name));

        // Create with owner1
        let resp1 = app
            .client
            .put(&url)
            .json(&json!({ "ownerName": owner1 }))
            .send()
            .await
            .expect("PUT 1 failed");
        assert_eq!(resp1.status().as_u16(), 200);

        let body1: serde_json::Value = resp1.json().await.expect("parse response 1");
        assert_eq!(body1["ownerName"], owner1);

        // PUT again with a different owner — the upsert preserves the original owner
        let resp2 = app
            .client
            .put(&url)
            .json(&json!({ "ownerName": owner2 }))
            .send()
            .await
            .expect("PUT 2 failed");
        assert_eq!(resp2.status().as_u16(), 200);

        // GET should still reflect the original owner
        let get_resp = app.client.get(&url).send().await.expect("GET failed");
        assert_eq!(get_resp.status().as_u16(), 200);

        let get_body: serde_json::Value = get_resp.json().await.expect("parse GET response");
        assert_eq!(get_body["ownerName"], owner1);
    }
}
