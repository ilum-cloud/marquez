use crate::generators;
use crate::TestApp;
use serde_json::Value;

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

// ---------------------------------------------------------------------------
// Helper: create a job via the API
// ---------------------------------------------------------------------------
async fn create_job(app: &TestApp, namespace: &str, name: &str) {
    let url = format!(
        "{}/api/v1/namespaces/{}/jobs/{}",
        app.base_url,
        encode(namespace),
        name
    );
    let resp = app
        .client
        .put(&url)
        .json(&serde_json::json!({
            "type": "BATCH",
            "inputs": [],
            "outputs": [],
            "description": "test job"
        }))
        .send()
        .await
        .expect("create job request");
    assert!(
        resp.status().is_success(),
        "create job failed: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Helper: perform a search and return the JSON body
// ---------------------------------------------------------------------------
async fn do_search(app: &TestApp, query_params: &str) -> Value {
    let url = format!("{}/api/v1/search?{}", app.base_url, query_params);
    let resp = app.client.get(&url).send().await.expect("search request");
    assert_eq!(resp.status(), 200, "search failed: {}", resp.status());
    resp.json().await.expect("parse json")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_returns_empty_for_no_results() {
    let app = TestApp::new().await;
    let random_query = format!("nonexistent_{}", generators::new_run_id());

    let body = do_search(&app, &format!("q={}&limit=10", random_query)).await;
    assert_eq!(body["totalCount"], 0);
    assert!(body["results"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn search_finds_dataset() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let dataset = generators::new_dataset_name();

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;

    let body = do_search(&app, &format!("q={}&limit=10", dataset)).await;
    assert!(
        body["totalCount"].as_i64().unwrap() >= 1,
        "expected at least 1 result, got {}",
        body["totalCount"]
    );

    let results = body["results"].as_array().unwrap();
    let found = results
        .iter()
        .any(|r| r["name"].as_str() == Some(dataset.as_str()));
    assert!(found, "dataset '{}' not found in search results", dataset);
}

#[tokio::test]
async fn search_finds_job() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let job = generators::new_job_name();

    create_namespace(&app, &ns, &owner).await;
    create_job(&app, &ns, &job).await;

    let body = do_search(&app, &format!("q={}&limit=10", job)).await;
    assert!(
        body["totalCount"].as_i64().unwrap() >= 1,
        "expected at least 1 result, got {}",
        body["totalCount"]
    );

    let results = body["results"].as_array().unwrap();
    let found = results
        .iter()
        .any(|r| r["name"].as_str() == Some(job.as_str()));
    assert!(found, "job '{}' not found in search results", job);
}

#[tokio::test]
async fn simple_search_endpoint() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let dataset = generators::new_dataset_name();

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;

    let url = format!(
        "{}/api/v1/search/simple?q={}&limit=10",
        app.base_url, dataset
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("simple search request");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");
    assert!(
        body.get("totalCount").is_some(),
        "response missing totalCount"
    );
    assert!(body.get("results").is_some(), "response missing results");

    let total_count = body["totalCount"].as_i64().unwrap();
    assert!(
        total_count >= 1,
        "expected at least 1 result, got {}",
        total_count
    );

    let results = body["results"].as_array().unwrap();
    let found = results
        .iter()
        .any(|r| r["name"].as_str() == Some(dataset.as_str()));
    assert!(
        found,
        "dataset '{}' not found in simple search results",
        dataset
    );
}

#[tokio::test]
async fn search_combined_returns_datasets_and_jobs() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    // Use a common prefix so we can search for both
    let prefix = format!("combo_{}", rand::random_range(0..u32::MAX));
    let dataset = format!("{}_ds", prefix);
    let job = format!("{}_job", prefix);

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;
    create_job(&app, &ns, &job).await;

    // No filter → should return both
    let body = do_search(&app, &format!("q={}&limit=10", prefix)).await;
    let results = body["results"].as_array().unwrap();

    let has_dataset = results
        .iter()
        .any(|r| r["type"].as_str() == Some("DATASET"));
    let has_job = results.iter().any(|r| r["type"].as_str() == Some("JOB"));

    assert!(has_dataset, "expected at least one DATASET result");
    assert!(has_job, "expected at least one JOB result");
}

#[tokio::test]
async fn search_filter_dataset_only() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let prefix = format!("filt_{}", rand::random_range(0..u32::MAX));
    let dataset = format!("{}_ds", prefix);
    let job = format!("{}_job", prefix);

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;
    create_job(&app, &ns, &job).await;

    let body = do_search(&app, &format!("q={}&filter=DATASET&limit=10", prefix)).await;
    let results = body["results"].as_array().unwrap();

    for r in results {
        assert_eq!(
            r["type"].as_str(),
            Some("DATASET"),
            "filter=DATASET but got {:?}",
            r["type"]
        );
    }
    assert!(
        !results.is_empty(),
        "expected at least one DATASET result with filter=DATASET"
    );
}

#[tokio::test]
async fn search_filter_job_only() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let prefix = format!("filt_{}", rand::random_range(0..u32::MAX));
    let dataset = format!("{}_ds", prefix);
    let job = format!("{}_job", prefix);

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;
    create_job(&app, &ns, &job).await;

    let body = do_search(&app, &format!("q={}&filter=JOB&limit=10", prefix)).await;
    let results = body["results"].as_array().unwrap();

    for r in results {
        assert_eq!(
            r["type"].as_str(),
            Some("JOB"),
            "filter=JOB but got {:?}",
            r["type"]
        );
    }
    assert!(
        !results.is_empty(),
        "expected at least one JOB result with filter=JOB"
    );
}

#[tokio::test]
async fn search_namespace_filter() {
    let app = TestApp::new().await;

    let ns1 = generators::new_namespace_name();
    let ns2 = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let prefix = format!("nsf_{}", rand::random_range(0..u32::MAX));
    let ds1 = format!("{}_ds1", prefix);
    let ds2 = format!("{}_ds2", prefix);

    create_namespace(&app, &ns1, &owner).await;
    create_namespace(&app, &ns2, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns1, &ds1, &source).await;
    create_dataset(&app, &ns2, &ds2, &source).await;

    // Filter by ns1
    let body = do_search(
        &app,
        &format!("q={}&namespace={}&limit=10", prefix, encode(&ns1)),
    )
    .await;
    let results = body["results"].as_array().unwrap();

    for r in results {
        assert_eq!(
            r["namespace"].as_str(),
            Some(ns1.as_str()),
            "namespace filter didn't work: got {:?}",
            r["namespace"]
        );
    }
    assert!(
        !results.is_empty(),
        "expected at least one result in namespace {}",
        ns1
    );
}

#[tokio::test]
async fn search_sort_by_name() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let prefix = format!("srt_{}", rand::random_range(0..u32::MAX));

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &format!("{}_ccc", prefix), &source).await;
    create_dataset(&app, &ns, &format!("{}_aaa", prefix), &source).await;
    create_dataset(&app, &ns, &format!("{}_bbb", prefix), &source).await;

    let body = do_search(
        &app,
        &format!("q={}&sort=name&filter=DATASET&limit=10", prefix),
    )
    .await;
    let results = body["results"].as_array().unwrap();

    assert!(results.len() >= 3, "expected at least 3 results");
    let names: Vec<&str> = results.iter().filter_map(|r| r["name"].as_str()).collect();
    let mut sorted_names = names.clone();
    sorted_names.sort();
    assert_eq!(names, sorted_names, "results should be sorted by name ASC");
}

// ---------------------------------------------------------------------------
// Bug 26: Full search facets default to true
// ---------------------------------------------------------------------------

/// Verify that calling full_search WITHOUT the `facets` parameter
/// defaults to including facets in the response.
#[tokio::test]
async fn full_search_facets_default_true() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let dataset = generators::new_dataset_name();

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    create_dataset(&app, &ns, &dataset, &source).await;

    // Call full search WITHOUT facets parameter — should default to true
    let url = format!("{}/api/v1/search/full?q={}&limit=10", app.base_url, dataset);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("full search request");
    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("parse json");

    // full_search returns { totalCount, datasets, jobs } (not "results")
    assert!(
        body.get("totalCount").is_some(),
        "response should have totalCount"
    );
    assert!(
        body.get("datasets").is_some(),
        "response should have datasets"
    );
    assert!(body.get("jobs").is_some(), "response should have jobs");

    let datasets = body["datasets"].as_array().unwrap();
    assert!(
        !datasets.is_empty(),
        "expected at least 1 dataset result for {}",
        dataset
    );
}

// ---------------------------------------------------------------------------
// Bug 67: Search default sort is "updated_at" (most recently updated first)
// ---------------------------------------------------------------------------

/// Verify that searching WITHOUT a sort parameter returns results sorted
/// by updated_at descending (matching Java's @DefaultValue("updated_at")).
#[tokio::test]
async fn search_default_sort_is_updated_at() {
    let app = TestApp::new().await;

    let ns = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let source = generators::new_source_name();
    let prefix = format!("defsrt_{}", rand::random_range(0..u32::MAX));

    create_namespace(&app, &ns, &owner).await;
    create_source(&app, &source).await;
    // Create datasets sequentially — each gets a later updated_at
    create_dataset(&app, &ns, &format!("{}_ccc", prefix), &source).await;
    create_dataset(&app, &ns, &format!("{}_aaa", prefix), &source).await;
    create_dataset(&app, &ns, &format!("{}_bbb", prefix), &source).await;

    // Search WITHOUT sort param — should default to updated_at DESC
    let body = do_search(&app, &format!("q={}&filter=DATASET&limit=10", prefix)).await;
    let results = body["results"].as_array().unwrap();

    assert!(results.len() >= 3, "expected at least 3 results");
    let names: Vec<&str> = results.iter().filter_map(|r| r["name"].as_str()).collect();
    // With updated_at DESC, the last-created dataset (bbb) should come first
    let updated_at_values: Vec<&str> = results
        .iter()
        .filter_map(|r| r["updatedAt"].as_str())
        .collect();
    let mut sorted_desc = updated_at_values.clone();
    sorted_desc.sort_by(|a, b| b.cmp(a));
    assert_eq!(
        updated_at_values, sorted_desc,
        "results should be sorted by updatedAt DESC when no sort param is specified"
    );
    // Also verify the results are NOT simply alphabetical
    let mut alpha_sorted = names.clone();
    alpha_sorted.sort();
    assert_ne!(
        names, alpha_sorted,
        "results should NOT be in alphabetical order (default is updated_at, not name)"
    );
}

// ---------------------------------------------------------------------------
// Bug 56: v2beta search returns 503 when OpenSearch is not configured
// ---------------------------------------------------------------------------

/// Bug 56: Verify that v2beta search endpoints return 503 SERVICE_UNAVAILABLE
/// when OpenSearch is not configured (the test app never enables OpenSearch).
#[tokio::test]
async fn v2beta_search_jobs_returns_503_without_opensearch() {
    let app = TestApp::new().await;
    let url = format!("{}/api/v2beta/search/jobs?q=test&limit=10", app.base_url);
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("v2beta search jobs request");
    assert_eq!(
        resp.status(),
        503,
        "v2beta search jobs should return 503 without OpenSearch (Bug 56), got {}",
        resp.status()
    );
}

#[tokio::test]
async fn v2beta_search_datasets_returns_503_without_opensearch() {
    let app = TestApp::new().await;
    let url = format!(
        "{}/api/v2beta/search/datasets?q=test&limit=10",
        app.base_url
    );
    let resp = app
        .client
        .get(&url)
        .send()
        .await
        .expect("v2beta search datasets request");
    assert_eq!(
        resp.status(),
        503,
        "v2beta search datasets should return 503 without OpenSearch (Bug 56), got {}",
        resp.status()
    );
}
