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
    async fn create_and_get_source() {
        let app = TestApp::new().await;
        let name = generators::new_source_name();
        let connection_url = generators::new_connection_url();

        // PUT to create the source
        let put_resp = app
            .client
            .put(&format!(
                "{}/api/v1/sources/{}",
                app.base_url,
                encode(&name)
            ))
            .json(&json!({
                "type": "POSTGRESQL",
                "connectionUrl": connection_url
            }))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(put_resp.status().as_u16(), 200);

        let put_body: serde_json::Value = put_resp.json().await.expect("parse PUT response");
        assert_eq!(put_body["name"], name);
        assert_eq!(put_body["type"], "POSTGRESQL");
        assert_eq!(put_body["connectionUrl"], connection_url);
        assert!(put_body.get("createdAt").is_some());
        assert!(put_body.get("updatedAt").is_some());

        // GET to retrieve the source
        let get_resp = app
            .client
            .get(&format!(
                "{}/api/v1/sources/{}",
                app.base_url,
                encode(&name)
            ))
            .send()
            .await
            .expect("GET request failed");
        assert_eq!(get_resp.status().as_u16(), 200);

        let get_body: serde_json::Value = get_resp.json().await.expect("parse GET response");
        assert_eq!(get_body["name"], name);
        assert_eq!(get_body["type"], "POSTGRESQL");
        assert_eq!(get_body["connectionUrl"], connection_url);
    }

    #[tokio::test]
    async fn create_source_with_description() {
        let app = TestApp::new().await;
        let name = generators::new_source_name();
        let connection_url = generators::new_connection_url();
        let description = generators::new_description();

        let resp = app
            .client
            .put(&format!(
                "{}/api/v1/sources/{}",
                app.base_url,
                encode(&name)
            ))
            .json(&json!({
                "type": "POSTGRESQL",
                "connectionUrl": connection_url,
                "description": description
            }))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse response");
        assert_eq!(body["name"], name);
        assert_eq!(body["type"], "POSTGRESQL");
        assert_eq!(body["connectionUrl"], connection_url);
        assert_eq!(body["description"], description);
    }

    #[tokio::test]
    async fn list_sources() {
        let app = TestApp::new().await;
        let name1 = generators::new_source_name();
        let name2 = generators::new_source_name();

        // Create two sources
        app.client
            .put(&format!(
                "{}/api/v1/sources/{}",
                app.base_url,
                encode(&name1)
            ))
            .json(&json!({
                "type": "POSTGRESQL",
                "connectionUrl": generators::new_connection_url()
            }))
            .send()
            .await
            .expect("PUT src1 failed");

        app.client
            .put(&format!(
                "{}/api/v1/sources/{}",
                app.base_url,
                encode(&name2)
            ))
            .json(&json!({
                "type": "POSTGRESQL",
                "connectionUrl": generators::new_connection_url()
            }))
            .send()
            .await
            .expect("PUT src2 failed");

        // List sources
        let resp = app
            .client
            .get(&format!("{}/api/v1/sources", app.base_url))
            .send()
            .await
            .expect("GET list failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse list response");
        let sources = body["sources"].as_array().expect("sources is array");
        assert!(sources.len() >= 2);

        let names: Vec<&str> = sources
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&name1.as_str()));
        assert!(names.contains(&name2.as_str()));
    }

    #[tokio::test]
    async fn get_source_not_found() {
        let app = TestApp::new().await;
        let name = "nonexistent-source-that-does-not-exist";

        let resp = app
            .client
            .get(&format!("{}/api/v1/sources/{}", app.base_url, encode(name)))
            .send()
            .await
            .expect("GET request failed");
        assert_eq!(resp.status().as_u16(), 404);
    }

    #[tokio::test]
    async fn create_source_idempotent() {
        let app = TestApp::new().await;
        let name = generators::new_source_name();
        let connection_url = generators::new_connection_url();

        let request_body = json!({
            "type": "POSTGRESQL",
            "connectionUrl": connection_url
        });

        let url = format!("{}/api/v1/sources/{}", app.base_url, encode(&name));

        // First PUT
        let resp1 = app
            .client
            .put(&url)
            .json(&request_body)
            .send()
            .await
            .expect("PUT 1 failed");
        assert_eq!(resp1.status().as_u16(), 200);

        let body1: serde_json::Value = resp1.json().await.expect("parse response 1");

        // Second PUT with same data
        let resp2 = app
            .client
            .put(&url)
            .json(&request_body)
            .send()
            .await
            .expect("PUT 2 failed");
        assert_eq!(resp2.status().as_u16(), 200);

        let body2: serde_json::Value = resp2.json().await.expect("parse response 2");

        assert_eq!(body1["name"], body2["name"]);
        assert_eq!(body1["type"], body2["type"]);
        assert_eq!(body1["connectionUrl"], body2["connectionUrl"]);
    }

    // -----------------------------------------------------------------------
    // Bug 34: Source upsert clears description when set to null
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn source_upsert_clears_description() {
        let app = TestApp::new().await;
        let name = generators::new_source_name();
        let connection_url = generators::new_connection_url();
        let url = format!("{}/api/v1/sources/{}", app.base_url, encode(&name));

        // Create source WITH description
        let resp1 = app
            .client
            .put(&url)
            .json(&json!({
                "type": "POSTGRESQL",
                "connectionUrl": connection_url,
                "description": "original description"
            }))
            .send()
            .await
            .expect("PUT 1 failed");
        assert_eq!(resp1.status().as_u16(), 200);
        let body1: serde_json::Value = resp1.json().await.unwrap();
        assert_eq!(body1["description"], "original description");

        // Update source WITHOUT description (should clear it)
        let resp2 = app
            .client
            .put(&url)
            .json(&json!({
                "type": "POSTGRESQL",
                "connectionUrl": connection_url
            }))
            .send()
            .await
            .expect("PUT 2 failed");
        assert_eq!(resp2.status().as_u16(), 200);
        let body2: serde_json::Value = resp2.json().await.unwrap();
        assert!(
            body2["description"].is_null(),
            "description should be cleared (null) after update without description, got {:?}",
            body2["description"]
        );
    }
}
