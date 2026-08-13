use chrono::Utc;

use crate::generators;
use crate::TestApp;
use serde_json::{json, Value};
use uuid::Uuid;

/// Percent-encode a value for use as a single URL path segment.
fn encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
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

/// Helper: GET a job by namespace and name.
async fn get_job(app: &TestApp, ns: &str, name: &str) -> Value {
    let url = format!(
        "{}/api/v1/namespaces/{}/jobs/{}",
        app.base_url,
        encode(ns),
        encode(name)
    );
    let resp = app.client.get(&url).send().await.expect("get job");
    assert_eq!(resp.status(), 200, "get job failed: {}", resp.status());
    resp.json().await.expect("parse job json")
}

/// Helper: GET a dataset by namespace and name.
async fn get_dataset(app: &TestApp, ns: &str, name: &str) -> Value {
    let url = format!(
        "{}/api/v1/namespaces/{}/datasets/{}",
        app.base_url,
        encode(ns),
        encode(name)
    );
    let resp = app.client.get(&url).send().await.expect("get dataset");
    assert_eq!(resp.status(), 200, "get dataset failed: {}", resp.status());
    resp.json().await.expect("parse dataset json")
}

/// Helper: GET run by UUID.
async fn get_run(app: &TestApp, run_id: Uuid) -> Value {
    let url = format!("{}/api/v1/jobs/runs/{}", app.base_url, run_id);
    let resp = app.client.get(&url).send().await.expect("get run");
    assert_eq!(resp.status(), 200, "get run failed: {}", resp.status());
    resp.json().await.expect("parse run json")
}

/// Helper: build a run event JSON.
#[allow(clippy::too_many_arguments)]
fn make_run_event(
    event_type: &str,
    ns: &str,
    job_name: &str,
    run_id: &str,
    inputs: Vec<Value>,
    outputs: Vec<Value>,
    run_facets: Option<Value>,
    job_facets: Option<Value>,
) -> Value {
    let mut event = json!({
        "eventType": event_type,
        "eventTime": "2024-01-01T00:00:00Z",
        "run": {
            "runId": run_id
        },
        "job": {
            "namespace": ns,
            "name": job_name
        },
        "inputs": inputs,
        "outputs": outputs,
        "producer": "test-producer"
    });
    if let Some(rf) = run_facets {
        event["run"]["facets"] = rf;
    }
    if let Some(jf) = job_facets {
        event["job"]["facets"] = jf;
    }
    event
}

// ---------------------------------------------------------------------------
// Wave 0: Parent Job Hierarchy Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn airflow_parent_job_hierarchy() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let dag_name = format!("dag_{}", rand::random_range(0..u32::MAX));
    let task_name = format!("{}.task1", dag_name);
    let run_id = Uuid::new_v4().to_string();
    let parent_run_id = Uuid::new_v4().to_string();

    let parent_facet = json!({
        "parent": {
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id },
            "job": { "namespace": ns, "name": task_name }
        }
    });

    let event = make_run_event(
        "START",
        &ns,
        &task_name,
        &run_id,
        vec![],
        vec![],
        Some(parent_facet),
        None,
    );

    post_lineage(&app, &event).await;

    // Parent job should exist with the dag_name (stripped from task name)
    let parent_job = get_job(&app, &ns, &dag_name).await;
    assert_eq!(parent_job["name"], dag_name);

    // Child job should exist with FQN = dag_name.task1
    let child_job = get_job(&app, &ns, &task_name).await;
    assert_eq!(child_job["name"], task_name);
}

#[tokio::test]
async fn parent_run_facet_alias() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let dag_name = format!("dag_{}", rand::random_range(0..u32::MAX));
    let task_name = format!("{}.task_alias", dag_name);
    let run_id = Uuid::new_v4().to_string();
    let parent_run_id = Uuid::new_v4().to_string();

    // Use `parentRun` key instead of `parent`
    let parent_facet = json!({
        "parentRun": {
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id },
            "job": { "namespace": ns, "name": task_name }
        }
    });

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &task_name,
        &run_id,
        vec![],
        vec![],
        Some(parent_facet),
        None,
    );

    post_lineage(&app, &event).await;

    // Parent job should be created even with `parentRun` alias
    let parent_job = get_job(&app, &ns, &dag_name).await;
    assert_eq!(parent_job["name"], dag_name);
}

#[tokio::test]
async fn parent_on_start_event_only() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let dag_name = format!("dag_{}", rand::random_range(0..u32::MAX));
    let task_name = format!("{}.task_start_only", dag_name);
    let run_id = Uuid::new_v4().to_string();
    let parent_run_id = Uuid::new_v4().to_string();

    // START event with parent facet
    let parent_facet = json!({
        "parent": {
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id },
            "job": { "namespace": ns, "name": task_name }
        }
    });

    let start_event = make_run_event(
        "START",
        &ns,
        &task_name,
        &run_id,
        vec![],
        vec![],
        Some(parent_facet),
        None,
    );
    post_lineage(&app, &start_event).await;

    // COMPLETE event WITHOUT parent facet
    let complete_event = make_run_event(
        "COMPLETE",
        &ns,
        &task_name,
        &run_id,
        vec![],
        vec![],
        None,
        None,
    );
    post_lineage(&app, &complete_event).await;

    // Parent job should still exist from the START event
    let parent_job = get_job(&app, &ns, &dag_name).await;
    assert_eq!(parent_job["name"], dag_name);
}

#[tokio::test]
async fn old_airflow_non_uuid_parent_run_id() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let dag_name = format!("dag_{}", rand::random_range(0..u32::MAX));
    let task_name = format!("{}.task_old", dag_name);
    let run_id = Uuid::new_v4().to_string();
    // Non-UUID run ID (old Airflow style)
    let parent_run_id = "scheduled__2022-04-15T00:00:00+00:00";

    let parent_facet = json!({
        "parent": {
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id },
            "job": { "namespace": ns, "name": task_name }
        }
    });

    let event = make_run_event(
        "START",
        &ns,
        &task_name,
        &run_id,
        vec![],
        vec![],
        Some(parent_facet),
        None,
    );

    // Should succeed — non-UUID parent run ID gets converted to v5 UUID
    post_lineage(&app, &event).await;

    let parent_job = get_job(&app, &ns, &dag_name).await;
    assert_eq!(parent_job["name"], dag_name);
}

// ---------------------------------------------------------------------------
// Wave 1: DatasetEvent + JobEvent Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn send_dataset_event() {
    let app = TestApp::new().await;
    let ns = format!("ds_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("ds_{}", rand::random_range(0..u32::MAX));

    let event = json!({
        "eventTime": "2024-06-01T12:00:00Z",
        "dataset": {
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        { "name": "id", "type": "INTEGER" },
                        { "name": "name", "type": "VARCHAR" }
                    ]
                }
            }
        },
        "producer": "test-producer",
        "schemaURL": "https://openlineage.io/spec/2-0-0/OpenLineage.json#/definitions/DatasetEvent"
    });

    post_lineage(&app, &event).await;

    // Verify dataset exists and has fields
    let dataset = get_dataset(&app, &ns, &ds_name).await;
    assert_eq!(dataset["name"], ds_name);

    // Verify fields
    let fields = dataset["fields"].as_array();
    assert!(fields.is_some(), "dataset should have fields");
    let fields = fields.unwrap();
    assert_eq!(fields.len(), 2);
}

#[tokio::test]
async fn send_job_event() {
    let app = TestApp::new().await;
    let ns = format!("job_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("job_{}", rand::random_range(0..u32::MAX));
    let input_ns = format!("input_ns_{}", rand::random_range(0..u32::MAX));
    let output_ns = format!("output_ns_{}", rand::random_range(0..u32::MAX));

    let event = json!({
        "eventType": "COMPLETE",
        "eventTime": "2024-06-01T12:00:00Z",
        "job": {
            "namespace": ns,
            "name": job_name
        },
        "inputs": [{
            "namespace": input_ns,
            "name": "input_table"
        }],
        "outputs": [{
            "namespace": output_ns,
            "name": "output_table"
        }],
        "producer": "test-producer"
    });

    post_lineage(&app, &event).await;

    // Bug 59: Job events now perform full model orchestration.
    // Verify job IS created (reverting Bug 51).
    let url = format!(
        "{}/api/v1/namespaces/{}/jobs/{}",
        app.base_url,
        encode(&ns),
        encode(&job_name)
    );
    let resp = app.client.get(&url).send().await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "Bug 59: Job event SHOULD create a job row (full model orchestration)"
    );
}

// ---------------------------------------------------------------------------
// Wave 2: Fixture-Based Round-Trip Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn send_event_full_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_full.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;

    // Verify the job was created
    let job = get_job(&app, "my-scheduler-namespace", "myjob.mytask").await;
    assert_eq!(job["name"], "myjob.mytask");
}

#[tokio::test]
async fn send_event_simple_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_simple.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;
}

#[tokio::test]
async fn send_event_required_only_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_required_only.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;
}

#[tokio::test]
async fn send_event_unicode_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_unicode.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;
}

#[tokio::test]
async fn send_event_namespace_naming_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_namespace_naming.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;
}

#[tokio::test]
async fn send_event_null_nominal_end_time_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/null_nominal_end_time.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;
}

#[tokio::test]
async fn send_event_without_schema_url_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_without_schema_url.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;
}

#[tokio::test]
async fn send_event_dataset_event_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_dataset_event.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");
    post_lineage(&app, &event).await;

    // Verify the dataset was created via model updating
    let ds = get_dataset(&app, "my-dataset-namespace", "my-dataset-name").await;
    assert_eq!(ds["name"], "my-dataset-name");

    // Verify schema field was created
    let fields = ds["fields"].as_array();
    assert!(fields.is_some(), "dataset should have fields");
    assert_eq!(fields.unwrap().len(), 1);
}

#[tokio::test]
async fn send_event_job_event_fixture() {
    let app = TestApp::new().await;
    let fixture = include_str!("../../fixtures/event_job_event.json");
    let event: Value = serde_json::from_str(fixture).expect("parse fixture");

    // Bug 59: Job events now perform full model orchestration.
    // The POST should succeed and create model objects.
    let url = format!("{}/api/v1/lineage", app.base_url);
    let resp = app.client.post(&url).json(&event).send().await.unwrap();
    assert!(
        resp.status().is_success(),
        "Job event fixture should be accepted, got {}",
        resp.status()
    );

    // Verify the job was created (Bug 59: full model orchestration)
    let job = get_job(&app, "my-scheduler-namespace", "myjob").await;
    assert_eq!(job["name"], "myjob");
}

// ---------------------------------------------------------------------------
// Run State Machine Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_lifecycle_start_complete() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    // Send START event
    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        None,
    );
    post_lineage(&app, &start_event).await;

    // Verify run is RUNNING
    let run = get_run(&app, run_id).await;
    assert_eq!(run["state"], "RUNNING");
    assert!(
        run["startedAt"].as_str().is_some(),
        "startedAt should be set"
    );

    // Send COMPLETE event
    let complete_event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        None,
    );
    post_lineage(&app, &complete_event).await;

    // Verify run is COMPLETED
    let run = get_run(&app, run_id).await;
    assert_eq!(run["state"], "COMPLETED");
    assert!(run["endedAt"].as_str().is_some(), "endedAt should be set");
}

#[tokio::test]
async fn run_lifecycle_start_fail() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        None,
    );
    post_lineage(&app, &start_event).await;

    let fail_event = make_run_event(
        "FAIL",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        None,
    );
    post_lineage(&app, &fail_event).await;

    let run = get_run(&app, run_id).await;
    assert_eq!(run["state"], "FAILED");
    assert!(run["endedAt"].as_str().is_some(), "endedAt should be set");
}

// ---------------------------------------------------------------------------
// Event Listing / Sorting Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn events_sorted_desc() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    // Post START and then COMPLETE events with recent timestamps
    let now = Utc::now();
    let t1 = (now - chrono::Duration::minutes(10)).to_rfc3339();
    let t2 = (now - chrono::Duration::minutes(5)).to_rfc3339();
    let start_event = json!({
        "eventType": "START",
        "eventTime": t1,
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [],
        "producer": "test"
    });
    post_lineage(&app, &start_event).await;

    let complete_event = json!({
        "eventType": "COMPLETE",
        "eventTime": t2,
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [],
        "producer": "test"
    });
    post_lineage(&app, &complete_event).await;

    // List events DESC (default)
    let url = format!("{}/api/v1/events/lineage?sortDirection=DESC", app.base_url);
    let resp = app.client.get(&url).send().await.expect("list events");
    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    let results = body["events"].as_array().unwrap();
    assert!(results.len() >= 2, "should have at least 2 events");
}

#[tokio::test]
async fn events_sorted_asc() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let now = Utc::now();
    let t1 = (now - chrono::Duration::minutes(10)).to_rfc3339();
    let t2 = (now - chrono::Duration::minutes(5)).to_rfc3339();
    let start_event = json!({
        "eventType": "START",
        "eventTime": t1,
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [],
        "producer": "test"
    });
    post_lineage(&app, &start_event).await;

    let complete_event = json!({
        "eventType": "COMPLETE",
        "eventTime": t2,
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [],
        "producer": "test"
    });
    post_lineage(&app, &complete_event).await;

    // List events ASC
    let url = format!("{}/api/v1/events/lineage?sortDirection=ASC", app.base_url);
    let resp = app.client.get(&url).send().await.expect("list events");
    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    let results = body["events"].as_array().unwrap();
    assert!(results.len() >= 2, "should have at least 2 events");
}

// ---------------------------------------------------------------------------
// Facet-Driven Type Derivation Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_type_from_facets() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let job_facets = json!({
        "jobType": {
            "_producer": "test",
            "_schemaURL": "test",
            "processingType": "STREAMING",
            "integration": "FLINK",
            "jobType": "JOB"
        }
    });

    let event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        Some(job_facets),
    );
    post_lineage(&app, &event).await;

    let job = get_job(&app, &ns, &job_name).await;
    assert_eq!(job["type"], "STREAM");
}

#[tokio::test]
async fn datasource_facet_creates_source() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();
    let ds_ns = format!("ds_ns_{}", rand::random_range(0..u32::MAX));

    let event = json!({
        "eventType": "COMPLETE",
        "eventTime": "2024-01-01T00:00:00Z",
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [{
            "namespace": ds_ns,
            "name": "output_table",
            "facets": {
                "dataSource": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "name": "my_postgres",
                    "uri": "postgresql://host:5432/mydb"
                }
            }
        }],
        "producer": "test"
    });
    post_lineage(&app, &event).await;

    // Verify dataset was created
    let ds = get_dataset(&app, &ds_ns, "output_table").await;
    assert_eq!(ds["name"], "output_table");
    // Source should be "my_postgres" with type POSTGRESQL
    assert_eq!(ds["sourceName"], "my_postgres");
}

// ---------------------------------------------------------------------------
// Lifecycle State DROP Tests (Bug 1)
// ---------------------------------------------------------------------------

/// A dataset with `lifecycleStateChange: DROP` facet should be soft-deleted.
#[tokio::test]
async fn lifecycle_drop_marks_dataset_deleted() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();
    let ds_ns = format!("ds_drop_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("ds_drop_{}", rand::random_range(0..u32::MAX));

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "lifecycleStateChange": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "lifecycleStateChange": "DROP"
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event).await;

    let ds = get_dataset(&app, &ds_ns, &ds_name).await;
    assert_eq!(ds["name"], ds_name);
    assert_eq!(
        ds["deleted"], true,
        "Dataset with DROP lifecycle should be soft-deleted"
    );
    assert_eq!(
        ds["lastLifecycleState"], "DROP",
        "Dataset with DROP lifecycle should have lastLifecycleState='DROP'"
    );
}

/// A dataset with a non-DROP lifecycle state should NOT be deleted.
#[tokio::test]
async fn lifecycle_non_drop_does_not_delete() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();
    let ds_ns = format!("ds_nodrop_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("ds_nodrop_{}", rand::random_range(0..u32::MAX));

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "lifecycleStateChange": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "lifecycleStateChange": "ALTER"
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event).await;

    let ds = get_dataset(&app, &ds_ns, &ds_name).await;
    assert_eq!(
        ds["deleted"], false,
        "Dataset with ALTER lifecycle should NOT be soft-deleted"
    );
    assert_eq!(
        ds["lastLifecycleState"], "ALTER",
        "Dataset with ALTER lifecycle should have lastLifecycleState='ALTER'"
    );
}

/// A dataset created without a lifecycleStateChange facet should have
/// lastLifecycleState present as an empty string.
#[tokio::test]
async fn lifecycle_state_default_without_facet() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();
    let ds_ns = format!("ds_nolc_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("ds_nolc_{}", rand::random_range(0..u32::MAX));

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
        })],
        None,
        None,
    );
    post_lineage(&app, &event).await;

    let ds = get_dataset(&app, &ds_ns, &ds_name).await;
    assert_eq!(ds["name"], ds_name);
    assert!(
        ds.get("lastLifecycleState").is_some(),
        "lastLifecycleState field should be present even without lifecycle facet"
    );
    assert_eq!(
        ds["lastLifecycleState"], "",
        "lastLifecycleState should be empty string when no lifecycle facet is sent"
    );
}

/// The LIST datasets endpoint uses a different code path
/// (find_lifecycle_states_batch + HashMap) than single GET. Verify
/// lastLifecycleState is populated there too.
#[tokio::test]
async fn lifecycle_state_visible_in_list_endpoint() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();
    let ds_ns = format!("ds_list_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("ds_list_{}", rand::random_range(0..u32::MAX));

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "lifecycleStateChange": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "lifecycleStateChange": "ALTER"
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event).await;

    // Use the list endpoint
    let url = format!(
        "{}/api/v1/namespaces/{}/datasets",
        app.base_url,
        encode(&ds_ns)
    );
    let resp = app.client.get(&url).send().await.expect("list datasets");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    let datasets = body["datasets"].as_array().expect("datasets array");

    let ds = datasets
        .iter()
        .find(|d| d["name"] == ds_name)
        .expect("dataset should appear in list");
    assert_eq!(
        ds["lastLifecycleState"], "ALTER",
        "lastLifecycleState should be 'ALTER' in list endpoint"
    );
}

/// Tests state progression: no facet → ALTER → DROP.
#[tokio::test]
async fn lifecycle_state_transitions() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let ds_ns = format!("ds_trans_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("ds_trans_{}", rand::random_range(0..u32::MAX));

    // Step 1: event without lifecycle facet
    let run_id1 = Uuid::new_v4();
    let event1 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id1.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
        })],
        None,
        None,
    );
    post_lineage(&app, &event1).await;

    let ds = get_dataset(&app, &ds_ns, &ds_name).await;
    assert_eq!(ds["lastLifecycleState"], "", "Step 1: should be empty");
    assert_eq!(ds["deleted"], false, "Step 1: should not be deleted");

    // Step 2: event with ALTER lifecycle
    let run_id2 = Uuid::new_v4();
    let event2 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id2.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "lifecycleStateChange": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "lifecycleStateChange": "ALTER"
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event2).await;

    let ds = get_dataset(&app, &ds_ns, &ds_name).await;
    assert_eq!(ds["lastLifecycleState"], "ALTER", "Step 2: should be ALTER");
    assert_eq!(ds["deleted"], false, "Step 2: should not be deleted");

    // Step 3: event with DROP lifecycle
    let run_id3 = Uuid::new_v4();
    let event3 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id3.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "lifecycleStateChange": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "lifecycleStateChange": "DROP"
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event3).await;

    let ds = get_dataset(&app, &ds_ns, &ds_name).await;
    assert_eq!(ds["lastLifecycleState"], "DROP", "Step 3: should be DROP");
    assert_eq!(ds["deleted"], true, "Step 3: should be deleted");
}

/// Regression guard: verify all expected top-level fields are present in a
/// dataset response created via an OL event with schema + lifecycle facets.
#[tokio::test]
async fn dataset_response_field_completeness() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();
    let ds_ns = format!("ds_fields_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("ds_fields_{}", rand::random_range(0..u32::MAX));

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        { "name": "col_a", "type": "VARCHAR" },
                        { "name": "col_b", "type": "INTEGER" }
                    ]
                },
                "lifecycleStateChange": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "lifecycleStateChange": "CREATE"
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event).await;

    let ds = get_dataset(&app, &ds_ns, &ds_name).await;

    // All expected top-level fields must be present
    let expected_fields = [
        "id",
        "type",
        "name",
        "physicalName",
        "createdAt",
        "updatedAt",
        "namespace",
        "sourceName",
        "fields",
        "tags",
        "lastLifecycleState",
        "currentVersion",
        "facets",
        "deleted",
    ];
    for field in &expected_fields {
        assert!(
            ds.get(field).is_some(),
            "missing field '{}' in dataset response",
            field
        );
    }

    // Verify specific values
    assert_eq!(
        ds["lastLifecycleState"], "CREATE",
        "lastLifecycleState should be 'CREATE'"
    );
    assert_eq!(ds["deleted"], false, "CREATE lifecycle should not delete");

    let fields = ds["fields"].as_array().expect("fields should be an array");
    assert_eq!(fields.len(), 2, "should have 2 schema fields");

    // currentVersion should be a valid UUID string
    let cv = ds["currentVersion"]
        .as_str()
        .expect("currentVersion should be a string");
    assert!(
        Uuid::parse_str(cv).is_ok(),
        "currentVersion '{}' should be a valid UUID",
        cv
    );
}

// ---------------------------------------------------------------------------
// IO Mapping Cleanup Tests (Bug 2)
// ---------------------------------------------------------------------------

/// Helper: GET lineage graph from a job node and return the graph array.
async fn get_lineage_graph(app: &TestApp, ns: &str, job_name: &str) -> Vec<Value> {
    let node_id = format!("job:{}:{}", ns, job_name);
    let url = format!(
        "{}/api/v1/lineage?nodeId={}&depth=10",
        app.base_url, node_id
    );
    let resp = app.client.get(&url).send().await.expect("get lineage");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    body["graph"].as_array().cloned().unwrap_or_default()
}

/// When a job sends a second event with no inputs, the lineage graph should
/// no longer show the old input dataset.
#[tokio::test]
async fn io_mapping_cleanup_removes_stale_inputs() {
    let app = TestApp::new().await;
    let ns = format!("io_cleanup_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = generators::new_job_name();
    let run_id1 = Uuid::new_v4();
    let input_ds = format!("input_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_{}", rand::random_range(0..u32::MAX));

    // First COMPLETE event: job reads input_ds, writes output_ds
    let event1 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id1.to_string(),
        vec![json!({ "namespace": ns, "name": input_ds })],
        vec![json!({ "namespace": ns, "name": output_ds })],
        None,
        None,
    );
    post_lineage(&app, &event1).await;

    // Verify lineage has 3 nodes: job + input + output
    let graph1 = get_lineage_graph(&app, &ns, &job_name).await;
    assert_eq!(
        graph1.len(),
        3,
        "Expected 3 nodes (job + input + output), got {}",
        graph1.len()
    );

    // Second COMPLETE event: same job, NO inputs, same output
    let run_id2 = Uuid::new_v4();
    let event2 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id2.to_string(),
        vec![],
        vec![json!({ "namespace": ns, "name": output_ds })],
        None,
        None,
    );
    post_lineage(&app, &event2).await;

    // Now the lineage graph should only have 2 nodes: job + output
    // (input_ds should be gone since no inputs were provided)
    let graph2 = get_lineage_graph(&app, &ns, &job_name).await;
    assert_eq!(
        graph2.len(),
        2,
        "Expected 2 nodes (job + output) after clearing inputs, got {}. Graph: {:?}",
        graph2.len(),
        graph2
    );
}

// ---------------------------------------------------------------------------
// Streaming Job Version Tests (Bug 3)
// ---------------------------------------------------------------------------

/// Helper: GET job versions count for a job.
async fn get_job_versions_count(app: &TestApp, ns: &str, job_name: &str) -> i64 {
    let url = format!(
        "{}/api/v1/namespaces/{}/jobs/{}/versions",
        app.base_url,
        encode(ns),
        encode(job_name)
    );
    let resp = app.client.get(&url).send().await.expect("get job versions");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    // API returns { "versions": [...] } — count the array length
    body["versions"]
        .as_array()
        .map(|a| a.len() as i64)
        .unwrap_or(0)
}

/// Streaming jobs should only create one job version despite multiple events.
#[tokio::test]
async fn streaming_job_single_version() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let job_facets = json!({
        "jobType": {
            "_producer": "test",
            "_schemaURL": "test",
            "processingType": "STREAMING",
            "integration": "FLINK",
            "jobType": "JOB"
        }
    });

    // Send START event
    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![json!({ "namespace": ns, "name": "input_stream" })],
        vec![json!({ "namespace": ns, "name": "output_stream" })],
        None,
        Some(job_facets.clone()),
    );
    post_lineage(&app, &start_event).await;

    // Send RUNNING event (no datasets — streaming job sending heartbeat)
    let running_event = make_run_event(
        "RUNNING",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        Some(job_facets.clone()),
    );
    post_lineage(&app, &running_event).await;

    // Send another RUNNING event
    post_lineage(&app, &running_event).await;

    // Should only have 1 job version despite 3 events
    let version_count = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(
        version_count, 1,
        "Streaming job should have exactly 1 version, got {}",
        version_count
    );
}

/// Batch jobs should NOT create a job version on START, only on COMPLETE.
#[tokio::test]
async fn batch_job_version_gating() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    // Send START event
    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![json!({ "namespace": ns, "name": "batch_input" })],
        vec![json!({ "namespace": ns, "name": "batch_output" })],
        None,
        None,
    );
    post_lineage(&app, &start_event).await;

    // No job version should exist yet (START doesn't create versions for batch)
    let version_count_after_start = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(
        version_count_after_start, 0,
        "Batch job should have 0 versions after START, got {}",
        version_count_after_start
    );

    // Send COMPLETE event
    let complete_event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![json!({ "namespace": ns, "name": "batch_input" })],
        vec![json!({ "namespace": ns, "name": "batch_output" })],
        None,
        None,
    );
    post_lineage(&app, &complete_event).await;

    // Now should have exactly 1 job version
    let version_count_after_complete = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(
        version_count_after_complete, 1,
        "Batch job should have 1 version after COMPLETE, got {}",
        version_count_after_complete
    );
}

/// Streaming START with no datasets should still create a job version
/// so that latestRun is populated in the lineage graph.
/// Regression test for the overly broad `is_streaming_event_no_datasets` guard.
#[tokio::test]
async fn streaming_start_no_datasets_creates_version() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let job_facets = json!({
        "jobType": {
            "_producer": "test",
            "_schemaURL": "test",
            "processingType": "STREAMING",
            "integration": "SPARK",
            "jobType": "JOB"
        }
    });

    // Send START with no datasets (common for Spark streaming child tasks)
    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        Some(job_facets.clone()),
    );
    post_lineage(&app, &start_event).await;

    // A job version should exist so latestRun can be resolved
    let version_count = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(
        version_count, 1,
        "Streaming START with no datasets should create a version, got {}",
        version_count
    );

    // Lineage graph should show latestRun
    let graph = get_lineage_graph(&app, &ns, &job_name).await;
    let job_node = graph
        .iter()
        .find(|n| n["type"] == "JOB")
        .expect("job node in lineage");
    assert!(
        !job_node["data"]["latestRun"].is_null(),
        "latestRun should be populated for streaming START, got null"
    );
}

/// Streaming COMPLETE with no datasets should NOT create a job version
/// (matches Java's isTerminalEventForStreamingJobWithNoDatasets guard).
#[tokio::test]
async fn streaming_complete_no_datasets_skips_version() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let job_facets = json!({
        "jobType": {
            "_producer": "test",
            "_schemaURL": "test",
            "processingType": "STREAMING",
            "integration": "SPARK",
            "jobType": "JOB"
        }
    });

    // Send only a COMPLETE event with no datasets
    let complete_event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        Some(job_facets.clone()),
    );
    post_lineage(&app, &complete_event).await;

    let version_count = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(
        version_count, 0,
        "Streaming COMPLETE with no datasets should NOT create a version, got {}",
        version_count
    );
}

/// Streaming RUNNING heartbeat should not clear IO mappings from a prior event.
#[tokio::test]
async fn streaming_heartbeat_preserves_io_mappings() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let job_facets = json!({
        "jobType": {
            "_producer": "test",
            "_schemaURL": "test",
            "processingType": "STREAMING",
            "integration": "FLINK",
            "jobType": "JOB"
        }
    });

    // START with datasets
    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![json!({ "namespace": ns, "name": "stream_in" })],
        vec![json!({ "namespace": ns, "name": "stream_out" })],
        None,
        Some(job_facets.clone()),
    );
    post_lineage(&app, &start_event).await;

    // RUNNING heartbeat with no datasets
    let heartbeat = make_run_event(
        "RUNNING",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        Some(job_facets.clone()),
    );
    post_lineage(&app, &heartbeat).await;

    // Lineage should still show the datasets from START
    let graph = get_lineage_graph(&app, &ns, &job_name).await;
    let job_node = graph.iter().find(|n| n["type"] == "JOB").expect("job node");
    let inputs = job_node["data"]["inputs"].as_array().expect("inputs array");
    let outputs = job_node["data"]["outputs"]
        .as_array()
        .expect("outputs array");

    assert_eq!(
        inputs.len(),
        1,
        "Heartbeat should not clear inputs, got {:?}",
        inputs
    );
    assert_eq!(
        outputs.len(),
        1,
        "Heartbeat should not clear outputs, got {:?}",
        outputs
    );
}

// ---------------------------------------------------------------------------
// Bug 4: Run Args Persistence Tests
// ---------------------------------------------------------------------------

/// A COMPLETE event with nominalTime facet should persist run args.
#[tokio::test]
async fn run_args_persisted_from_nominal_time_facet() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();

    let run_facets = json!({
        "nominalTime": {
            "_producer": "test",
            "_schemaURL": "test",
            "nominalStartTime": "2024-01-01T00:00:00Z",
            "nominalEndTime": "2024-01-01T01:00:00Z"
        }
    });

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        Some(run_facets),
        None,
    );
    post_lineage(&app, &event).await;

    let run = get_run(&app, run_id).await;
    // Run args should contain the nominal times
    let args = &run["args"];
    assert!(
        !args.is_null(),
        "Run args should be populated from nominalTime facet"
    );
    let args_map: Value = if args.is_string() {
        serde_json::from_str(args.as_str().unwrap()).unwrap_or_default()
    } else {
        args.clone()
    };
    assert_eq!(
        args_map["nominal_start_time"], "2024-01-01T00:00:00Z",
        "nominal_start_time should be in run args"
    );
    assert_eq!(
        args_map["nominal_end_time"], "2024-01-01T01:00:00Z",
        "nominal_end_time should be in run args"
    );
}

/// A COMPLETE event with parent facet should persist parent info in run args.
#[tokio::test]
async fn run_args_persisted_from_parent_facet() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let dag_name = format!("dag_{}", rand::random_range(0..u32::MAX));
    let task_name = format!("{}.task_args", dag_name);
    let run_id = Uuid::new_v4();
    let parent_run_id = Uuid::new_v4().to_string();

    let run_facets = json!({
        "parent": {
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id },
            "job": { "namespace": ns, "name": task_name }
        }
    });

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &task_name,
        &run_id.to_string(),
        vec![],
        vec![],
        Some(run_facets),
        None,
    );
    post_lineage(&app, &event).await;

    let run = get_run(&app, run_id).await;
    let args = &run["args"];
    assert!(
        !args.is_null(),
        "Run args should be populated from parent facet"
    );
    let args_map: Value = if args.is_string() {
        serde_json::from_str(args.as_str().unwrap()).unwrap_or_default()
    } else {
        args.clone()
    };
    assert_eq!(
        args_map["run_id"], parent_run_id,
        "parent run_id should be in run args"
    );
}

// ---------------------------------------------------------------------------
// Bug 5: Symlinks Facet Processing Tests
// ---------------------------------------------------------------------------

/// A dataset with symlinks facet should create alternate symlinks.
#[tokio::test]
async fn symlinks_facet_creates_alternate_names() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let run_id = Uuid::new_v4();
    let ds_ns = format!("sym_ns_{}", rand::random_range(0..u32::MAX));
    let ds_name = format!("sym_primary_{}", rand::random_range(0..u32::MAX));
    let alt_ns = format!("sym_alt_ns_{}", rand::random_range(0..u32::MAX));
    let alt_name = format!("sym_alt_{}", rand::random_range(0..u32::MAX));

    let event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![json!({
            "namespace": ds_ns,
            "name": ds_name,
            "facets": {
                "symlinks": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "identifiers": [
                        {
                            "namespace": alt_ns,
                            "name": alt_name,
                            "type": "TABLE"
                        }
                    ]
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event).await;

    // Primary dataset should exist
    let ds = get_dataset(&app, &ds_ns, &ds_name).await;
    assert_eq!(ds["name"], ds_name);

    // Alternate symlink should also resolve to the same dataset
    let alt_ds = get_dataset(&app, &alt_ns, &alt_name).await;
    assert_eq!(
        alt_ds["name"], alt_name,
        "Alternate symlink dataset should be findable"
    );
}

// ---------------------------------------------------------------------------
// Bug 7: Airflow Parent Run UUID Conflict Tests
// ---------------------------------------------------------------------------

/// Two events with the same parent run UUID but different parent job namespaces
/// should result in separate parent runs (no data corruption).
#[tokio::test]
async fn airflow_parent_uuid_conflict_resolution() {
    let app = TestApp::new().await;
    let ns1 = generators::new_namespace_name();
    let ns2 = generators::new_namespace_name();
    let dag1 = format!("dag_{}", rand::random_range(0..u32::MAX));
    let dag2 = format!("dag_{}", rand::random_range(0..u32::MAX));
    let task1 = format!("{}.task1", dag1);
    let task2 = format!("{}.task2", dag2);
    let run_id1 = Uuid::new_v4();
    let run_id2 = Uuid::new_v4();
    // Same parent run UUID — simulates old Airflow bug
    let shared_parent_run_id = Uuid::new_v4().to_string();

    // First event: task1 in ns1 with parent dag1 in ns1
    let event1 = make_run_event(
        "COMPLETE",
        &ns1,
        &task1,
        &run_id1.to_string(),
        vec![],
        vec![],
        Some(json!({
            "parent": {
                "_producer": "test",
                "_schemaURL": "test",
                "run": { "runId": shared_parent_run_id },
                "job": { "namespace": ns1, "name": task1 }
            }
        })),
        None,
    );
    post_lineage(&app, &event1).await;

    // Second event: task2 in ns2 with SAME parent run UUID but different parent job
    let event2 = make_run_event(
        "COMPLETE",
        &ns2,
        &task2,
        &run_id2.to_string(),
        vec![],
        vec![],
        Some(json!({
            "parent": {
                "_producer": "test",
                "_schemaURL": "test",
                "run": { "runId": shared_parent_run_id },
                "job": { "namespace": ns2, "name": task2 }
            }
        })),
        None,
    );
    post_lineage(&app, &event2).await;

    // Both parent jobs should exist
    let parent1 = get_job(&app, &ns1, &dag1).await;
    assert_eq!(parent1["name"], dag1);

    let parent2 = get_job(&app, &ns2, &dag2).await;
    assert_eq!(parent2["name"], dag2);

    // Both child jobs should exist
    let child1 = get_job(&app, &ns1, &task1).await;
    assert_eq!(child1["name"], task1);

    let child2 = get_job(&app, &ns2, &task2).await;
    assert_eq!(child2["name"], task2);
}

// ---------------------------------------------------------------------------
// Bug 8: Dataset Version UUID Determinism
// ---------------------------------------------------------------------------

/// Two COMPLETE events for different runs of the same job writing to the same
/// output dataset **with different schemas** should produce distinct dataset
/// versions (because the schema fields contribute to the version UUID).
#[tokio::test]
async fn dataset_version_uuid_includes_schema() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let ds_name = format!("schema_ds_{}", rand::random_range(0..u32::MAX));

    // Run 1: output dataset with schema {id: INT}
    let run1 = Uuid::new_v4();
    let event1 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run1.to_string(),
        vec![],
        vec![json!({
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        { "name": "id", "type": "INT" }
                    ]
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event1).await;

    let ds1 = get_dataset(&app, &ns, &ds_name).await;
    let version1 = ds1["currentVersion"].as_str().unwrap().to_string();

    // Run 2: same dataset but schema {id: INT, name: VARCHAR}
    let run2 = Uuid::new_v4();
    let event2 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run2.to_string(),
        vec![],
        vec![json!({
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        { "name": "id", "type": "INT" },
                        { "name": "name", "type": "VARCHAR" }
                    ]
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event2).await;

    let ds2 = get_dataset(&app, &ns, &ds_name).await;
    let version2 = ds2["currentVersion"].as_str().unwrap().to_string();

    assert_ne!(
        version1, version2,
        "Different schemas should produce different dataset version UUIDs"
    );
}

// ---------------------------------------------------------------------------
// Bug 9: Manual Run Completion Creates Job Version
// ---------------------------------------------------------------------------

/// Creating a run via REST API and completing it should create a job version.
#[tokio::test]
async fn manual_run_completion_creates_job_version() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();

    // First, create the job via a lineage event so it exists with IO datasets
    let setup_run = Uuid::new_v4();
    let setup_event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &setup_run.to_string(),
        vec![json!({ "namespace": ns, "name": "manual_input" })],
        vec![json!({ "namespace": ns, "name": "manual_output" })],
        None,
        None,
    );
    post_lineage(&app, &setup_event).await;

    // Now create a run via REST API
    let create_resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/runs",
            app.base_url,
            url::form_urlencoded::byte_serialize(ns.as_bytes()).collect::<String>(),
            &job_name
        ))
        .json(&json!({}))
        .send()
        .await
        .expect("create run");
    assert_eq!(create_resp.status().as_u16(), 201);
    let run_body: Value = create_resp.json().await.unwrap();
    let run_id: Uuid = run_body["id"]
        .as_str()
        .unwrap()
        .parse()
        .expect("valid run uuid");

    // Complete the run via REST API
    let complete_resp = app
        .client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/complete",
            app.base_url, run_id
        ))
        .send()
        .await
        .expect("complete run");
    assert_eq!(complete_resp.status().as_u16(), 200);

    // The run should now have a jobVersion
    let run = get_run(&app, run_id).await;
    assert!(
        !run["jobVersion"].is_null(),
        "Completed run should have a jobVersion link, got: {:?}",
        run["jobVersion"]
    );
}

// ---------------------------------------------------------------------------
// Bug 11: Job Version UUID Determinism
// ---------------------------------------------------------------------------

/// Two COMPLETE events for the same job with the same IO datasets should
/// produce the same job version UUID (not two separate versions).
#[tokio::test]
async fn job_version_uuid_deterministic_for_same_io() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let input = format!("jv_input_{}", rand::random_range(0..u32::MAX));
    let output = format!("jv_output_{}", rand::random_range(0..u32::MAX));

    // Run 1
    let run1 = Uuid::new_v4();
    let event1 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run1.to_string(),
        vec![json!({ "namespace": ns, "name": input })],
        vec![json!({ "namespace": ns, "name": output })],
        None,
        None,
    );
    post_lineage(&app, &event1).await;

    let vc1 = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(vc1, 1, "Should have 1 job version after first run");

    // Run 2 — same IO datasets
    let run2 = Uuid::new_v4();
    let event2 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run2.to_string(),
        vec![json!({ "namespace": ns, "name": input })],
        vec![json!({ "namespace": ns, "name": output })],
        None,
        None,
    );
    post_lineage(&app, &event2).await;

    let vc2 = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(
        vc2, 1,
        "Same IO datasets should produce same job version UUID — expected 1 version, got {}",
        vc2
    );
}

/// Changing IO datasets should produce a new job version.
#[tokio::test]
async fn job_version_uuid_changes_with_io() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let input1 = format!("jv_in1_{}", rand::random_range(0..u32::MAX));
    let output1 = format!("jv_out1_{}", rand::random_range(0..u32::MAX));
    let output2 = format!("jv_out2_{}", rand::random_range(0..u32::MAX));

    // Run 1 with output1
    let run1 = Uuid::new_v4();
    let event1 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run1.to_string(),
        vec![json!({ "namespace": ns, "name": input1 })],
        vec![json!({ "namespace": ns, "name": output1 })],
        None,
        None,
    );
    post_lineage(&app, &event1).await;

    let vc1 = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(vc1, 1, "Should have 1 version after first run");

    // Run 2 with different output (output2)
    let run2 = Uuid::new_v4();
    let event2 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run2.to_string(),
        vec![json!({ "namespace": ns, "name": input1 })],
        vec![json!({ "namespace": ns, "name": output2 })],
        None,
        None,
    );
    post_lineage(&app, &event2).await;

    let vc2 = get_job_versions_count(&app, &ns, &job_name).await;
    assert_eq!(
        vc2, 2,
        "Different IO datasets should produce a new job version — expected 2 versions, got {}",
        vc2
    );
}

// ---------------------------------------------------------------------------
// Bug 19: Events pagination offset test
// ---------------------------------------------------------------------------

/// Verify that the events endpoint respects the `offset` parameter,
/// returning different events for offset=0 vs offset=1.
#[tokio::test]
async fn events_pagination_offset() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();

    // Post 3 events with different run IDs
    for i in 0..3 {
        let run_id = Uuid::new_v4();
        let event = make_run_event(
            "COMPLETE",
            &ns,
            &job_name,
            &run_id.to_string(),
            vec![],
            vec![json!({ "namespace": ns, "name": format!("ds_{}", i) })],
            None,
            None,
        );
        post_lineage(&app, &event).await;
    }

    // Fetch first page: limit=1, offset=0
    let url0 = format!("{}/api/v1/events/lineage?limit=1&offset=0", app.base_url);
    let resp0 = app.client.get(&url0).send().await.unwrap();
    assert_eq!(resp0.status(), 200);
    let body0: serde_json::Value = resp0.json().await.unwrap();
    let events0 = body0["events"].as_array().expect("events array");
    assert_eq!(events0.len(), 1, "expected 1 event with limit=1");

    // Fetch second page: limit=1, offset=1
    let url1 = format!("{}/api/v1/events/lineage?limit=1&offset=1", app.base_url);
    let resp1 = app.client.get(&url1).send().await.unwrap();
    assert_eq!(resp1.status(), 200);
    let body1: serde_json::Value = resp1.json().await.unwrap();
    let events1 = body1["events"].as_array().expect("events array");
    assert_eq!(events1.len(), 1, "expected 1 event with limit=1 offset=1");

    // The two pages should return different events
    assert_ne!(
        events0[0], events1[0],
        "offset=0 and offset=1 should return different events"
    );
}

// ---------------------------------------------------------------------------
// Bug 23: Parent facet priority test
// ---------------------------------------------------------------------------

/// When both `parent` and `parentRun` facets are present in the same event,
/// the `parent` facet should take priority (matching Java behavior).
#[tokio::test]
async fn parent_facet_takes_priority_over_parent_run() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let dag_via_parent = format!("dag_parent_{}", rand::random_range(0..u32::MAX));
    let dag_via_parent_run = format!("dag_parentrun_{}", rand::random_range(0..u32::MAX));
    let task_name = format!("{}.task_priority", dag_via_parent);
    let run_id = Uuid::new_v4().to_string();
    let parent_run_id_1 = Uuid::new_v4().to_string();
    let parent_run_id_2 = Uuid::new_v4().to_string();

    // Send event with BOTH `parent` and `parentRun` pointing to DIFFERENT parent jobs
    let run_facets = json!({
        "parent": {
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id_1 },
            "job": { "namespace": ns, "name": task_name }
        },
        "parentRun": {
            "_producer": "test",
            "_schemaURL": "test",
            "run": { "runId": parent_run_id_2 },
            "job": { "namespace": ns, "name": format!("{}.task_priority", dag_via_parent_run) }
        }
    });

    let event = make_run_event(
        "START",
        &ns,
        &task_name,
        &run_id,
        vec![],
        vec![],
        Some(run_facets),
        None,
    );
    post_lineage(&app, &event).await;

    // The `parent` facet should win — so dag_via_parent should be the parent job
    let parent_job = get_job(&app, &ns, &dag_via_parent).await;
    assert_eq!(
        parent_job["name"], dag_via_parent,
        "parent facet should take priority over parentRun"
    );
}

// ---------------------------------------------------------------------------
// Bug 41: Column lineage skips malformed entries
// ---------------------------------------------------------------------------

/// Verify that a lineage event with a malformed columnLineage entry (missing
/// inputFields) does not crash and valid entries are still processed.
#[tokio::test]
async fn column_lineage_skips_malformed_entry() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let run_id = generators::new_run_id().to_string();
    let job_name = generators::new_job_name();
    let input_ds = generators::new_dataset_name();
    let output_ds = generators::new_dataset_name();

    // START event with input dataset
    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id,
        vec![json!({
            "namespace": ns,
            "name": input_ds,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        {"name": "input_col", "type": "VARCHAR"}
                    ]
                }
            }
        })],
        vec![],
        None,
        None,
    );
    post_lineage(&app, &start_event).await;

    // COMPLETE event with output dataset containing columnLineage with:
    // 1. A valid entry ("good_col" with proper inputFields)
    // 2. A malformed entry ("bad_col" with just a string instead of object)
    let complete_event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id,
        vec![json!({
            "namespace": ns,
            "name": input_ds,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        {"name": "input_col", "type": "VARCHAR"}
                    ]
                }
            }
        })],
        vec![json!({
            "namespace": ns,
            "name": output_ds,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        {"name": "good_col", "type": "VARCHAR"},
                        {"name": "bad_col", "type": "VARCHAR"}
                    ]
                },
                "columnLineage": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": {
                        "good_col": {
                            "inputFields": [
                                {"namespace": ns, "name": input_ds, "field": "input_col"}
                            ],
                            "transformationDescription": "identity",
                            "transformationType": "IDENTITY"
                        },
                        "bad_col": "not-an-object"
                    }
                }
            }
        })],
        None,
        None,
    );
    // Should succeed without error — malformed entry is skipped
    post_lineage(&app, &complete_event).await;

    // Verify the output dataset was created successfully
    let ds = get_dataset(&app, &ns, &output_ds).await;
    assert_eq!(ds["name"], output_ds);
}

// ---------------------------------------------------------------------------
// Bug 55: Unknown OL event type returns 200
// ---------------------------------------------------------------------------

/// Bug 55: Verify that posting an event with no `run`, `dataset`, or `job`
/// key returns 200 (not 400), echoing the body back.
#[tokio::test]
async fn unknown_event_returns_422() {
    let app = TestApp::new().await;
    let url = format!("{}/api/v1/lineage", app.base_url);
    let unknown_body = json!({
        "eventType": "UNKNOWN",
        "eventTime": "2024-01-01T00:00:00Z",
        "producer": "test-producer",
        "custom_field": "custom_value"
    });

    let resp = app
        .client
        .post(&url)
        .json(&unknown_body)
        .send()
        .await
        .expect("post unknown event");

    assert_eq!(
        resp.status(),
        422,
        "Unknown event type should return 422 (Java parity), got {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Lineage graph: START-with-IO then bare-COMPLETE pattern (seed data pattern)
// ---------------------------------------------------------------------------

/// Helper: GET lineage graph from a dataset node.
async fn get_lineage_graph_for_dataset(app: &TestApp, ns: &str, ds_name: &str) -> Vec<Value> {
    let node_id = format!("dataset:{}:{}", ns, ds_name);
    let url = format!(
        "{}/api/v1/lineage?nodeId={}&depth=10",
        app.base_url, node_id
    );
    let resp = app.client.get(&url).send().await.expect("get lineage");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    body["graph"].as_array().cloned().unwrap_or_default()
}

/// When START events carry inputs/outputs and COMPLETE events have empty
/// arrays (the pattern used by the seed data / many Airflow integrations),
/// the lineage graph must still show the full connected graph.
///
/// This test reproduces the exact bug where querying
/// `dataset:ns:output_ds` returned a single orphan dataset node with
/// empty edges instead of the full graph.
#[tokio::test]
async fn lineage_graph_with_bare_complete_events() {
    let app = TestApp::new().await;
    let ns = format!("lineage_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("etl_job_{}", rand::random_range(0..u32::MAX));
    let run_id = Uuid::new_v4();
    let input_ds = format!("input_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_{}", rand::random_range(0..u32::MAX));

    // START event: carries inputs and outputs
    let start_event = make_run_event(
        "START",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![json!({"namespace": ns, "name": input_ds})],
        vec![json!({"namespace": ns, "name": output_ds})],
        None,
        None,
    );
    post_lineage(&app, &start_event).await;

    // COMPLETE event: empty inputs and outputs (seed data pattern)
    let complete_event = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run_id.to_string(),
        vec![],
        vec![],
        None,
        None,
    );
    post_lineage(&app, &complete_event).await;

    // Query lineage from the output dataset
    let graph = get_lineage_graph_for_dataset(&app, &ns, &output_ds).await;

    // Should have 3 nodes: input dataset, job, output dataset
    assert!(
        graph.len() >= 3,
        "Expected at least 3 nodes (input ds, job, output ds), got {}: {:?}",
        graph.len(),
        graph.iter().map(|n| n["id"].as_str()).collect::<Vec<_>>()
    );

    // Find the job node and verify it has edges
    let job_node = graph
        .iter()
        .find(|n| n["type"] == "JOB")
        .expect("job node should be in graph");
    let job_in_edges = job_node["inEdges"].as_array().expect("inEdges array");
    let job_out_edges = job_node["outEdges"].as_array().expect("outEdges array");
    assert!(
        !job_in_edges.is_empty(),
        "Job should have inEdges (from input dataset)"
    );
    assert!(
        !job_out_edges.is_empty(),
        "Job should have outEdges (to output dataset)"
    );

    // Find the output dataset node and verify it has inEdges
    let output_node_id = format!("dataset:{}:{}", ns, output_ds);
    let output_node = graph
        .iter()
        .find(|n| n["id"] == output_node_id)
        .expect("output dataset node should be in graph");
    let ds_in_edges = output_node["inEdges"].as_array().expect("inEdges array");
    assert!(
        !ds_in_edges.is_empty(),
        "Output dataset should have inEdges from the job"
    );
}

// ---------------------------------------------------------------------------
// Lineage graph ordering
// ---------------------------------------------------------------------------

/// Verifies that the lineage graph returns nodes sorted lexicographically by
/// NodeId and edges sorted by (origin, destination), matching the Java
/// backend's `ImmutableSortedSet<Node>` behavior.
#[tokio::test]
async fn lineage_graph_ordering_is_deterministic() {
    let app = TestApp::new().await;
    let ns = format!("ordering_ns_{}", rand::random_range(0..u32::MAX));

    // Create a 3-job pipeline:
    //   job_alpha  -> ds_middle
    //   ds_middle  -> job_beta -> ds_zeta
    //   ds_middle  -> job_gamma -> ds_zeta
    //
    // Node IDs (sorted) should be:
    //   dataset:<ns>:ds_middle
    //   dataset:<ns>:ds_zeta
    //   job:<ns>:job_alpha
    //   job:<ns>:job_beta
    //   job:<ns>:job_gamma

    let run1 = Uuid::new_v4();
    let run2 = Uuid::new_v4();
    let run3 = Uuid::new_v4();

    // job_alpha COMPLETE: outputs ds_middle
    post_lineage(
        &app,
        &make_run_event(
            "COMPLETE",
            &ns,
            "job_alpha",
            &run1.to_string(),
            vec![],
            vec![json!({ "namespace": ns, "name": "ds_middle" })],
            None,
            None,
        ),
    )
    .await;

    // job_beta COMPLETE: inputs ds_middle, outputs ds_zeta
    post_lineage(
        &app,
        &make_run_event(
            "COMPLETE",
            &ns,
            "job_beta",
            &run2.to_string(),
            vec![json!({ "namespace": ns, "name": "ds_middle" })],
            vec![json!({ "namespace": ns, "name": "ds_zeta" })],
            None,
            None,
        ),
    )
    .await;

    // job_gamma COMPLETE: inputs ds_middle, outputs ds_zeta
    post_lineage(
        &app,
        &make_run_event(
            "COMPLETE",
            &ns,
            "job_gamma",
            &run3.to_string(),
            vec![json!({ "namespace": ns, "name": "ds_middle" })],
            vec![json!({ "namespace": ns, "name": "ds_zeta" })],
            None,
            None,
        ),
    )
    .await;

    // Fetch lineage graph starting from job_alpha
    let graph = get_lineage_graph(&app, &ns, "job_alpha").await;

    // 1. Verify nodes are sorted lexicographically by id
    let node_ids: Vec<String> = graph
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    let mut sorted_ids = node_ids.clone();
    sorted_ids.sort();
    assert_eq!(
        node_ids, sorted_ids,
        "Nodes must be sorted lexicographically by id"
    );

    // 2. Verify edges within each node are sorted by (origin, destination)
    for node in &graph {
        for edge_field in ["inEdges", "outEdges"] {
            if let Some(edges) = node[edge_field].as_array() {
                let edge_keys: Vec<(String, String)> = edges
                    .iter()
                    .map(|e| {
                        (
                            e["origin"].as_str().unwrap().to_string(),
                            e["destination"].as_str().unwrap().to_string(),
                        )
                    })
                    .collect();
                let mut sorted_keys = edge_keys.clone();
                sorted_keys.sort();
                assert_eq!(
                    edge_keys, sorted_keys,
                    "Edges in {} of node {} must be sorted",
                    edge_field, node["id"]
                );
            }
        }
    }

    // 3. Verify idempotency — second call returns identical ordering
    let graph2 = get_lineage_graph(&app, &ns, "job_alpha").await;
    assert_eq!(
        graph, graph2,
        "Lineage graph ordering must be deterministic across calls"
    );
}

// -------------------------------------------------------------------------
// dataset_fields_reflect_current_version
// -------------------------------------------------------------------------
/// When a dataset schema evolves (columns dropped), GET dataset and list
/// endpoints should only show the current version's fields, not stale ones.
#[tokio::test]
async fn dataset_fields_reflect_current_version() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();
    let ds_name = format!("ds_schema_evo_{}", rand::random_range(0..u32::MAX));

    // Run 1: dataset has fields id, name, email
    let run1 = Uuid::new_v4();
    let event1 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run1.to_string(),
        vec![],
        vec![json!({
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        {"name": "id", "type": "INT"},
                        {"name": "name", "type": "VARCHAR"},
                        {"name": "email", "type": "VARCHAR"}
                    ]
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event1).await;

    // GET single dataset — should have 3 fields
    let ds = get_dataset(&app, &ns, &ds_name).await;
    let fields = ds["fields"].as_array().expect("fields array");
    assert_eq!(
        fields.len(),
        3,
        "expected 3 fields after run 1, got {}: {:?}",
        fields.len(),
        fields
            .iter()
            .map(|f| f["name"].as_str())
            .collect::<Vec<_>>()
    );

    // GET list — should have 3 fields for this dataset
    let list_url = format!(
        "{}/api/v1/namespaces/{}/datasets",
        app.base_url,
        encode(&ns)
    );
    let list_body: Value = app
        .client
        .get(&list_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let list_ds = list_body["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"].as_str() == Some(&ds_name))
        .expect("dataset in list");
    assert_eq!(
        list_ds["fields"].as_array().unwrap().len(),
        3,
        "list should show 3 fields after run 1"
    );

    // Run 2: same dataset, now only has field "id"
    let run2 = Uuid::new_v4();
    let event2 = make_run_event(
        "COMPLETE",
        &ns,
        &job_name,
        &run2.to_string(),
        vec![],
        vec![json!({
            "namespace": ns,
            "name": ds_name,
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [
                        {"name": "id", "type": "INT"}
                    ]
                }
            }
        })],
        None,
        None,
    );
    post_lineage(&app, &event2).await;

    // GET single dataset — should now show only 1 field
    let ds = get_dataset(&app, &ns, &ds_name).await;
    let fields = ds["fields"].as_array().expect("fields array");
    assert_eq!(
        fields.len(),
        1,
        "expected 1 field after run 2, got {}: {:?}",
        fields.len(),
        fields
            .iter()
            .map(|f| f["name"].as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(fields[0]["name"], "id");

    // GET list — should also show only 1 field
    let list_body: Value = app
        .client
        .get(&list_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let list_ds = list_body["datasets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"].as_str() == Some(&ds_name))
        .expect("dataset in list");
    let list_fields = list_ds["fields"].as_array().unwrap();
    assert_eq!(
        list_fields.len(),
        1,
        "list should show 1 field after run 2, got {}: {:?}",
        list_fields.len(),
        list_fields
            .iter()
            .map(|f| f["name"].as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(list_fields[0]["name"], "id");
}
