use crate::generators;
use crate::TestApp;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Percent-encode a value for use as a single URL path segment.
fn encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Helper: create a namespace via PUT and return its name.
async fn create_namespace(app: &TestApp) -> String {
    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}",
            app.base_url,
            encode(&ns)
        ))
        .json(&json!({ "ownerName": owner }))
        .send()
        .await
        .expect("create namespace request failed");
    assert!(
        resp.status().is_success(),
        "create namespace returned {}",
        resp.status()
    );
    ns
}

/// Helper: create a namespace and a job, return (namespace, job_name).
async fn create_namespace_and_job(app: &TestApp) -> (String, String) {
    let ns = create_namespace(app).await;
    let job = generators::new_job_name();
    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job
        ))
        .json(&json!({ "type": "BATCH" }))
        .send()
        .await
        .expect("create job request failed");
    assert!(
        resp.status().is_success(),
        "create job returned {}",
        resp.status()
    );
    (ns, job)
}

/// Helper: create a run for the given namespace/job, return the run id as a string.
async fn create_run(app: &TestApp, ns: &str, job: &str) -> String {
    let resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/runs",
            app.base_url,
            encode(ns),
            job
        ))
        .json(&json!({}))
        .send()
        .await
        .expect("create run request failed");
    assert_eq!(
        resp.status().as_u16(),
        201,
        "create run should return 201, got {}",
        resp.status()
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"]
        .as_str()
        .expect("run id should be a string")
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_and_get_run() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;

    let run_id = create_run(&app, &ns, &job).await;

    // GET run by id
    let get_resp = app
        .client
        .get(format!("{}/api/v1/jobs/runs/{}", app.base_url, run_id))
        .send()
        .await
        .unwrap();
    assert!(get_resp.status().is_success());

    let body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(body["id"], run_id);
    assert!(body.get("createdAt").is_some());
    assert!(body.get("updatedAt").is_some());
    assert!(body.get("state").is_some());
}

#[tokio::test]
async fn start_run() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;
    let run_id = create_run(&app, &ns, &job).await;

    // POST start
    let resp = app
        .client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/start",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], run_id);
    assert_eq!(body["state"], "RUNNING");
    assert!(
        body.get("startedAt").is_some() && !body["startedAt"].is_null(),
        "startedAt should be set"
    );
}

#[tokio::test]
async fn complete_run() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;
    let run_id = create_run(&app, &ns, &job).await;

    // Start
    let start_resp = app
        .client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/start",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    assert!(start_resp.status().is_success());

    // Complete
    let resp = app
        .client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/complete",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], run_id);
    assert_eq!(body["state"], "COMPLETED");
    assert!(
        body.get("endedAt").is_some() && !body["endedAt"].is_null(),
        "endedAt should be set"
    );
}

#[tokio::test]
async fn fail_run() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;
    let run_id = create_run(&app, &ns, &job).await;

    // Start
    let start_resp = app
        .client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/start",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    assert!(start_resp.status().is_success());

    // Fail
    let resp = app
        .client
        .post(format!("{}/api/v1/jobs/runs/{}/fail", app.base_url, run_id))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], run_id);
    assert_eq!(body["state"], "FAILED");
    assert!(
        body.get("endedAt").is_some() && !body["endedAt"].is_null(),
        "endedAt should be set"
    );
}

#[tokio::test]
async fn abort_run() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;
    let run_id = create_run(&app, &ns, &job).await;

    // Start
    let start_resp = app
        .client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/start",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    assert!(start_resp.status().is_success());

    // Abort
    let resp = app
        .client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/abort",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], run_id);
    assert_eq!(body["state"], "ABORTED");
    assert!(
        body.get("endedAt").is_some() && !body["endedAt"].is_null(),
        "endedAt should be set"
    );
}

#[tokio::test]
async fn get_run_not_found() {
    let app = TestApp::new().await;
    let random_id = Uuid::new_v4();

    let resp = app
        .client
        .get(format!("{}/api/v1/jobs/runs/{}", app.base_url, random_id))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        404,
        "expected 404 for nonexistent run, got {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Bug 60: Run args SHA-256 checksum uses Java KV-joined format
// ---------------------------------------------------------------------------

/// Verify that the run_args checksum uses Java's KV-joined format:
/// SHA-256 of "key1=value1#key2=value2" (sorted by key, joined with `#`,
/// key=value pairs via `=`). This matches Java's `Utils.checksumFor()`
/// which uses `Hashing.sha256().hashString(KV_JOINER.join(kvMap))`.
#[tokio::test]
async fn run_args_checksum_is_kv_format() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;

    let args = json!({"key2": "value2", "key1": "value1"});

    let resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/runs",
            app.base_url,
            encode(&ns),
            job
        ))
        .json(&json!({"args": args}))
        .send()
        .await
        .expect("create run with args");
    assert_eq!(resp.status().as_u16(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    let run_id = body["id"].as_str().unwrap();

    // Fetch the run to get the run_args_uuid
    let run_resp = app
        .client
        .get(format!("{}/api/v1/jobs/runs/{}", app.base_url, run_id))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status(), 200);
    let run_body: serde_json::Value = run_resp.json().await.unwrap();

    // The args should be returned
    let returned_args = &run_body["args"];
    assert!(!returned_args.is_null(), "run should have args in response");

    // Verify the checksum in the DB uses KV-joined format (Bug 60)
    let run_uuid: Uuid = run_id.parse().unwrap();
    let row: (Option<Uuid>,) = sqlx::query_as("SELECT run_args_uuid FROM runs WHERE uuid = $1")
        .bind(run_uuid)
        .fetch_one(&app.db.pool)
        .await
        .unwrap();
    if let Some(args_uuid) = row.0 {
        let args_row: (String,) = sqlx::query_as("SELECT checksum FROM run_args WHERE uuid = $1")
            .bind(args_uuid)
            .fetch_one(&app.db.pool)
            .await
            .unwrap();
        let actual_checksum = args_row.0;

        // Compute expected: KV-joined format (sorted keys) then SHA-256
        // "key1=value1#key2=value2" (sorted alphabetically by key)
        let kv_string = "key1=value1#key2=value2";
        let mut hasher = Sha256::new();
        hasher.update(kv_string.as_bytes());
        let expected_checksum = format!("{:x}", hasher.finalize());

        assert_eq!(
            actual_checksum, expected_checksum,
            "run_args checksum should be SHA-256 of KV-joined format (Bug 60)"
        );
    }
}

// ---------------------------------------------------------------------------
// Bug 28: create_run returns Location header
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_run_returns_location_header() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;

    let resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/runs",
            app.base_url,
            encode(&ns),
            job
        ))
        .json(&json!({}))
        .send()
        .await
        .expect("create run request failed");
    assert_eq!(resp.status().as_u16(), 201);

    let location = resp
        .headers()
        .get("location")
        .expect("response should have Location header")
        .to_str()
        .unwrap()
        .to_string();

    let body: serde_json::Value = resp.json().await.unwrap();
    let run_id = body["id"].as_str().unwrap();

    assert!(
        location.contains(run_id),
        "Location header '{}' should contain run id '{}'",
        location,
        run_id
    );
    assert!(
        location.starts_with("/api/v1/jobs/runs/"),
        "Location header should start with /api/v1/jobs/runs/, got '{}'",
        location
    );
}

// ---------------------------------------------------------------------------
// Bug 29: get_facets accepts DATASET type
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_facets_dataset_type_returns_200() {
    let app = TestApp::new().await;
    let (ns, job) = create_namespace_and_job(&app).await;
    let run_id = create_run(&app, &ns, &job).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/jobs/runs/{}/facets?type=DATASET",
            app.base_url, run_id
        ))
        .send()
        .await
        .expect("get facets request failed");

    assert_eq!(
        resp.status().as_u16(),
        200,
        "DATASET facet type should return 200, got {}",
        resp.status()
    );
}
