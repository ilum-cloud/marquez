use crate::generators;
use crate::TestApp;
use serde_json::{json, Value};

/// Percent-encode a value for use as a single URL path segment.
fn encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Helper: create prerequisite namespace and source via API, returning their names.
async fn setup(app: &TestApp) -> (String, String) {
    let ns = generators::new_namespace_name();
    let src = generators::new_source_name();

    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}",
            app.base_url,
            encode(&ns)
        ))
        .json(&json!({"ownerName": "test_owner"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "namespace creation failed");

    let resp = app
        .client
        .put(format!("{}/api/v1/sources/{}", app.base_url, src))
        .json(&json!({
            "type": "POSTGRESQL",
            "connectionUrl": "postgresql://localhost:5432/test"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "source creation failed");

    (ns, src)
}

/// Helper: create a dataset and return its name.
async fn create_dataset(app: &TestApp, ns: &str, src: &str, description: Option<&str>) -> String {
    let ds = generators::new_dataset_name();
    let physical = generators::new_physical_name();

    let mut body = json!({
        "type": "DB_TABLE",
        "physicalName": physical,
        "sourceName": src,
    });
    if let Some(desc) = description {
        body.as_object_mut()
            .unwrap()
            .insert("description".to_string(), json!(desc));
    }

    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(ns),
            ds
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "dataset creation failed");

    ds
}

/// Helper: create a tag via API.
async fn create_tag(app: &TestApp, tag_name: &str) {
    let resp = app
        .client
        .put(format!("{}/api/v1/tags/{}", app.base_url, tag_name))
        .json(&json!({"description": "test tag"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "tag creation failed");
}

// -------------------------------------------------------------------------
// 1. create_and_get_dataset
// -------------------------------------------------------------------------

#[tokio::test]
async fn create_and_get_dataset() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    let ds_name = generators::new_dataset_name();
    let physical = generators::new_physical_name();

    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .json(&json!({
            "type": "DB_TABLE",
            "physicalName": physical,
            "sourceName": src,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], ds_name);
    assert_eq!(body["physicalName"], physical);
    assert_eq!(body["sourceName"], src);
    assert_eq!(body["type"], "DB_TABLE");
    assert_eq!(body["namespace"], ns);

    // GET the dataset back
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], ds_name);
    assert_eq!(body["physicalName"], physical);
    assert_eq!(body["sourceName"], src);
    assert_eq!(body["type"], "DB_TABLE");
    assert_eq!(body["namespace"], ns);
}

// -------------------------------------------------------------------------
// 2. create_dataset_with_description
// -------------------------------------------------------------------------

#[tokio::test]
async fn create_dataset_with_description() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    let ds_name = generators::new_dataset_name();
    let desc = generators::new_description();

    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .json(&json!({
            "type": "DB_TABLE",
            "physicalName": generators::new_physical_name(),
            "sourceName": src,
            "description": desc,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], ds_name);
    assert_eq!(body["description"], desc);
}

// -------------------------------------------------------------------------
// 3. list_datasets
// -------------------------------------------------------------------------

#[tokio::test]
async fn list_datasets() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    create_dataset(&app, &ns, &src, None).await;
    create_dataset(&app, &ns, &src, None).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets",
            app.base_url,
            encode(&ns)
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let total_count = body["totalCount"].as_i64().unwrap();
    let datasets = body["datasets"].as_array().unwrap();
    assert!(
        total_count >= 2,
        "expected totalCount >= 2, got {total_count}"
    );
    assert!(datasets.len() >= 2, "expected at least 2 datasets");
}

// -------------------------------------------------------------------------
// 4. list_datasets_with_pagination
// -------------------------------------------------------------------------

#[tokio::test]
async fn list_datasets_with_pagination() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    create_dataset(&app, &ns, &src, None).await;
    create_dataset(&app, &ns, &src, None).await;
    create_dataset(&app, &ns, &src, None).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets?limit=2",
            app.base_url,
            encode(&ns)
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let total_count = body["totalCount"].as_i64().unwrap();
    let datasets = body["datasets"].as_array().unwrap();
    assert!(
        total_count >= 3,
        "expected totalCount >= 3, got {total_count}"
    );
    assert!(
        datasets.len() <= 2,
        "expected at most 2 datasets with limit=2, got {}",
        datasets.len()
    );
}

// -------------------------------------------------------------------------
// 5. delete_dataset
// -------------------------------------------------------------------------

#[tokio::test]
async fn delete_dataset() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    let ds_name = create_dataset(&app, &ns, &src, None).await;

    // DELETE
    let resp = app
        .client
        .delete(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], ds_name);

    // GET after delete should return 404
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// -------------------------------------------------------------------------
// 6. get_dataset_not_found
// -------------------------------------------------------------------------

#[tokio::test]
async fn get_dataset_not_found() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let ds = generators::new_dataset_name();

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// -------------------------------------------------------------------------
// 7. create_dataset_idempotent
// -------------------------------------------------------------------------

#[tokio::test]
async fn create_dataset_idempotent() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    let ds_name = generators::new_dataset_name();
    let physical = generators::new_physical_name();

    let body = json!({
        "type": "DB_TABLE",
        "physicalName": physical,
        "sourceName": src,
    });

    // First PUT
    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Second PUT with the same name — should not fail
    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let result: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(result["name"], ds_name);
}

// -------------------------------------------------------------------------
// 8. tag_and_untag_dataset
// -------------------------------------------------------------------------

#[tokio::test]
async fn tag_and_untag_dataset() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;
    let ds_name = create_dataset(&app, &ns, &src, None).await;

    let tag_name = generators::new_tag_name();
    create_tag(&app, &tag_name).await;

    // POST tag onto dataset
    let resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/datasets/{}/tags/{}",
            app.base_url,
            encode(&ns),
            ds_name,
            tag_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET dataset and verify tag is present
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let tags = body["tags"].as_array().expect("tags should be an array");
    assert!(
        tags.iter().any(|t| t.as_str() == Some(&tag_name)),
        "expected tag '{}' in tags {:?}",
        tag_name,
        tags
    );

    // DELETE tag from dataset
    let resp = app
        .client
        .delete(format!(
            "{}/api/v1/namespaces/{}/datasets/{}/tags/{}",
            app.base_url,
            encode(&ns),
            ds_name,
            tag_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET dataset and verify tag is removed
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let tags = body["tags"].as_array().expect("tags should be an array");
    assert!(
        !tags.iter().any(|t| t.as_str() == Some(&tag_name)),
        "expected tag '{}' to be removed, but found in tags {:?}",
        tag_name,
        tags
    );
}

// -------------------------------------------------------------------------
// 9. list_dataset_versions
// -------------------------------------------------------------------------

#[tokio::test]
async fn list_dataset_versions() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;
    let ds_name = create_dataset(&app, &ns, &src, None).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}/versions",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("totalCount").is_some(),
        "response should have totalCount"
    );
    assert!(
        body.get("versions").is_some(),
        "response should have versions"
    );
    let total_count = body["totalCount"].as_i64().unwrap();
    let versions = body["versions"].as_array().unwrap();
    assert!(total_count >= 0);
    assert_eq!(versions.len() as i64, total_count);
}

// -------------------------------------------------------------------------
// 10. create_dataset_db_table_type
// -------------------------------------------------------------------------

#[tokio::test]
async fn create_dataset_db_table_type() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    let ds_name = generators::new_dataset_name();

    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .json(&json!({
            "type": "DB_TABLE",
            "physicalName": generators::new_physical_name(),
            "sourceName": src,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["type"], "DB_TABLE");
    assert_eq!(body["name"], ds_name);
    assert_eq!(body["namespace"], ns);
}

// -------------------------------------------------------------------------
// put_dataset_fields_in_version_mapping
// -------------------------------------------------------------------------

/// PUT-created datasets should map fields to the version, so updating with
/// fewer fields shows only the new set.
#[tokio::test]
async fn put_dataset_fields_in_version_mapping() {
    let app = TestApp::new().await;
    let (ns, src) = setup(&app).await;

    let ds_name = generators::new_dataset_name();
    let physical = generators::new_physical_name();

    // PUT with fields [a, b, c]
    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .json(&json!({
            "type": "DB_TABLE",
            "physicalName": physical,
            "sourceName": src,
            "fields": [
                {"name": "a", "type": "INT"},
                {"name": "b", "type": "VARCHAR"},
                {"name": "c", "type": "VARCHAR"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET — should see 3 fields
    let ds: Value = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fields = ds["fields"].as_array().expect("fields array");
    assert_eq!(
        fields.len(),
        3,
        "expected 3 fields after first PUT, got {}: {:?}",
        fields.len(),
        fields
            .iter()
            .map(|f| f["name"].as_str())
            .collect::<Vec<_>>()
    );

    // PUT with fields [a] only
    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .json(&json!({
            "type": "DB_TABLE",
            "physicalName": physical,
            "sourceName": src,
            "fields": [
                {"name": "a", "type": "INT"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET — should now see only 1 field
    let ds: Value = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fields = ds["fields"].as_array().expect("fields array");
    assert_eq!(
        fields.len(),
        1,
        "expected 1 field after second PUT, got {}: {:?}",
        fields.len(),
        fields
            .iter()
            .map(|f| f["name"].as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(fields[0]["name"], "a");
}

// -------------------------------------------------------------------------
// Bug 22: untag_field removes from version JSONB
// -------------------------------------------------------------------------

/// Tag a dataset field, then untag it. Verify the version JSONB is updated
/// to reflect the removal.
#[tokio::test]
async fn untag_field_updates_version_jsonb() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = uuid::Uuid::new_v4();
    let ds_name = format!("ds_untag_{}", rand::random_range(0..u32::MAX));

    // Create dataset with a field via lineage event
    let event = json!({
        "eventType": "COMPLETE",
        "eventTime": "2024-01-01T00:00:00Z",
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [{
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        {"name": "email", "type": "VARCHAR"}
                    ]
                }
            }
        }],
        "producer": "test-producer"
    });
    let url = format!("{}/api/v1/lineage", app.base_url);
    let resp = app.client.post(&url).json(&event).send().await.unwrap();
    assert_eq!(resp.status(), 201);

    let tag_name = "PII";
    create_tag(&app, tag_name).await;

    // Tag the field
    let tag_url = format!(
        "{}/api/v1/namespaces/{}/datasets/{}/fields/email/tags/{}",
        app.base_url,
        encode(&ns),
        ds_name,
        tag_name
    );
    let resp = app.client.post(&tag_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify tag is present on field
    let ds: Value = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fields = ds["fields"].as_array().expect("fields array");
    let email_field = fields
        .iter()
        .find(|f| f["name"] == "email")
        .expect("email field");
    let tags = email_field["tags"].as_array().unwrap();
    assert!(
        tags.iter().any(|t| t.as_str() == Some(tag_name)),
        "field should have tag {} before untag",
        tag_name
    );

    // Untag the field
    let untag_url = format!(
        "{}/api/v1/namespaces/{}/datasets/{}/fields/email/tags/{}",
        app.base_url,
        encode(&ns),
        ds_name,
        tag_name
    );
    let resp = app.client.delete(&untag_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify tag is removed from field
    let ds: Value = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fields = ds["fields"].as_array().expect("fields array");
    let email_field = fields
        .iter()
        .find(|f| f["name"] == "email")
        .expect("email field");
    let tags = email_field["tags"].as_array().unwrap();
    assert!(
        !tags.iter().any(|t| t.as_str() == Some(tag_name)),
        "field tag {} should be removed after untag",
        tag_name
    );
}

// -------------------------------------------------------------------------
// Bug 25: Tag names uppercased
// -------------------------------------------------------------------------

/// Verify that tag names are uppercased when tagging a field with a lowercase
/// tag name.
#[tokio::test]
async fn tag_field_uppercases_tag_name() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = uuid::Uuid::new_v4();
    let ds_name = format!("ds_upcase_{}", rand::random_range(0..u32::MAX));

    // Create dataset with a field via lineage event
    let event = json!({
        "eventType": "COMPLETE",
        "eventTime": "2024-01-01T00:00:00Z",
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [{
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        {"name": "ssn", "type": "VARCHAR"}
                    ]
                }
            }
        }],
        "producer": "test-producer"
    });
    let url = format!("{}/api/v1/lineage", app.base_url);
    let resp = app.client.post(&url).json(&event).send().await.unwrap();
    assert_eq!(resp.status(), 201);

    // Tag the field with a LOWERCASE tag name "pii"
    let tag_url = format!(
        "{}/api/v1/namespaces/{}/datasets/{}/fields/ssn/tags/pii",
        app.base_url,
        encode(&ns),
        ds_name,
    );
    let resp = app.client.post(&tag_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify the tag is stored as uppercase "PII"
    let ds: Value = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/datasets/{}",
            app.base_url,
            encode(&ns),
            ds_name
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fields = ds["fields"].as_array().expect("fields array");
    let ssn_field = fields
        .iter()
        .find(|f| f["name"] == "ssn")
        .expect("ssn field");
    let tags = ssn_field["tags"].as_array().unwrap();
    assert!(
        tags.iter().any(|t| t.as_str() == Some("PII")),
        "lowercase tag 'pii' should be stored as uppercase 'PII', got tags: {:?}",
        tags
    );
}
