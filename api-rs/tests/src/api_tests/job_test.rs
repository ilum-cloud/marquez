use crate::generators;
use crate::TestApp;
use serde_json::json;

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

/// Helper: create a job within a namespace and return its name.
async fn create_job(app: &TestApp, ns: &str, desc: Option<&str>) -> String {
    let job = generators::new_job_name();
    let mut body = json!({ "type": "BATCH" });
    if let Some(d) = desc {
        body["description"] = json!(d);
    }
    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(ns),
            job
        ))
        .json(&body)
        .send()
        .await
        .expect("create job request failed");
    assert!(
        resp.status().is_success(),
        "create job returned {}",
        resp.status()
    );
    job
}

/// Helper: create a tag via PUT.
async fn create_tag(app: &TestApp, name: &str) {
    let resp = app
        .client
        .put(format!("{}/api/v1/tags/{}", app.base_url, name))
        .json(&json!({ "description": "test" }))
        .send()
        .await
        .expect("create tag request failed");
    assert!(
        resp.status().is_success(),
        "create tag returned {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_and_get_job() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = generators::new_job_name();

    // PUT job
    let put_resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .json(&json!({ "type": "BATCH" }))
        .send()
        .await
        .unwrap();
    assert!(put_resp.status().is_success());
    let put_body: serde_json::Value = put_resp.json().await.unwrap();
    assert_eq!(put_body["name"], job_name);
    assert_eq!(put_body["type"], "BATCH");
    assert_eq!(put_body["namespace"], ns);

    // GET job
    let get_resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .send()
        .await
        .unwrap();
    assert!(get_resp.status().is_success());
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(get_body["name"], job_name);
    assert_eq!(get_body["type"], "BATCH");
    assert_eq!(get_body["namespace"], ns);
    assert!(get_body.get("createdAt").is_some());
    assert!(get_body.get("updatedAt").is_some());
}

#[tokio::test]
async fn create_job_with_description() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = generators::new_job_name();
    let desc = generators::new_description();

    let resp = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .json(&json!({ "type": "BATCH", "description": desc }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], job_name);
    assert_eq!(body["description"], desc);
}

#[tokio::test]
async fn list_jobs() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    create_job(&app, &ns, None).await;
    create_job(&app, &ns, None).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs",
            app.base_url,
            encode(&ns)
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    let total_count = body["totalCount"].as_i64().unwrap();
    let jobs = body["jobs"].as_array().unwrap();
    assert!(
        total_count >= 2,
        "expected at least 2 jobs, got {}",
        total_count
    );
    assert!(jobs.len() >= 2);
}

#[tokio::test]
async fn list_jobs_with_pagination() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    create_job(&app, &ns, None).await;
    create_job(&app, &ns, None).await;
    create_job(&app, &ns, None).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs?limit=2",
            app.base_url,
            encode(&ns)
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    let total_count = body["totalCount"].as_i64().unwrap();
    let jobs = body["jobs"].as_array().unwrap();
    assert!(total_count >= 3, "totalCount should reflect all jobs");
    assert_eq!(jobs.len(), 2, "results should be limited to 2");
}

#[tokio::test]
async fn delete_job() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = create_job(&app, &ns, None).await;

    // DELETE
    let del_resp = app
        .client
        .delete(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .send()
        .await
        .unwrap();
    assert!(del_resp.status().is_success());
    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["name"], job_name);

    // GET after delete should return 404
    let get_resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        get_resp.status().as_u16(),
        404,
        "expected 404 after delete, got {}",
        get_resp.status()
    );
}

#[tokio::test]
async fn get_job_not_found() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs/nonexistent_job",
            app.base_url,
            encode(&ns)
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        404,
        "expected 404, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn create_job_idempotent() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = generators::new_job_name();
    let body = json!({ "type": "BATCH", "description": "first" });

    // First PUT
    let resp1 = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(resp1.status().is_success());

    // Second PUT (same data)
    let resp2 = app
        .client
        .put(format!(
            "{}/api/v1/namespaces/{}/jobs/{}",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(
        resp2.status().is_success(),
        "second PUT should succeed (idempotent), got {}",
        resp2.status()
    );

    let body2: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body2["name"], job_name);
    assert_eq!(body2["description"], "first");
}

#[tokio::test]
async fn list_all_jobs() {
    let app = TestApp::new().await;

    // Create jobs in two different namespaces
    let ns1 = create_namespace(&app).await;
    let ns2 = create_namespace(&app).await;
    let job1 = create_job(&app, &ns1, None).await;
    let job2 = create_job(&app, &ns2, None).await;

    let resp = app
        .client
        .get(format!("{}/api/v1/jobs", app.base_url))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    let total_count = body["totalCount"].as_i64().unwrap();
    let jobs = body["jobs"].as_array().unwrap();
    assert!(total_count >= 2, "expected at least 2 total jobs");

    let names: Vec<&str> = jobs.iter().filter_map(|j| j["name"].as_str()).collect();
    assert!(
        names.contains(&job1.as_str()),
        "should contain job from ns1"
    );
    assert!(
        names.contains(&job2.as_str()),
        "should contain job from ns2"
    );
}

#[tokio::test]
async fn tag_and_untag_job() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = create_job(&app, &ns, None).await;
    let tag = generators::new_tag_name();
    create_tag(&app, &tag).await;

    // POST tag
    let tag_resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/tags/{}",
            app.base_url,
            encode(&ns),
            job_name,
            tag
        ))
        .send()
        .await
        .unwrap();
    assert!(tag_resp.status().is_success());
    let tag_body: serde_json::Value = tag_resp.json().await.unwrap();
    let tags = tag_body["tags"]
        .as_array()
        .expect("tags should be an array");
    let tag_strs: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
    assert!(
        tag_strs.contains(&tag.as_str()),
        "job should have tag '{}', got {:?}",
        tag,
        tag_strs
    );

    // DELETE tag
    let untag_resp = app
        .client
        .delete(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/tags/{}",
            app.base_url,
            encode(&ns),
            job_name,
            tag
        ))
        .send()
        .await
        .unwrap();
    assert!(untag_resp.status().is_success());
    let untag_body: serde_json::Value = untag_resp.json().await.unwrap();
    let tags_after = untag_body["tags"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(
        !tags_after.contains(&tag.as_str()),
        "tag '{}' should be removed, but still found in {:?}",
        tag,
        tags_after
    );
}

#[tokio::test]
async fn list_job_versions() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = create_job(&app, &ns, None).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/versions",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("versions").is_some(),
        "response should have versions"
    );
}

// ---------------------------------------------------------------------------
// Bug 31: Job versions response should NOT have totalCount
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_versions_no_total_count() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = create_job(&app, &ns, None).await;

    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/versions",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("versions").is_some(),
        "response should have versions key"
    );
    assert!(
        body.get("totalCount").is_none(),
        "response should NOT have totalCount (Java compatibility)"
    );
}

// ---------------------------------------------------------------------------
// Bug 27: Jobs list supports lastRunStates filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_jobs_filter_by_last_run_state() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = create_job(&app, &ns, None).await;

    // Create a run and complete it
    let run_resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/runs",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status().as_u16(), 201);
    let run_body: serde_json::Value = run_resp.json().await.unwrap();
    let run_id = run_body["id"].as_str().unwrap();

    // Start and complete the run
    app.client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/start",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    app.client
        .post(format!(
            "{}/api/v1/jobs/runs/{}/complete",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();

    // Filter by COMPLETED — should include this job
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs?lastRunStates=COMPLETED",
            app.base_url,
            encode(&ns)
        ))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    let jobs = body["jobs"].as_array().unwrap();
    let found = jobs.iter().any(|j| j["name"].as_str() == Some(&job_name));
    assert!(
        found,
        "job with COMPLETED run should appear when filtering by COMPLETED"
    );

    // Filter by RUNNING — should NOT include this job (it's COMPLETED)
    let resp2 = app
        .client
        .get(format!(
            "{}/api/v1/namespaces/{}/jobs?lastRunStates=RUNNING",
            app.base_url,
            encode(&ns)
        ))
        .send()
        .await
        .unwrap();
    assert!(resp2.status().is_success());
    let body2: serde_json::Value = resp2.json().await.unwrap();
    let jobs2 = body2["jobs"].as_array().unwrap();
    let found2 = jobs2.iter().any(|j| j["name"].as_str() == Some(&job_name));
    assert!(
        !found2,
        "job with COMPLETED run should NOT appear when filtering by RUNNING"
    );
}

// ---------------------------------------------------------------------------
// Bug 70: Facets DATASET type returns null (not [])
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_facets_dataset_returns_null() {
    let app = TestApp::new().await;
    let ns = create_namespace(&app).await;
    let job_name = create_job(&app, &ns, None).await;

    // Create a run
    let run_resp = app
        .client
        .post(format!(
            "{}/api/v1/namespaces/{}/jobs/{}/runs",
            app.base_url,
            encode(&ns),
            job_name
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status().as_u16(), 201);
    let run_body: serde_json::Value = run_resp.json().await.unwrap();
    let run_id = run_body["id"].as_str().unwrap();

    // GET facets with type=DATASET should return null
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/jobs/runs/{}/facets?type=DATASET",
            app.base_url, run_id
        ))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "facets DATASET request failed: {}",
        resp.status()
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.is_null(),
        "facets for DATASET type should return null, got: {}",
        body
    );
}
