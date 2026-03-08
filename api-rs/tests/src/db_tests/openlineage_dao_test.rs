// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use chrono::{Duration, Utc};
use marquez_api::db::openlineage;
use marquez_api::models::openlineage::{
    InputDatasetRef, JobRef, LineageEvent, OutputDatasetRef, RunRef,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Event storage tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_lineage_event() {
    let db = TestDb::new().await;
    let job_name = format!("lineage_job_{}", Uuid::new_v4());
    let ns_name = format!("lineage_ns_{}", Uuid::new_v4());
    let event = serde_json::json!({"test": true});
    let event_time = Utc::now();

    openlineage::create_lineage_event(
        &db.pool,
        "START",
        event_time,
        &job_name,
        &ns_name,
        &event,
        "test_producer",
    )
    .await
    .unwrap();

    // Verify by querying back
    let events = openlineage::find_events_by_job(&db.pool, &job_name, &ns_name)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type.as_deref(), Some("START"));
    assert_eq!(events[0].job_name.as_deref(), Some(job_name.as_str()));
    assert_eq!(events[0].job_namespace.as_deref(), Some(ns_name.as_str()));
    assert_eq!(events[0].producer.as_deref(), Some("test_producer"));
}

#[tokio::test]
async fn create_dataset_event() {
    let db = TestDb::new().await;
    let event = serde_json::json!({"dataset": "test_ds"});
    let event_time = Utc::now();

    openlineage::create_dataset_event(&db.pool, event_time, &event, "test_producer")
        .await
        .unwrap();

    // Verify by counting
    let count = openlineage::get_total_count(
        &db.pool,
        event_time + Duration::seconds(10),
        event_time - Duration::seconds(10),
    )
    .await
    .unwrap();
    assert!(count >= 1);
}

#[tokio::test]
async fn create_job_event() {
    let db = TestDb::new().await;
    let job_name = format!("job_evt_{}", Uuid::new_v4());
    let ns_name = format!("job_evt_ns_{}", Uuid::new_v4());
    let event = serde_json::json!({"job": "test_job"});
    let event_time = Utc::now();

    openlineage::create_job_event(
        &db.pool,
        event_time,
        &job_name,
        &ns_name,
        &event,
        "test_producer",
    )
    .await
    .unwrap();

    let events = openlineage::find_events_by_job(&db.pool, &job_name, &ns_name)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]._event_type.as_deref(), Some("JOB_EVENT"));
}

#[tokio::test]
async fn find_events_by_job_empty() {
    let db = TestDb::new().await;

    let events = openlineage::find_events_by_job(&db.pool, "nonexistent", "nonexistent")
        .await
        .unwrap();
    assert!(events.is_empty());
}

#[tokio::test]
async fn get_all_events_desc() {
    let db = TestDb::new().await;
    let base_time = Utc::now();

    // Insert multiple events with slightly different times
    for i in 0..3 {
        let event = serde_json::json!({"i": i});
        let et = base_time + Duration::milliseconds(i * 100);
        openlineage::create_lineage_event(
            &db.pool,
            "START",
            et,
            &format!("job_{}", i),
            "ns",
            &event,
            "prod",
        )
        .await
        .unwrap();
    }

    let events = openlineage::get_all_events_desc(
        &db.pool,
        base_time + Duration::seconds(10),
        base_time - Duration::seconds(10),
        10,
        0,
    )
    .await
    .unwrap();
    assert!(events.len() >= 3, "expected at least 3 events");
    // Events should be in descending order
    for w in events.windows(2) {
        assert!(w[0].event_time >= w[1].event_time);
    }
}

#[tokio::test]
async fn get_all_events_asc() {
    let db = TestDb::new().await;
    let base_time = Utc::now();

    for i in 0..3 {
        let event = serde_json::json!({"i": i});
        let et = base_time + Duration::milliseconds(i * 100);
        openlineage::create_lineage_event(
            &db.pool,
            "COMPLETE",
            et,
            &format!("job_asc_{}", i),
            "ns_asc",
            &event,
            "prod",
        )
        .await
        .unwrap();
    }

    let events = openlineage::get_all_events_asc(
        &db.pool,
        base_time + Duration::seconds(10),
        base_time - Duration::seconds(10),
        10,
        0,
    )
    .await
    .unwrap();
    assert!(events.len() >= 3, "expected at least 3 events");
    // Events should be in ascending order
    for w in events.windows(2) {
        assert!(w[0].event_time <= w[1].event_time);
    }
}

#[tokio::test]
async fn get_total_count() {
    let db = TestDb::new().await;
    let base_time = Utc::now();

    for i in 0..5 {
        let event = serde_json::json!({"count": i});
        let et = base_time + Duration::milliseconds(i * 100);
        openlineage::create_lineage_event(
            &db.pool,
            "START",
            et,
            &format!("count_job_{}", i),
            "count_ns",
            &event,
            "prod",
        )
        .await
        .unwrap();
    }

    let count = openlineage::get_total_count(
        &db.pool,
        base_time + Duration::seconds(10),
        base_time - Duration::seconds(10),
    )
    .await
    .unwrap();
    assert!(count >= 5, "expected at least 5 events, got {}", count);
}

// ---------------------------------------------------------------------------
// Utility function tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_run_state_mapping() {
    assert_eq!(openlineage::get_run_state("COMPLETE"), "COMPLETED");
    assert_eq!(openlineage::get_run_state("ABORT"), "ABORTED");
    assert_eq!(openlineage::get_run_state("FAIL"), "FAILED");
    assert_eq!(openlineage::get_run_state("START"), "RUNNING");
    assert_eq!(openlineage::get_run_state("RUNNING"), "RUNNING");
    assert_eq!(openlineage::get_run_state("complete"), "COMPLETED");
    // Java defaults unknown event types to RUNNING (OpenLineageDao.java:1116-1117)
    assert_eq!(openlineage::get_run_state("unknown"), "RUNNING");
}

#[tokio::test]
async fn run_to_uuid_valid() {
    let uuid_str = "43d7db3b-8984-4cda-b237-8c6919b36670";
    let result = openlineage::run_to_uuid(uuid_str);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), Uuid::parse_str(uuid_str).unwrap());
}

#[tokio::test]
async fn run_to_uuid_invalid() {
    let result = openlineage::run_to_uuid("not-a-uuid");
    assert!(result.is_some());
    // Should produce a deterministic UUID derived from the string via MD5
    let uuid = result.unwrap();
    // The same input should always produce the same UUID
    let result2 = openlineage::run_to_uuid("not-a-uuid");
    assert_eq!(uuid, result2.unwrap());
}

/// Bug 12: Verify `run_to_uuid` for non-UUID inputs produces the same UUID as
/// Java's `UUID.nameUUIDFromBytes()` (raw MD5, not v5 with namespace prefix).
///
/// Java: UUID.nameUUIDFromBytes("my-custom-run-id".getBytes(UTF_8))
///     = 08c2b0fb-a89b-352f-9a45-4048ff4ab6a1
#[tokio::test]
async fn run_to_uuid_md5_parity_with_java() {
    // Pre-computed from Java: UUID.nameUUIDFromBytes("my-custom-run-id".getBytes(UTF_8))
    let expected = Uuid::parse_str("4d66c044-f331-3045-9b75-1f227336446e").unwrap();
    let result = openlineage::run_to_uuid("my-custom-run-id");
    assert!(result.is_some());
    assert_eq!(
        result.unwrap(),
        expected,
        "run_to_uuid should match Java's UUID.nameUUIDFromBytes (raw MD5, not v5)"
    );
}

/// Also verify the underlying `uuid_name_from_bytes` directly.
#[tokio::test]
async fn uuid_name_from_bytes_java_parity() {
    // Java: UUID.nameUUIDFromBytes("test".getBytes(UTF_8))
    //     = 098f6bcd-4621-3373-8ade-4e832627b4f6
    let expected = Uuid::parse_str("098f6bcd-4621-3373-8ade-4e832627b4f6").unwrap();
    let result = openlineage::uuid_name_from_bytes(b"test");
    assert_eq!(
        result, expected,
        "uuid_name_from_bytes should match Java's UUID.nameUUIDFromBytes"
    );
}

// ---------------------------------------------------------------------------
// Orchestration tests
// ---------------------------------------------------------------------------

fn make_start_event(ns: &str, job_name: &str, run_id: Uuid) -> LineageEvent {
    LineageEvent {
        event_type: Some("START".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.to_string(),
            name: job_name.to_string(),
            facets: None,
        },
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    }
}

fn make_complete_event(ns: &str, job_name: &str, run_id: Uuid) -> LineageEvent {
    LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.to_string(),
            name: job_name.to_string(),
            facets: None,
        },
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    }
}

#[tokio::test]
async fn update_model_start() {
    let db = TestDb::new().await;
    let run_id = Uuid::new_v4();
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));

    let event = make_start_event(&ns, &job_name, run_id);

    openlineage::update_marquez_model(&db.pool, &event)
        .await
        .unwrap();

    // Verify namespace was created
    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap();
    assert!(ns_row.is_some());

    // Verify job was created
    let job_row = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap();
    assert!(job_row.is_some());

    // Verify run was created
    let run_row = marquez_api::db::run::find_by_uuid(&db.pool, run_id)
        .await
        .unwrap();
    assert!(run_row.is_some());
    let run = run_row.unwrap();
    assert_eq!(run.current_run_state.as_deref(), Some("RUNNING"));
    assert!(run.started_at.is_some());
}

#[tokio::test]
async fn update_model_complete() {
    let db = TestDb::new().await;
    let run_id = Uuid::new_v4();
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));

    // First send START
    let start_event = make_start_event(&ns, &job_name, run_id);
    openlineage::update_marquez_model(&db.pool, &start_event)
        .await
        .unwrap();

    // Then send COMPLETE
    let complete_event = make_complete_event(&ns, &job_name, run_id);
    openlineage::update_marquez_model(&db.pool, &complete_event)
        .await
        .unwrap();

    // Verify run state is COMPLETED
    let run_row = marquez_api::db::run::find_by_uuid(&db.pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run_row.current_run_state.as_deref(), Some("COMPLETED"));
    assert!(run_row.ended_at.is_some());
}

#[tokio::test]
async fn update_model_with_datasets() {
    let db = TestDb::new().await;
    let run_id = Uuid::new_v4();
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));
    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    let event = LineageEvent {
        event_type: Some("START".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: Some(vec![InputDatasetRef {
            namespace: ns.clone(),
            name: input_ds.clone(),
            facets: Some(
                [(
                    "schema".to_string(),
                    serde_json::json!({
                        "fields": [
                            {"name": "id", "type": "INTEGER"},
                            {"name": "name", "type": "VARCHAR"}
                        ]
                    }),
                )]
                .into_iter()
                .collect(),
            ),
            input_facets: None,
        }]),
        outputs: Some(vec![OutputDatasetRef {
            namespace: ns.clone(),
            name: output_ds.clone(),
            facets: None,
            output_facets: None,
        }]),
        producer: "test-producer".to_string(),
        schema_url: None,
    };

    openlineage::update_marquez_model(&db.pool, &event)
        .await
        .unwrap();

    // Verify input dataset was created
    let input_row = marquez_api::db::dataset::find_dataset_as_row(&db.pool, &ns, &input_ds)
        .await
        .unwrap();
    assert!(input_row.is_some(), "Input dataset should exist");

    // Verify output dataset was created
    let output_row = marquez_api::db::dataset::find_dataset_as_row(&db.pool, &ns, &output_ds)
        .await
        .unwrap();
    assert!(output_row.is_some(), "Output dataset should exist");

    // Verify input dataset has a current version
    let input_ds_row = input_row.unwrap();
    assert!(
        input_ds_row.current_version_uuid.is_some(),
        "Input dataset should have a current version"
    );

    // Verify run-to-input mapping
    let input_versions =
        marquez_api::db::dataset_version::find_input_versions_for(&db.pool, run_id)
            .await
            .unwrap();
    assert_eq!(
        input_versions.len(),
        1,
        "Run should have 1 input dataset version"
    );

    // Verify input dataset fields were created
    let dv_uuid = input_ds_row.current_version_uuid.unwrap();
    let fields = marquez_api::db::dataset_field::find_by_dataset_version(&db.pool, dv_uuid)
        .await
        .unwrap();
    assert_eq!(fields.len(), 2, "Input dataset should have 2 fields");
}

#[tokio::test]
async fn update_model_with_facets() {
    let db = TestDb::new().await;
    let run_id = Uuid::new_v4();
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));

    let event = LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.to_string(),
            facets: Some(
                [(
                    "environment".to_string(),
                    serde_json::json!({"name": "production"}),
                )]
                .into_iter()
                .collect(),
            ),
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: Some(
                [("sql".to_string(), serde_json::json!({"query": "SELECT 1"}))]
                    .into_iter()
                    .collect(),
            ),
        },
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    };

    openlineage::update_marquez_model(&db.pool, &event)
        .await
        .unwrap();

    // Verify run facets were stored
    let run_facets = marquez_api::db::facets::find_run_facets_by_run(&db.pool, run_id)
        .await
        .unwrap();
    assert!(run_facets.is_some(), "Run should have facets");

    // Verify job facets were stored
    let job_facets = marquez_api::db::facets::find_job_facets_by_run(&db.pool, run_id)
        .await
        .unwrap();
    assert!(job_facets.is_some(), "Job should have facets");
}

// ---------------------------------------------------------------------------
// IO mapping tests
// ---------------------------------------------------------------------------

/// Verify that IO mappings are created on the very first COMPLETE event.
///
/// This was the primary bug: IO mappings were created BEFORE the job version
/// existed, so the first event for a job would silently skip them.
#[tokio::test]
async fn io_mapping_created_on_first_event() {
    let db = TestDb::new().await;
    let run_id = Uuid::new_v4();
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));
    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    let event = LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: Some(vec![InputDatasetRef {
            namespace: ns.clone(),
            name: input_ds.clone(),
            facets: None,
            input_facets: None,
        }]),
        outputs: Some(vec![OutputDatasetRef {
            namespace: ns.clone(),
            name: output_ds.clone(),
            facets: None,
            output_facets: None,
        }]),
        producer: "test-producer".to_string(),
        schema_url: None,
    };

    openlineage::update_marquez_model(&db.pool, &event)
        .await
        .unwrap();

    // Verify the job has a current version
    let job_row = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap()
        .expect("Job should exist");
    assert!(
        job_row.current_version_uuid.is_some(),
        "Job should have a current version after first event"
    );
    let jv_uuid = job_row.current_version_uuid.unwrap();

    // Verify IO mappings exist for the current job version
    let input_mappings: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT dataset_uuid, io_type FROM job_versions_io_mapping \
         WHERE job_version_uuid = $1 AND io_type = 'INPUT' AND is_current_job_version = true",
    )
    .bind(jv_uuid)
    .fetch_all(&db.pool)
    .await
    .unwrap();

    assert_eq!(
        input_mappings.len(),
        1,
        "Should have 1 INPUT IO mapping after first event"
    );

    let output_mappings: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT dataset_uuid, io_type FROM job_versions_io_mapping \
         WHERE job_version_uuid = $1 AND io_type = 'OUTPUT' AND is_current_job_version = true",
    )
    .bind(jv_uuid)
    .fetch_all(&db.pool)
    .await
    .unwrap();

    assert_eq!(
        output_mappings.len(),
        1,
        "Should have 1 OUTPUT IO mapping after first event"
    );
}

/// Verify that a second event updates IO mappings to the new job version.
#[tokio::test]
async fn io_mapping_updated_on_second_event() {
    let db = TestDb::new().await;
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));
    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));
    let output_ds = format!("output_ds_{}", rand::random_range(0..u32::MAX));

    // First event
    let run_id_1 = Uuid::new_v4();
    let event1 = LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id_1.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: Some(vec![InputDatasetRef {
            namespace: ns.clone(),
            name: input_ds.clone(),
            facets: None,
            input_facets: None,
        }]),
        outputs: Some(vec![OutputDatasetRef {
            namespace: ns.clone(),
            name: output_ds.clone(),
            facets: None,
            output_facets: None,
        }]),
        producer: "test-producer".to_string(),
        schema_url: None,
    };

    openlineage::update_marquez_model(&db.pool, &event1)
        .await
        .unwrap();

    let job_row_1 = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap()
        .expect("Job should exist");
    let jv_uuid_1 = job_row_1.current_version_uuid.unwrap();

    // Second event with a new run and DIFFERENT IO datasets (different version hash)
    let run_id_2 = Uuid::new_v4();
    let input_ds_2 = format!("input_ds_2_{}", rand::random_range(0..u32::MAX));
    let output_ds_2 = format!("output_ds_2_{}", rand::random_range(0..u32::MAX));
    let event2 = LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now() + Duration::seconds(1),
        run: RunRef {
            run_id: run_id_2.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: Some(vec![InputDatasetRef {
            namespace: ns.clone(),
            name: input_ds_2.clone(),
            facets: None,
            input_facets: None,
        }]),
        outputs: Some(vec![OutputDatasetRef {
            namespace: ns.clone(),
            name: output_ds_2.clone(),
            facets: None,
            output_facets: None,
        }]),
        producer: "test-producer".to_string(),
        schema_url: None,
    };

    openlineage::update_marquez_model(&db.pool, &event2)
        .await
        .unwrap();

    let job_row_2 = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap()
        .expect("Job should exist");
    let jv_uuid_2 = job_row_2.current_version_uuid.unwrap();

    // The two job versions should be different (different run UUIDs)
    assert_ne!(
        jv_uuid_1, jv_uuid_2,
        "Second event should create a new job version"
    );

    // The second job version should have current IO mappings
    let current_mappings: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT dataset_uuid, io_type FROM job_versions_io_mapping \
         WHERE job_version_uuid = $1 AND is_current_job_version = true",
    )
    .bind(jv_uuid_2)
    .fetch_all(&db.pool)
    .await
    .unwrap();

    assert_eq!(
        current_mappings.len(),
        2,
        "Second job version should have 2 current IO mappings (1 INPUT + 1 OUTPUT)"
    );

    // The first job version's mappings should be marked as not current
    let old_current: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT dataset_uuid FROM job_versions_io_mapping \
         WHERE job_version_uuid = $1 AND is_current_job_version = true",
    )
    .bind(jv_uuid_1)
    .fetch_all(&db.pool)
    .await
    .unwrap();

    assert_eq!(
        old_current.len(),
        0,
        "First job version's IO mappings should no longer be current"
    );
}

// ---------------------------------------------------------------------------
// Bug 42: run_state_done includes ABORTED
// ---------------------------------------------------------------------------

fn make_abort_event(ns: &str, job_name: &str, run_id: Uuid) -> LineageEvent {
    LineageEvent {
        event_type: Some("ABORT".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.to_string(),
            name: job_name.to_string(),
            facets: None,
        },
        inputs: None,
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    }
}

/// Bug 42: Verify that ABORT events create a job version (run_state_done
/// includes ABORTED). Before fix, ABORTED was missing from the match.
#[tokio::test]
async fn abort_event_creates_job_version() {
    let db = TestDb::new().await;
    let run_id = Uuid::new_v4();
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));

    // Send START then ABORT
    let start_event = make_start_event(&ns, &job_name, run_id);
    openlineage::update_marquez_model(&db.pool, &start_event)
        .await
        .unwrap();

    let abort_event = make_abort_event(&ns, &job_name, run_id);
    openlineage::update_marquez_model(&db.pool, &abort_event)
        .await
        .unwrap();

    // Verify run state is ABORTED
    let run_row = marquez_api::db::run::find_by_uuid(&db.pool, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run_row.current_run_state.as_deref(), Some("ABORTED"));
    assert!(run_row.ended_at.is_some());

    // Verify job version was created (gated by run_state_done which now includes ABORTED)
    let job_row = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap()
        .expect("Job should exist");
    assert!(
        job_row.current_version_uuid.is_some(),
        "Job should have a current version after ABORT event (Bug 42)"
    );
}

// ---------------------------------------------------------------------------
// Bug 45: IO mapping ON CONFLICT re-activates stale mappings
// ---------------------------------------------------------------------------

/// Bug 45: Verify that stale IO mappings (is_current_job_version=false) are
/// re-activated when the same mapping is upserted again.
#[tokio::test]
async fn io_mapping_reactivates_stale_mapping() {
    let db = TestDb::new().await;
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));
    let input_ds = format!("input_ds_{}", rand::random_range(0..u32::MAX));

    // First event — creates IO mapping
    let run_id_1 = Uuid::new_v4();
    let event1 = LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now(),
        run: RunRef {
            run_id: run_id_1.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: Some(vec![InputDatasetRef {
            namespace: ns.clone(),
            name: input_ds.clone(),
            facets: None,
            input_facets: None,
        }]),
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    };
    openlineage::update_marquez_model(&db.pool, &event1)
        .await
        .unwrap();

    let job_row = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap()
        .unwrap();
    let jv_uuid_1 = job_row.current_version_uuid.unwrap();

    // Manually mark the IO mapping as not current (simulating staleness)
    sqlx::query(
        "UPDATE job_versions_io_mapping \
         SET is_current_job_version = false \
         WHERE job_version_uuid = $1",
    )
    .bind(jv_uuid_1)
    .execute(&db.pool)
    .await
    .unwrap();

    // Second event with same datasets — should re-activate the stale mapping
    let run_id_2 = Uuid::new_v4();
    let event2 = LineageEvent {
        event_type: Some("COMPLETE".to_string()),
        event_time: Utc::now() + Duration::seconds(1),
        run: RunRef {
            run_id: run_id_2.to_string(),
            facets: None,
        },
        job: JobRef {
            namespace: ns.clone(),
            name: job_name.clone(),
            facets: None,
        },
        inputs: Some(vec![InputDatasetRef {
            namespace: ns.clone(),
            name: input_ds.clone(),
            facets: None,
            input_facets: None,
        }]),
        outputs: None,
        producer: "test-producer".to_string(),
        schema_url: None,
    };
    openlineage::update_marquez_model(&db.pool, &event2)
        .await
        .unwrap();

    // The new job version should have current IO mappings
    let job_row_2 = marquez_api::db::job::find_by_name(&db.pool, &ns, &job_name)
        .await
        .unwrap()
        .unwrap();
    let jv_uuid_2 = job_row_2.current_version_uuid.unwrap();

    let current_mappings: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT dataset_uuid, io_type FROM job_versions_io_mapping \
         WHERE job_version_uuid = $1 AND is_current_job_version = true",
    )
    .bind(jv_uuid_2)
    .fetch_all(&db.pool)
    .await
    .unwrap();

    assert_eq!(
        current_mappings.len(),
        1,
        "IO mapping should be re-activated (Bug 45)"
    );
}

// ---------------------------------------------------------------------------
// Bug 53: upsert_namespace_inline undeletes hidden namespaces
// ---------------------------------------------------------------------------

/// Bug 53: Verify that a soft-deleted namespace is undeleted when referenced
/// by a new OL event.
#[tokio::test]
async fn ol_event_undeletes_hidden_namespace() {
    let db = TestDb::new().await;
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));
    let run_id = Uuid::new_v4();

    // Create namespace via OL event
    let event = make_start_event(&ns, &job_name, run_id);
    openlineage::update_marquez_model(&db.pool, &event)
        .await
        .unwrap();

    // Soft-delete the namespace
    marquez_api::db::namespace::delete(&db.pool, &ns)
        .await
        .unwrap();

    // Verify it's hidden (find_by_name queries raw table, so check is_hidden flag)
    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap()
        .expect("Namespace row should exist in raw table");
    assert_eq!(
        ns_row.is_hidden,
        Some(true),
        "Deleted namespace should be hidden"
    );

    // Send another OL event referencing the same namespace
    let run_id_2 = Uuid::new_v4();
    let event2 = make_start_event(&ns, &job_name, run_id_2);
    openlineage::update_marquez_model(&db.pool, &event2)
        .await
        .unwrap();

    // Namespace should be visible again (undeleted)
    let ns_row = marquez_api::db::namespace::find_by_name(&db.pool, &ns)
        .await
        .unwrap()
        .expect("Namespace row should exist");
    assert_eq!(
        ns_row.is_hidden,
        Some(false),
        "Namespace should be undeleted after OL event references it (Bug 53)"
    );
}

// ---------------------------------------------------------------------------
// Bug 43: Job upsert sets current_run_uuid
// ---------------------------------------------------------------------------

/// Bug 43: Verify that OL events set current_run_uuid on the job.
#[tokio::test]
async fn ol_event_sets_current_run_uuid() {
    let db = TestDb::new().await;
    let run_id = Uuid::new_v4();
    let ns = format!("test_ns_{}", rand::random_range(0..u32::MAX));
    let job_name = format!("test_job_{}", rand::random_range(0..u32::MAX));

    let event = make_start_event(&ns, &job_name, run_id);
    openlineage::update_marquez_model(&db.pool, &event)
        .await
        .unwrap();

    // Check the raw job row for current_run_uuid
    let row: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT current_run_uuid FROM jobs WHERE name = $1 AND namespace_name = $2")
            .bind(&job_name)
            .bind(&ns)
            .fetch_optional(&db.pool)
            .await
            .unwrap();

    assert!(row.is_some(), "Job row should exist");
    let current_run_uuid = row.unwrap().0;
    assert_eq!(
        current_run_uuid,
        Some(run_id),
        "Job should have current_run_uuid set to the run's UUID (Bug 43)"
    );
}
