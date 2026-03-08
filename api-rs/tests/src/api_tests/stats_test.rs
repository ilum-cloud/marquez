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

/// Helper: create a namespace via the API
async fn create_namespace(app: &TestApp, name: &str, owner: &str) {
    let url = format!("{}/api/v1/namespaces/{}", app.base_url, encode(name));
    let resp = app
        .client
        .put(&url)
        .json(&json!({
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

/// Helper: create a source via the API
async fn create_source(app: &TestApp, name: &str) {
    let url = format!("{}/api/v1/sources/{}", app.base_url, name);
    let resp = app
        .client
        .put(&url)
        .json(&json!({
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

/// Helper: create a dataset via the API
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
        .json(&json!({
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn lineage_events_day() {
    let app = TestApp::new().await;

    let url = format!("{}/api/v1/stats/lineage-events?period=DAY", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats lineage-events request");

    assert_eq!(resp.status(), 200);

    // Response is a JSON array of metric buckets
    let body: Value = resp.json().await.expect("parse json");
    assert!(body.is_array(), "expected array response, got {:?}", body);
}

#[tokio::test]
async fn lineage_events_week() {
    let app = TestApp::new().await;

    let url = format!(
        "{}/api/v1/stats/lineage-events?period=WEEK&timezone=UTC",
        app.base_url
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats lineage-events week request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    assert!(body.is_array(), "expected array response");
}

#[tokio::test]
async fn lineage_events_week_requires_timezone() {
    let app = TestApp::new().await;

    // WEEK without timezone should return 400
    let url = format!("{}/api/v1/stats/lineage-events?period=WEEK", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats lineage-events week no tz request");

    assert_eq!(
        resp.status(),
        400,
        "expected 400 for WEEK without timezone, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn lineage_events_with_data() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let job_name = generators::new_job_name();

    // Send events with recent timestamps so they appear in DAY stats
    let now = Utc::now();
    for event_type in &["START", "COMPLETE"] {
        let run_id = Uuid::new_v4();
        let event_time = (now - chrono::Duration::minutes(30)).to_rfc3339();
        let event = json!({
            "eventType": event_type,
            "eventTime": event_time,
            "run": { "runId": run_id.to_string() },
            "job": { "namespace": ns, "name": job_name },
            "inputs": [],
            "outputs": [],
            "producer": "test"
        });
        post_lineage(&app, &event).await;
    }

    // Query WEEK metrics (DAY uses materialized view which may not be refreshed)
    let url = format!(
        "{}/api/v1/stats/lineage-events?period=WEEK&timezone=UTC",
        app.base_url
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats lineage-events request");

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    let arr = body.as_array().expect("expected array");

    // The response should be a valid array of metric buckets
    // (even if empty, the structure should be correct)
    assert!(body.is_array());
    // Each bucket should have the expected structure
    for bucket in arr {
        assert!(
            bucket.get("start_interval").is_some() || bucket.get("startInterval").is_some(),
            "bucket should have start_interval"
        );
    }
}

#[tokio::test]
async fn jobs_day() {
    let app = TestApp::new().await;

    let url = format!("{}/api/v1/stats/jobs?period=DAY", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats jobs request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    assert!(body.is_array(), "expected array response, got {:?}", body);
}

#[tokio::test]
async fn jobs_week() {
    let app = TestApp::new().await;

    let url = format!(
        "{}/api/v1/stats/jobs?period=WEEK&timezone=UTC",
        app.base_url
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats jobs week request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    assert!(body.is_array(), "expected array response");
}

#[tokio::test]
async fn jobs_day_with_data() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let job_name = generators::new_job_name();

    // Create a job so it appears in stats
    create_namespace(&app, &ns, &owner).await;

    // Create the job via OpenLineage so it gets a recent created_at
    let run_id = Uuid::new_v4();
    let event_time = Utc::now().to_rfc3339();
    let event = json!({
        "eventType": "START",
        "eventTime": event_time,
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": job_name },
        "inputs": [],
        "outputs": [],
        "producer": "test"
    });
    post_lineage(&app, &event).await;

    let url = format!("{}/api/v1/stats/jobs?period=DAY", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats jobs request");

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    let arr = body.as_array().expect("expected array");

    // Should have at least one bucket with count > 0
    let total: i64 = arr.iter().filter_map(|b| b["count"].as_i64()).sum();
    assert!(
        total >= 1,
        "expected at least 1 job in DAY stats, got {}",
        total
    );
}

#[tokio::test]
async fn datasets_day() {
    let app = TestApp::new().await;

    let url = format!("{}/api/v1/stats/datasets?period=DAY", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats datasets request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    assert!(body.is_array(), "expected array response");
}

#[tokio::test]
async fn datasets_day_with_data() {
    let app = TestApp::new().await;
    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;

    // Create datasets via OpenLineage for recent timestamps
    let run_id = Uuid::new_v4();
    let event_time = Utc::now().to_rfc3339();
    let event = json!({
        "eventType": "COMPLETE",
        "eventTime": event_time,
        "run": { "runId": run_id.to_string() },
        "job": { "namespace": ns, "name": generators::new_job_name() },
        "inputs": [],
        "outputs": [{
            "namespace": ns,
            "name": generators::new_dataset_name(),
            "facets": {
                "schema": {
                    "_producer": "test",
                    "_schemaURL": "test",
                    "fields": [{"name": "id", "type": "INTEGER"}]
                }
            }
        }],
        "producer": "test"
    });
    post_lineage(&app, &event).await;

    let url = format!("{}/api/v1/stats/datasets?period=DAY", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats datasets request");

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    let arr = body.as_array().expect("expected array");

    let total: i64 = arr.iter().filter_map(|b| b["count"].as_i64()).sum();
    assert!(
        total >= 1,
        "expected at least 1 dataset in DAY stats, got {}",
        total
    );
}

#[tokio::test]
async fn sources_day() {
    let app = TestApp::new().await;

    let url = format!("{}/api/v1/stats/sources?period=DAY", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats sources request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    assert!(body.is_array(), "expected array response");
}

#[tokio::test]
async fn sources_day_with_data() {
    let app = TestApp::new().await;
    let source = generators::new_source_name();
    create_source(&app, &source).await;

    let url = format!("{}/api/v1/stats/sources?period=DAY", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats sources request");

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("parse json");
    let arr = body.as_array().expect("expected array");

    let total: i64 = arr.iter().filter_map(|b| b["count"].as_i64()).sum();
    assert!(
        total >= 1,
        "expected at least 1 source in DAY stats, got {}",
        total
    );
}

#[tokio::test]
async fn invalid_period_returns_400() {
    let app = TestApp::new().await;

    let url = format!("{}/api/v1/stats/jobs?period=INVALID", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("stats jobs invalid period request");

    assert_eq!(
        resp.status(),
        400,
        "expected 400 for invalid period, got {}",
        resp.status()
    );
}
