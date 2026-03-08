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
    async fn create_and_list_tags() {
        let app = TestApp::new().await;
        let name = generators::new_tag_name();
        let description = generators::new_description();

        // PUT to create the tag
        let put_resp = app
            .client
            .put(&format!("{}/api/v1/tags/{}", app.base_url, encode(&name)))
            .json(&json!({ "description": description }))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(put_resp.status().as_u16(), 200);

        let put_body: serde_json::Value = put_resp.json().await.expect("parse PUT response");
        assert_eq!(put_body["name"], name);
        assert_eq!(put_body["description"], description);

        // List tags and verify the created tag is present
        let list_resp = app
            .client
            .get(&format!("{}/api/v1/tags", app.base_url))
            .send()
            .await
            .expect("GET list failed");
        assert_eq!(list_resp.status().as_u16(), 200);

        let list_body: serde_json::Value = list_resp.json().await.expect("parse list response");
        let tags = list_body["tags"].as_array().expect("tags is array");
        assert!(tags.len() >= 1);

        let names: Vec<&str> = tags.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&name.as_str()));
    }

    #[tokio::test]
    async fn create_tag_with_description() {
        let app = TestApp::new().await;
        let name = generators::new_tag_name();
        let description = generators::new_description();

        let resp = app
            .client
            .put(&format!("{}/api/v1/tags/{}", app.base_url, encode(&name)))
            .json(&json!({ "description": description }))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse response");
        assert_eq!(body["name"], name);
        assert_eq!(body["description"], description);
    }

    #[tokio::test]
    async fn create_tag_without_description() {
        let app = TestApp::new().await;
        let name = generators::new_tag_name();

        let resp = app
            .client
            .put(&format!("{}/api/v1/tags/{}", app.base_url, encode(&name)))
            .json(&json!({}))
            .send()
            .await
            .expect("PUT request failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse response");
        assert_eq!(body["name"], name);
        assert!(
            body["description"].is_null(),
            "description should be null when not provided"
        );
    }

    #[tokio::test]
    async fn list_tags_pagination() {
        let app = TestApp::new().await;

        let mut created_names = Vec::new();
        for _ in 0..3 {
            let name = generators::new_tag_name();
            app.client
                .put(&format!("{}/api/v1/tags/{}", app.base_url, encode(&name)))
                .json(&json!({}))
                .send()
                .await
                .expect("PUT failed");
            created_names.push(name);
        }

        // Fetch page 1 with limit=2, offset=0
        let resp = app
            .client
            .get(&format!("{}/api/v1/tags?limit=2&offset=0", app.base_url))
            .send()
            .await
            .expect("GET page 1 failed");
        assert_eq!(resp.status().as_u16(), 200);

        let body: serde_json::Value = resp.json().await.expect("parse page 1");
        let tags = body["tags"].as_array().expect("tags is array");
        assert_eq!(tags.len(), 2);

        // Fetch page 2 with limit=2, offset=2
        let resp2 = app
            .client
            .get(&format!("{}/api/v1/tags?limit=2&offset=2", app.base_url))
            .send()
            .await
            .expect("GET page 2 failed");
        assert_eq!(resp2.status().as_u16(), 200);

        let body2: serde_json::Value = resp2.json().await.expect("parse page 2");
        let tags2 = body2["tags"].as_array().expect("tags is array");
        assert!(tags2.len() >= 1, "expected at least 1 result on page 2");
    }
}
