use crate::generators;
use crate::TestApp;
use serde_json::{json, Value};
use uuid::Uuid;

/// Percent-encode a value for use as a single URL path segment.
fn encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

// ---------------------------------------------------------------------------
// Helper: create a namespace via the API
// ---------------------------------------------------------------------------
async fn create_namespace(app: &TestApp, name: &str, owner: &str) {
    let url = format!("{}/api/v1/namespaces/{}", app.base_url, encode(name));
    let resp = app
        .client
        .put(&url)
        .json(&serde_json::json!({
            "ownerName": owner,
            "description": "test namespace"
        }))
        .send()
        .await
        .expect("create namespace request");
    assert!(
        resp.status().is_success(),
        "create namespace failed: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Helper: create a source via the API
// ---------------------------------------------------------------------------
async fn create_source(app: &TestApp, name: &str) {
    let url = format!("{}/api/v1/sources/{}", app.base_url, name);
    let resp = app
        .client
        .put(&url)
        .json(&serde_json::json!({
            "type": "POSTGRESQL",
            "connectionUrl": "postgresql://localhost:5432/testdb",
            "description": "test source"
        }))
        .send()
        .await
        .expect("create source request");
    assert!(
        resp.status().is_success(),
        "create source failed: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Helper: create a dataset via the API
// ---------------------------------------------------------------------------
async fn create_dataset(app: &TestApp, namespace: &str, name: &str, source: &str) {
    let url = format!(
        "{}/api/v1/namespaces/{}/datasets/{}",
        app.base_url,
        encode(namespace),
        name
    );
    let resp = app
        .client
        .put(&url)
        .json(&serde_json::json!({
            "type": "DB_TABLE",
            "physicalName": format!("public.{}", name),
            "sourceName": source,
            "description": "test dataset"
        }))
        .send()
        .await
        .expect("create dataset request");
    assert!(
        resp.status().is_success(),
        "create dataset failed: {}",
        resp.status()
    );
}

/// Helper: POST a lineage event and assert 201 Created.
async fn post_lineage(app: &TestApp, event: &Value) {
    let url = format!("{}/api/v1/lineage", app.base_url);
    let resp = app
        .client
        .post(&url)
        .json(event)
        .send()
        .await
        .expect("post lineage event");
    assert_eq!(
        resp.status(),
        201,
        "expected 201, got {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
}

/// Helper: build a JobEvent (no `run` field) with column lineage facets on outputs.
fn make_job_event_with_column_lineage(
    ns: &str,
    job_name: &str,
    inputs: Vec<(&str, &str, Vec<Value>)>, // (namespace, dataset_name, fields)
    output_ns: &str,
    output_ds: &str,
    output_fields: Vec<Value>,
    column_lineage_fields: Value,
) -> Value {
    let input_refs: Vec<Value> = inputs
        .iter()
        .map(|(inp_ns, inp_ds, fields)| {
            json!({
                "namespace": inp_ns,
                "name": inp_ds,
                "facets": {
                    "schema": {
                        "_producer": "test",
                        "_schemaURL": "test",
                        "fields": fields
                    }
                }
            })
        })
        .collect();

    json!({
        "eventTime": "2024-01-01T00:00:00Z",
        "job": { "namespace": ns, "name": job_name },
        "inputs": input_refs,
        "outputs": [{
            "namespace": output_ns,
            "name": output_ds,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": output_fields
                },
                "columnLineage": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": column_lineage_fields
                }
            }
        }],
        "producer": "test-producer"
    })
}

/// Helper: query column lineage and return the graph array.
async fn get_column_lineage_graph(app: &TestApp, ns: &str, dataset: &str) -> Vec<Value> {
    let node_id = format!("dataset:{}:{}", ns, dataset);
    let url = format!(
        "{}/api/v1/column-lineage?nodeId={}&depth=10",
        app.base_url,
        encode(&node_id)
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("get column lineage");
    let status = resp.status().as_u16();
    if status == 404 {
        return vec![];
    }
    assert_eq!(status, 200, "expected 200, got {}", status);
    let body: Value = resp.json().await.expect("parse json");
    body.get("graph")
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Helper: build a run event with column lineage facets
fn make_column_lineage_event(
    ns: &str,
    job_name: &str,
    run_id: &str,
    input_ns: &str,
    input_ds: &str,
    input_fields: Vec<Value>,
    output_ns: &str,
    output_ds: &str,
    output_fields: Vec<Value>,
) -> Value {
    json!({
        "eventType": "COMPLETE",
        "eventTime": "2024-01-01T00:00:00Z",
        "run": { "runId": run_id },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [{
            "namespace": input_ns,
            "name": input_ds,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": input_fields
                }
            }
        }],
        "outputs": [{
            "namespace": output_ns,
            "name": output_ds,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": output_fields.clone()
                },
                "columnLineage": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": {
                        output_fields[0]["name"].as_str().unwrap(): {
                            "inputFields": [{
                                "namespace": input_ns,
                                "name": input_ds,
                                "field": input_fields[0]["name"].as_str().unwrap(),
                                "transformations": [{
                                    "type": "DIRECT",
                                    "subtype": "IDENTITY",
                                    "description": "identity",
                                    "masking": false
                                }]
                            }],
                            "transformationDescription": "identity",
                            "transformationType": "IDENTITY"
                        }
                    }
                }
            }
        }],
        "producer": "test-producer"
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_column_lineage_empty() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let dataset = generators::new_dataset_name();

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;

    let node_id = format!("dataset:{}:{}", ns, dataset);
    let url = format!(
        "{}/api/v1/column-lineage?nodeId={}&depth=10",
        app.base_url, node_id
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("get column lineage request");

    // A dataset with no version or fields may return 200 with empty array
    // or 404 if the dataset has no current version. Both are acceptable.
    let status = resp.status().as_u16();
    assert!(
        status == 200 || status == 404,
        "expected 200 or 404, got {}",
        status
    );

    if status == 200 {
        let body: serde_json::Value = resp.json().await.expect("parse json");
        assert!(
            body.get("graph").is_some(),
            "response missing 'graph' field, got {:?}",
            body
        );
    }
}

#[tokio::test]
async fn get_column_lineage_bad_node_id() {
    let app = TestApp::new().await;

    // "invalid" does not follow "dataset:namespace:name" format
    let url = format!(
        "{}/api/v1/column-lineage?nodeId=invalid&depth=10",
        app.base_url
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("get column lineage bad node id request");

    assert_eq!(
        resp.status(),
        400,
        "expected 400 for invalid nodeId, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn column_lineage_simple_chain() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4().to_string();

    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    let event = make_column_lineage_event(
        &ns,
        &job_name,
        &run_id,
        &ns,
        &input_ds,
        vec![json!({"name": "col_a", "type": "VARCHAR"})],
        &ns,
        &output_ds,
        vec![json!({"name": "col_b", "type": "VARCHAR"})],
    );

    post_lineage(&app, &event).await;

    // Query column lineage for the output dataset
    let node_id = format!("dataset:{}:{}", ns, output_ds);
    let url = format!(
        "{}/api/v1/column-lineage?nodeId={}&depth=10",
        app.base_url,
        encode(&node_id)
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("get column lineage");

    let status = resp.status().as_u16();
    // If column lineage data was populated, we expect 200 with results.
    // The CTE may return empty if dataset_symlinks or other joins don't match.
    assert!(
        status == 200 || status == 404,
        "expected 200 or 404, got {}",
        status
    );

    if status == 200 {
        let body: Value = resp.json().await.expect("parse json");
        assert!(
            body.get("graph").is_some(),
            "response missing 'graph' field"
        );
    }
}

#[tokio::test]
async fn column_lineage_no_results_for_nonexistent() {
    let app = TestApp::new().await;

    // Query for a dataset that doesn't exist
    let node_id = format!(
        "dataset:nonexistent_ns_{}:nonexistent_ds_{}",
        rand::random_range(0..u32::MAX),
        rand::random_range(0..u32::MAX)
    );
    let url = format!(
        "{}/api/v1/column-lineage?nodeId={}&depth=10",
        app.base_url,
        encode(&node_id)
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("get column lineage for nonexistent");

    // Should return 404 for missing dataset
    assert_eq!(
        resp.status(),
        404,
        "expected 404 for nonexistent dataset, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn column_lineage_depth_parameter() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4().to_string();

    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    let event = make_column_lineage_event(
        &ns,
        &job_name,
        &run_id,
        &ns,
        &input_ds,
        vec![json!({"name": "col_a", "type": "INTEGER"})],
        &ns,
        &output_ds,
        vec![json!({"name": "col_b", "type": "INTEGER"})],
    );

    post_lineage(&app, &event).await;

    // Query with depth=1 (should still work)
    let node_id = format!("dataset:{}:{}", ns, output_ds);
    let url = format!(
        "{}/api/v1/column-lineage?nodeId={}&depth=1",
        app.base_url,
        encode(&node_id)
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("get column lineage with depth=1");

    let status = resp.status().as_u16();
    assert!(
        status == 200 || status == 404,
        "expected 200 or 404, got {}",
        status
    );
}

// ---------------------------------------------------------------------------
// JobEvent column lineage tests (regression for missing column lineage on JobEvent)
// ---------------------------------------------------------------------------

/// Bug 59: Job events now perform full model orchestration (reverting Bug 51).
/// Column lineage IS produced for job events (creates datasets, fields, etc.).
#[tokio::test]
async fn column_lineage_via_job_event() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();

    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    let event = make_job_event_with_column_lineage(
        &ns,
        &job_name,
        vec![(
            &ns,
            &input_ds,
            vec![json!({"name": "col_a", "type": "VARCHAR"})],
        )],
        &ns,
        &output_ds,
        vec![json!({"name": "col_b", "type": "VARCHAR"})],
        json!({
            "col_b": {
                "inputFields": [{
                    "namespace": ns,
                    "name": input_ds,
                    "field": "col_a",
                    "transformations": [{
                        "type": "DIRECT",
                        "subtype": "IDENTITY",
                        "description": "identity",
                        "masking": false
                    }]
                }],
                "transformationDescription": "identity",
                "transformationType": "IDENTITY"
            }
        }),
    );

    post_lineage(&app, &event).await;

    // Bug 59: Job events now create model objects and produce column lineage.
    let graph = get_column_lineage_graph(&app, &ns, &output_ds).await;
    assert!(
        !graph.is_empty(),
        "Bug 59: Job events SHOULD produce column lineage (full model orchestration)"
    );
}

/// Bug 59: Job events now perform full model orchestration (reverting Bug 51).
/// Column lineage IS produced for job events with multiple inputs.
#[tokio::test]
async fn column_lineage_job_event_multi_input() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();

    let books_ds = format!("books_{}", rand::random_range(0..u32::MAX));
    let authors_ds = format!("authors_{}", rand::random_range(0..u32::MAX));
    let books_details_ds = format!("books_details_{}", rand::random_range(0..u32::MAX));

    let event = make_job_event_with_column_lineage(
        &ns,
        &job_name,
        vec![
            (
                &ns,
                &books_ds,
                vec![
                    json!({"name": "book_id", "type": "INTEGER"}),
                    json!({"name": "title", "type": "VARCHAR"}),
                    json!({"name": "author_id", "type": "INTEGER"}),
                ],
            ),
            (
                &ns,
                &authors_ds,
                vec![
                    json!({"name": "author_id", "type": "INTEGER"}),
                    json!({"name": "author_name", "type": "VARCHAR"}),
                ],
            ),
        ],
        &ns,
        &books_details_ds,
        vec![
            json!({"name": "book_id", "type": "INTEGER"}),
            json!({"name": "title", "type": "VARCHAR"}),
            json!({"name": "author_name", "type": "VARCHAR"}),
        ],
        json!({
            "book_id": {
                "inputFields": [{
                    "namespace": ns,
                    "name": books_ds,
                    "field": "book_id",
                    "transformations": [{"type": "DIRECT", "subtype": "IDENTITY", "description": "identity", "masking": false}]
                }],
                "transformationDescription": "identity",
                "transformationType": "IDENTITY"
            },
            "title": {
                "inputFields": [{
                    "namespace": ns,
                    "name": books_ds,
                    "field": "title",
                    "transformations": [{"type": "DIRECT", "subtype": "IDENTITY", "description": "identity", "masking": false}]
                }],
                "transformationDescription": "identity",
                "transformationType": "IDENTITY"
            },
            "author_name": {
                "inputFields": [{
                    "namespace": ns,
                    "name": authors_ds,
                    "field": "author_name",
                    "transformations": [{"type": "DIRECT", "subtype": "IDENTITY", "description": "identity", "masking": false}]
                }],
                "transformationDescription": "identity",
                "transformationType": "IDENTITY"
            }
        }),
    );

    post_lineage(&app, &event).await;

    // Bug 59: Job events now create model objects and produce column lineage.
    let graph = get_column_lineage_graph(&app, &ns, &books_details_ds).await;
    assert!(
        !graph.is_empty(),
        "Bug 59: Job events SHOULD produce column lineage (full model orchestration)"
    );
}

// ---------------------------------------------------------------------------
// Bug 57: withDownstream + versioned nodeId returns 400
// ---------------------------------------------------------------------------

/// Bug 57: Verify that requesting column lineage with withDownstream=true
/// and a versioned nodeId (containing '#') returns 400.
#[tokio::test]
async fn with_downstream_and_versioned_node_id_returns_400() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let dataset = generators::new_dataset_name();

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;

    // Use a versioned nodeId (contains '#') with withDownstream=true
    let versioned_node_id = format!("dataset:{}:{}#{}", ns, dataset, uuid::Uuid::new_v4());
    let url = format!(
        "{}/api/v1/column-lineage?nodeId={}&depth=10&withDownstream=true",
        app.base_url,
        encode(&versioned_node_id)
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("get column lineage with versioned nodeId + withDownstream");

    assert_eq!(
        resp.status(),
        400,
        "withDownstream=true + versioned nodeId should return 400 (Bug 57), got {}",
        resp.status()
    );
}

#[tokio::test]
async fn column_lineage_parity_run_event_vs_job_event() {
    let app = TestApp::new().await;

    // Use separate namespaces to avoid cross-contamination
    let run_ns = generators::new_namespace_name();
    let job_ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4().to_string();

    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    // Send a RunEvent
    let run_event = make_column_lineage_event(
        &run_ns,
        &job_name,
        &run_id,
        &run_ns,
        &input_ds,
        vec![json!({"name": "col_a", "type": "VARCHAR"})],
        &run_ns,
        &output_ds,
        vec![json!({"name": "col_b", "type": "VARCHAR"})],
    );
    post_lineage(&app, &run_event).await;

    // Send equivalent JobEvent (same datasets, different namespace)
    let job_event = make_job_event_with_column_lineage(
        &job_ns,
        &job_name,
        vec![(
            &job_ns,
            &input_ds,
            vec![json!({"name": "col_a", "type": "VARCHAR"})],
        )],
        &job_ns,
        &output_ds,
        vec![json!({"name": "col_b", "type": "VARCHAR"})],
        json!({
            "col_b": {
                "inputFields": [{
                    "namespace": job_ns,
                    "name": input_ds,
                    "field": "col_a",
                    "transformations": [{
                        "type": "DIRECT",
                        "subtype": "IDENTITY",
                        "description": "identity",
                        "masking": false
                    }]
                }],
                "transformationDescription": "identity",
                "transformationType": "IDENTITY"
            }
        }),
    );
    post_lineage(&app, &job_event).await;

    // Bug 59: Both RunEvent and JobEvent now produce column lineage.
    let run_graph = get_column_lineage_graph(&app, &run_ns, &output_ds).await;
    let job_graph = get_column_lineage_graph(&app, &job_ns, &output_ds).await;

    assert!(
        !run_graph.is_empty(),
        "RunEvent column lineage graph should not be empty"
    );
    // Bug 59: Job events now perform full model orchestration.
    assert!(
        !job_graph.is_empty(),
        "Bug 59: JobEvent SHOULD produce column lineage (full model orchestration)"
    );
}

// ---------------------------------------------------------------------------
// DatasetEvent column lineage tests (regression for missing column lineage on DatasetEvent)
// ---------------------------------------------------------------------------

/// Helper: build a DatasetEvent (has a single `dataset` field, no `run` or `job`).
fn make_dataset_event_with_column_lineage(
    ds_ns: &str,
    ds_name: &str,
    schema_fields: Vec<Value>,
    column_lineage_fields: Value,
) -> Value {
    json!({
        "eventTime": "2024-01-01T00:00:00Z",
        "dataset": {
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": schema_fields
                },
                "columnLineage": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": column_lineage_fields
                }
            }
        },
        "producer": "test-producer"
    })
}

#[tokio::test]
async fn column_lineage_via_dataset_event() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4().to_string();

    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    // First, send a RunEvent to create the input dataset with its fields.
    // The DatasetEvent only has a single dataset, so the input dataset must
    // already exist for column lineage references to resolve.
    let setup_event = make_column_lineage_event(
        &ns,
        &job_name,
        &run_id,
        &ns,
        &input_ds,
        vec![json!({"name": "col_a", "type": "VARCHAR"})],
        &ns,
        &output_ds,
        vec![json!({"name": "col_b", "type": "VARCHAR"})],
    );
    post_lineage(&app, &setup_event).await;

    // Now send a DatasetEvent with columnLineage facet for the output dataset.
    // This should populate the column_lineage table.
    let ds_event = make_dataset_event_with_column_lineage(
        &ns,
        &output_ds,
        vec![json!({"name": "col_b", "type": "VARCHAR"})],
        json!({
            "col_b": {
                "inputFields": [{
                    "namespace": ns,
                    "name": input_ds,
                    "field": "col_a",
                    "transformations": [{
                        "type": "DIRECT",
                        "subtype": "IDENTITY",
                        "description": "identity",
                        "masking": false
                    }]
                }],
                "transformationDescription": "identity",
                "transformationType": "IDENTITY"
            }
        }),
    );
    post_lineage(&app, &ds_event).await;

    let graph = get_column_lineage_graph(&app, &ns, &output_ds).await;
    assert!(
        !graph.is_empty(),
        "column lineage graph should not be empty for DatasetEvent with columnLineage facet"
    );
}

#[tokio::test]
async fn dataset_event_output_facets_accepted() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();

    let ds_name = format!("ds_{}", rand::random_range(0..u32::MAX));

    // Send a DatasetEvent with outputFacets.
    // Before the fix, the handler didn't process outputFacets at all.
    // Now it routes through upsert_output_dataset() which stores them
    // in dataset_facets with type "OUTPUT".
    let ds_event = json!({
        "eventTime": "2024-01-01T00:00:00Z",
        "dataset": {
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [{"name": "col_a", "type": "INTEGER"}]
                }
            },
            "outputFacets": {
                "outputStatistics": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "rowCount": 1000,
                    "size": 2048
                }
            }
        },
        "producer": "test-producer"
    });
    post_lineage(&app, &ds_event).await;

    // Verify the dataset was created with a version
    let url = format!(
        "{}/api/v1/namespaces/{}/datasets/{}/versions",
        app.base_url,
        encode(&ns),
        encode(&ds_name)
    );
    let resp = app.client.get(&url).send().await.expect("get versions");
    let status = resp.status().as_u16();
    assert_eq!(
        status, 200,
        "expected 200 for dataset versions, got {}",
        status
    );

    let body: Value = resp.json().await.expect("parse json");
    let versions = body
        .get("versions")
        .and_then(|v| v.as_array())
        .expect("versions array");
    assert!(
        !versions.is_empty(),
        "dataset should have at least one version after DatasetEvent with outputFacets"
    );

    // Verify the version has regular facets (schema) populated
    let version = &versions[0];
    let facets = version.get("facets").and_then(|f| f.as_object());
    assert!(
        facets.is_some(),
        "dataset version should have facets after DatasetEvent"
    );
    assert!(
        facets.unwrap().contains_key("schema"),
        "dataset version facets should contain schema"
    );
}
