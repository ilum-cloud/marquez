// SPDX-License-Identifier: Apache-2.0

//! Roundtrip tests: INSERT via raw SQL -> SELECT via `query_as` -> verify
//! fields match.
//!
//! Each test spins up a fresh TestContainers Postgres instance with all
//! Flyway migrations applied (via `TestDb::new()`).

use crate::common::TestDb;
use crate::generators;
use chrono::Utc;
use marquez_api::models::db::*;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// namespaces
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_namespace() {
    let db = TestDb::new().await;

    let id = Uuid::new_v4();
    let name = generators::new_namespace_name();
    let desc = generators::new_description();
    let owner = generators::new_owner_name();

    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name, description, current_owner_name, is_hidden)
         VALUES ($1, NOW(), NOW(), $2, $3, $4, false)",
    )
    .bind(id)
    .bind(&name)
    .bind(&desc)
    .bind(&owner)
    .execute(&db.pool)
    .await
    .expect("insert namespace");

    let row: NamespaceRow = sqlx::query_as("SELECT * FROM namespaces WHERE uuid = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .expect("select namespace");

    assert_eq!(row.uuid, id);
    assert_eq!(row.name, name);
    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
    assert_eq!(row.current_owner_name.as_deref(), Some(owner.as_str()));
    assert_eq!(row.is_hidden, Some(false));
}

// ---------------------------------------------------------------------------
// owners
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_owner() {
    let db = TestDb::new().await;

    let id = Uuid::new_v4();
    let name = generators::new_owner_name();

    sqlx::query(
        "INSERT INTO owners (uuid, created_at, name)
         VALUES ($1, NOW(), $2)",
    )
    .bind(id)
    .bind(&name)
    .execute(&db.pool)
    .await
    .expect("insert owner");

    let row: OwnerRow = sqlx::query_as("SELECT * FROM owners WHERE uuid = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .expect("select owner");

    assert_eq!(row.uuid, id);
    assert_eq!(row.name, name);
}

// ---------------------------------------------------------------------------
// sources
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_source() {
    let db = TestDb::new().await;

    let id = Uuid::new_v4();
    let name = generators::new_source_name();
    let url = generators::new_connection_url();
    let desc = generators::new_description();

    sqlx::query(
        "INSERT INTO sources (uuid, type, created_at, updated_at, name, connection_url, description)
         VALUES ($1, 'POSTGRESQL', NOW(), NOW(), $2, $3, $4)",
    )
    .bind(id)
    .bind(&name)
    .bind(&url)
    .bind(&desc)
    .execute(&db.pool)
    .await
    .expect("insert source");

    let row: SourceRow = sqlx::query_as("SELECT * FROM sources WHERE uuid = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .expect("select source");

    assert_eq!(row.uuid, id);
    assert_eq!(row.type_, "POSTGRESQL");
    assert_eq!(row.name, name);
    assert_eq!(row.connection_url, url);
    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

// ---------------------------------------------------------------------------
// tags
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_tag() {
    let db = TestDb::new().await;

    let id = Uuid::new_v4();
    let name = generators::new_tag_name();
    let desc = generators::new_description();

    sqlx::query(
        "INSERT INTO tags (uuid, created_at, updated_at, name, description)
         VALUES ($1, NOW(), NOW(), $2, $3)",
    )
    .bind(id)
    .bind(&name)
    .bind(&desc)
    .execute(&db.pool)
    .await
    .expect("insert tag");

    let row: TagRow = sqlx::query_as("SELECT * FROM tags WHERE uuid = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .expect("select tag");

    assert_eq!(row.uuid, id);
    assert_eq!(row.name, name);
    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

// ---------------------------------------------------------------------------
// datasets
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_dataset() {
    let db = TestDb::new().await;

    // Create prerequisite namespace and source.
    let ns_uuid = Uuid::new_v4();
    let ns_name = generators::new_namespace_name();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name, current_owner_name)
         VALUES ($1, NOW(), NOW(), $2, 'owner')",
    )
    .bind(ns_uuid)
    .bind(&ns_name)
    .execute(&db.pool)
    .await
    .expect("insert namespace");

    let src_uuid = Uuid::new_v4();
    let src_name = generators::new_source_name();
    let conn = generators::new_connection_url();
    sqlx::query(
        "INSERT INTO sources (uuid, type, created_at, updated_at, name, connection_url)
         VALUES ($1, 'POSTGRESQL', NOW(), NOW(), $2, $3)",
    )
    .bind(src_uuid)
    .bind(&src_name)
    .bind(&conn)
    .execute(&db.pool)
    .await
    .expect("insert source");

    let ds_uuid = Uuid::new_v4();
    let ds_name = generators::new_dataset_name();
    let desc = generators::new_description();

    sqlx::query(
        "INSERT INTO datasets
           (uuid, type, created_at, updated_at, namespace_uuid, namespace_name,
            source_uuid, source_name, name, physical_name, description,
            is_deleted, is_hidden)
         VALUES ($1, 'DB_TABLE', NOW(), NOW(), $2, $3, $4, $5, $6, $6, $7, false, false)",
    )
    .bind(ds_uuid)
    .bind(ns_uuid)
    .bind(&ns_name)
    .bind(src_uuid)
    .bind(&src_name)
    .bind(&ds_name)
    .bind(&desc)
    .execute(&db.pool)
    .await
    .expect("insert dataset");

    let row: DatasetRow = sqlx::query_as("SELECT * FROM datasets WHERE uuid = $1")
        .bind(ds_uuid)
        .fetch_one(&db.pool)
        .await
        .expect("select dataset");

    assert_eq!(row.uuid, ds_uuid);
    assert_eq!(row.type_, "DB_TABLE");
    assert_eq!(row.name, ds_name);
    assert_eq!(row.physical_name, ds_name);
    assert_eq!(row.namespace_uuid, Some(ns_uuid));
    assert_eq!(row.namespace_name.as_deref(), Some(ns_name.as_str()));
    assert_eq!(row.source_uuid, Some(src_uuid));
    assert_eq!(row.source_name.as_deref(), Some(src_name.as_str()));
    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
    assert_eq!(row.is_deleted, Some(false));
    assert_eq!(row.is_hidden, Some(false));
}

// ---------------------------------------------------------------------------
// dataset_versions
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_dataset_version() {
    let db = TestDb::new().await;

    // Create prerequisite: namespace -> source -> dataset.
    let ns_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name)
         VALUES ($1, NOW(), NOW(), $2)",
    )
    .bind(ns_uuid)
    .bind(generators::new_namespace_name())
    .execute(&db.pool)
    .await
    .unwrap();

    let src_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sources (uuid, type, created_at, updated_at, name, connection_url)
         VALUES ($1, 'POSTGRESQL', NOW(), NOW(), $2, $3)",
    )
    .bind(src_uuid)
    .bind(generators::new_source_name())
    .bind(generators::new_connection_url())
    .execute(&db.pool)
    .await
    .unwrap();

    let ds_uuid = Uuid::new_v4();
    let ds_name = generators::new_dataset_name();
    sqlx::query(
        "INSERT INTO datasets (uuid, type, created_at, updated_at, namespace_uuid, source_uuid, name, physical_name)
         VALUES ($1, 'DB_TABLE', NOW(), NOW(), $2, $3, $4, $4)",
    )
    .bind(ds_uuid)
    .bind(ns_uuid)
    .bind(src_uuid)
    .bind(&ds_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let dv_uuid = Uuid::new_v4();
    let version = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO dataset_versions (uuid, created_at, dataset_uuid, version, namespace_name, dataset_name)
         VALUES ($1, NOW(), $2, $3, 'ns', $4)",
    )
    .bind(dv_uuid)
    .bind(ds_uuid)
    .bind(version)
    .bind(&ds_name)
    .execute(&db.pool)
    .await
    .expect("insert dataset_version");

    let row: DatasetVersionRow = sqlx::query_as("SELECT * FROM dataset_versions WHERE uuid = $1")
        .bind(dv_uuid)
        .fetch_one(&db.pool)
        .await
        .expect("select dataset_version");

    assert_eq!(row.uuid, dv_uuid);
    assert_eq!(row.dataset_uuid, Some(ds_uuid));
    assert_eq!(row.version, version);
    assert_eq!(row.dataset_name.as_deref(), Some(ds_name.as_str()));
}

// ---------------------------------------------------------------------------
// dataset_fields
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_dataset_field() {
    let db = TestDb::new().await;

    // Prerequisites: namespace -> source -> dataset.
    let ns_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name)
         VALUES ($1, NOW(), NOW(), $2)",
    )
    .bind(ns_uuid)
    .bind(generators::new_namespace_name())
    .execute(&db.pool)
    .await
    .unwrap();

    let src_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sources (uuid, type, created_at, updated_at, name, connection_url)
         VALUES ($1, 'POSTGRESQL', NOW(), NOW(), $2, $3)",
    )
    .bind(src_uuid)
    .bind(generators::new_source_name())
    .bind(generators::new_connection_url())
    .execute(&db.pool)
    .await
    .unwrap();

    let ds_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO datasets (uuid, type, created_at, updated_at, namespace_uuid, source_uuid, name, physical_name)
         VALUES ($1, 'DB_TABLE', NOW(), NOW(), $2, $3, $4, $4)",
    )
    .bind(ds_uuid)
    .bind(ns_uuid)
    .bind(src_uuid)
    .bind(generators::new_dataset_name())
    .execute(&db.pool)
    .await
    .unwrap();

    let field_uuid = Uuid::new_v4();
    let field_name = generators::new_field_name();
    let desc = generators::new_description();

    sqlx::query(
        "INSERT INTO dataset_fields (uuid, type, created_at, updated_at, dataset_uuid, name, description)
         VALUES ($1, 'VARCHAR', NOW(), NOW(), $2, $3, $4)",
    )
    .bind(field_uuid)
    .bind(ds_uuid)
    .bind(&field_name)
    .bind(&desc)
    .execute(&db.pool)
    .await
    .expect("insert dataset_field");

    let row: DatasetFieldRow = sqlx::query_as("SELECT * FROM dataset_fields WHERE uuid = $1")
        .bind(field_uuid)
        .fetch_one(&db.pool)
        .await
        .expect("select dataset_field");

    assert_eq!(row.uuid, field_uuid);
    assert_eq!(row.type_.as_deref(), Some("VARCHAR"));
    assert_eq!(row.dataset_uuid, Some(ds_uuid));
    assert_eq!(row.name, field_name);
    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

// ---------------------------------------------------------------------------
// jobs
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_job() {
    let db = TestDb::new().await;

    let ns_uuid = Uuid::new_v4();
    let ns_name = generators::new_namespace_name();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name)
         VALUES ($1, NOW(), NOW(), $2)",
    )
    .bind(ns_uuid)
    .bind(&ns_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let job_uuid = Uuid::new_v4();
    let job_name = generators::new_job_name();
    let desc = generators::new_description();

    sqlx::query(
        "INSERT INTO jobs
           (uuid, type, created_at, updated_at, namespace_uuid, namespace_name,
            name, simple_name, description, is_hidden)
         VALUES ($1, 'BATCH', NOW(), NOW(), $2, $3, $4, $4, $5, false)",
    )
    .bind(job_uuid)
    .bind(ns_uuid)
    .bind(&ns_name)
    .bind(&job_name)
    .bind(&desc)
    .execute(&db.pool)
    .await
    .expect("insert job");

    let row: JobRow = sqlx::query_as("SELECT * FROM jobs WHERE uuid = $1")
        .bind(job_uuid)
        .fetch_one(&db.pool)
        .await
        .expect("select job");

    assert_eq!(row.uuid, job_uuid);
    assert_eq!(row.type_, "BATCH");
    assert_eq!(row.name, job_name);
    assert_eq!(row.simple_name.as_deref(), Some(job_name.as_str()));
    assert_eq!(row.namespace_uuid, Some(ns_uuid));
    assert_eq!(row.namespace_name.as_deref(), Some(ns_name.as_str()));
    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
    assert_eq!(row.is_hidden, Some(false));
}

// ---------------------------------------------------------------------------
// job_versions
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_job_version() {
    let db = TestDb::new().await;

    // Prerequisites: namespace -> job.
    let ns_uuid = Uuid::new_v4();
    let ns_name = generators::new_namespace_name();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name)
         VALUES ($1, NOW(), NOW(), $2)",
    )
    .bind(ns_uuid)
    .bind(&ns_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let job_uuid = Uuid::new_v4();
    let job_name = generators::new_job_name();
    sqlx::query(
        "INSERT INTO jobs (uuid, type, created_at, updated_at, namespace_uuid, namespace_name, name, simple_name, is_hidden)
         VALUES ($1, 'BATCH', NOW(), NOW(), $2, $3, $4, $4, false)",
    )
    .bind(job_uuid)
    .bind(ns_uuid)
    .bind(&ns_name)
    .bind(&job_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let jv_uuid = Uuid::new_v4();
    let version = Uuid::new_v4();
    let location = "https://github.com/test/repo";

    sqlx::query(
        "INSERT INTO job_versions
           (uuid, created_at, updated_at, job_uuid, version, location,
            namespace_uuid, namespace_name, job_name)
         VALUES ($1, NOW(), NOW(), $2, $3, $4, $5, $6, $7)",
    )
    .bind(jv_uuid)
    .bind(job_uuid)
    .bind(version)
    .bind(location)
    .bind(ns_uuid)
    .bind(&ns_name)
    .bind(&job_name)
    .execute(&db.pool)
    .await
    .expect("insert job_version");

    let row: JobVersionRow = sqlx::query_as("SELECT * FROM job_versions WHERE uuid = $1")
        .bind(jv_uuid)
        .fetch_one(&db.pool)
        .await
        .expect("select job_version");

    assert_eq!(row.uuid, jv_uuid);
    assert_eq!(row.job_uuid, Some(job_uuid));
    assert_eq!(row.version, version);
    assert_eq!(row.location.as_deref(), Some(location));
    assert_eq!(row.namespace_uuid, Some(ns_uuid));
    assert_eq!(row.namespace_name.as_deref(), Some(ns_name.as_str()));
    assert_eq!(row.job_name.as_deref(), Some(job_name.as_str()));
}

// ---------------------------------------------------------------------------
// runs
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_run() {
    let db = TestDb::new().await;

    // Prerequisites: namespace -> job.
    let ns_uuid = Uuid::new_v4();
    let ns_name = generators::new_namespace_name();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name)
         VALUES ($1, NOW(), NOW(), $2)",
    )
    .bind(ns_uuid)
    .bind(&ns_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let job_uuid = Uuid::new_v4();
    let job_name = generators::new_job_name();
    sqlx::query(
        "INSERT INTO jobs (uuid, type, created_at, updated_at, namespace_uuid, namespace_name, name, simple_name, is_hidden)
         VALUES ($1, 'BATCH', NOW(), NOW(), $2, $3, $4, $4, false)",
    )
    .bind(job_uuid)
    .bind(ns_uuid)
    .bind(&ns_name)
    .bind(&job_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let run_uuid = Uuid::new_v4();
    let ext_id = format!("ext-{}", Uuid::new_v4());

    sqlx::query(
        "INSERT INTO runs
           (uuid, created_at, updated_at, job_uuid, job_name, namespace_name,
            current_run_state, external_id, transitioned_at)
         VALUES ($1, NOW(), NOW(), $2, $3, $4, 'NEW', $5, NOW())",
    )
    .bind(run_uuid)
    .bind(job_uuid)
    .bind(&job_name)
    .bind(&ns_name)
    .bind(&ext_id)
    .execute(&db.pool)
    .await
    .expect("insert run");

    let row: RunRow = sqlx::query_as(
        "SELECT uuid, created_at, updated_at, job_uuid, job_version_uuid, \
                    parent_run_uuid, run_args_uuid, \
                    nominal_start_time::timestamptz AS nominal_start_time, \
                    nominal_end_time::timestamptz AS nominal_end_time, \
                    current_run_state, \
                    started_at::timestamptz AS started_at, \
                    start_run_state_uuid, \
                    ended_at::timestamptz AS ended_at, \
                    end_run_state_uuid, \
                    job_name, namespace_name, external_id, location, \
                    transitioned_at::timestamptz AS transitioned_at, \
                    job_context_uuid \
             FROM runs WHERE uuid = $1",
    )
    .bind(run_uuid)
    .fetch_one(&db.pool)
    .await
    .expect("select run");

    assert_eq!(row.uuid, run_uuid);
    assert_eq!(row.job_uuid, Some(job_uuid));
    assert_eq!(row.job_name.as_deref(), Some(job_name.as_str()));
    assert_eq!(row.namespace_name.as_deref(), Some(ns_name.as_str()));
    assert_eq!(row.current_run_state.as_deref(), Some("NEW"));
    assert_eq!(row.external_id.as_deref(), Some(ext_id.as_str()));
}

// ---------------------------------------------------------------------------
// run_states
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_run_state() {
    let db = TestDb::new().await;

    // Prerequisites: namespace -> job -> run.
    let ns_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name)
         VALUES ($1, NOW(), NOW(), $2)",
    )
    .bind(ns_uuid)
    .bind(generators::new_namespace_name())
    .execute(&db.pool)
    .await
    .unwrap();

    let job_uuid = Uuid::new_v4();
    let job_name = generators::new_job_name();
    sqlx::query(
        "INSERT INTO jobs (uuid, type, created_at, updated_at, namespace_uuid, name, simple_name, is_hidden)
         VALUES ($1, 'BATCH', NOW(), NOW(), $2, $3, $3, false)",
    )
    .bind(job_uuid)
    .bind(ns_uuid)
    .bind(&job_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let run_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO runs (uuid, created_at, updated_at, job_uuid, job_name, namespace_name, transitioned_at)
         VALUES ($1, NOW(), NOW(), $2, $3, 'ns', NOW())",
    )
    .bind(run_uuid)
    .bind(job_uuid)
    .bind(&job_name)
    .execute(&db.pool)
    .await
    .unwrap();

    let rs_uuid = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO run_states (uuid, transitioned_at, run_uuid, state)
         VALUES ($1, NOW(), $2, 'RUNNING')",
    )
    .bind(rs_uuid)
    .bind(run_uuid)
    .execute(&db.pool)
    .await
    .expect("insert run_state");

    let row: RunStateRow = sqlx::query_as("SELECT * FROM run_states WHERE uuid = $1")
        .bind(rs_uuid)
        .fetch_one(&db.pool)
        .await
        .expect("select run_state");

    assert_eq!(row.uuid, rs_uuid);
    assert_eq!(row.run_uuid, Some(run_uuid));
    assert_eq!(row.state, "RUNNING");
}

// ---------------------------------------------------------------------------
// column_lineage
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_column_lineage() {
    let db = TestDb::new().await;

    // Build prerequisite chain: namespace -> source -> dataset -> version + field.
    let ns_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO namespaces (uuid, created_at, updated_at, name)
         VALUES ($1, NOW(), NOW(), $2)",
    )
    .bind(ns_uuid)
    .bind(generators::new_namespace_name())
    .execute(&db.pool)
    .await
    .unwrap();

    let src_uuid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO sources (uuid, type, created_at, updated_at, name, connection_url)
         VALUES ($1, 'POSTGRESQL', NOW(), NOW(), $2, $3)",
    )
    .bind(src_uuid)
    .bind(generators::new_source_name())
    .bind(generators::new_connection_url())
    .execute(&db.pool)
    .await
    .unwrap();

    // Two datasets for input / output.
    let ds_out = Uuid::new_v4();
    let ds_in = Uuid::new_v4();
    for (ds, nm) in [
        (ds_out, generators::new_dataset_name()),
        (ds_in, generators::new_dataset_name()),
    ] {
        sqlx::query(
            "INSERT INTO datasets (uuid, type, created_at, updated_at, namespace_uuid, source_uuid, name, physical_name)
             VALUES ($1, 'DB_TABLE', NOW(), NOW(), $2, $3, $4, $4)",
        )
        .bind(ds)
        .bind(ns_uuid)
        .bind(src_uuid)
        .bind(&nm)
        .execute(&db.pool)
        .await
        .unwrap();
    }

    // Versions.
    let dv_out = Uuid::new_v4();
    let dv_in = Uuid::new_v4();
    for (dv, ds) in [(dv_out, ds_out), (dv_in, ds_in)] {
        sqlx::query(
            "INSERT INTO dataset_versions (uuid, created_at, dataset_uuid, version)
             VALUES ($1, NOW(), $2, $3)",
        )
        .bind(dv)
        .bind(ds)
        .bind(Uuid::new_v4())
        .execute(&db.pool)
        .await
        .unwrap();
    }

    // Fields.
    let f_out = Uuid::new_v4();
    let f_in = Uuid::new_v4();
    for (f, ds) in [(f_out, ds_out), (f_in, ds_in)] {
        sqlx::query(
            "INSERT INTO dataset_fields (uuid, type, created_at, updated_at, dataset_uuid, name)
             VALUES ($1, 'INT', NOW(), NOW(), $2, $3)",
        )
        .bind(f)
        .bind(ds)
        .bind(generators::new_field_name())
        .execute(&db.pool)
        .await
        .unwrap();
    }

    sqlx::query(
        "INSERT INTO column_lineage
           (output_dataset_version_uuid, output_dataset_field_uuid,
            input_dataset_version_uuid, input_dataset_field_uuid,
            transformation_description, transformation_type,
            created_at, updated_at)
         VALUES ($1, $2, $3, $4, 'identity', 'IDENTITY', NOW(), NOW())",
    )
    .bind(dv_out)
    .bind(f_out)
    .bind(dv_in)
    .bind(f_in)
    .execute(&db.pool)
    .await
    .expect("insert column_lineage");

    let row: ColumnLineageRow = sqlx::query_as(
        "SELECT * FROM column_lineage
         WHERE output_dataset_version_uuid = $1
           AND output_dataset_field_uuid = $2",
    )
    .bind(dv_out)
    .bind(f_out)
    .fetch_one(&db.pool)
    .await
    .expect("select column_lineage");

    assert_eq!(row.output_dataset_version_uuid, Some(dv_out));
    assert_eq!(row.output_dataset_field_uuid, Some(f_out));
    assert_eq!(row.input_dataset_version_uuid, Some(dv_in));
    assert_eq!(row.input_dataset_field_uuid, Some(f_in));
    assert_eq!(row.transformation_description.as_deref(), Some("identity"));
    assert_eq!(row.transformation_type.as_deref(), Some("IDENTITY"));
}

// ---------------------------------------------------------------------------
// lineage_events
// ---------------------------------------------------------------------------
#[tokio::test]
async fn roundtrip_lineage_event() {
    let db = TestDb::new().await;

    let event_time = Utc::now();
    let event_json = serde_json::json!({"eventType": "COMPLETE"});

    sqlx::query(
        "INSERT INTO lineage_events
           (event_time, event, event_type, job_name, job_namespace, producer)
         VALUES ($1, $2, 'COMPLETE', 'my_job', 'my_ns', 'my_producer')",
    )
    .bind(event_time)
    .bind(&event_json)
    .execute(&db.pool)
    .await
    .expect("insert lineage_event");

    let row: LineageEventRow = sqlx::query_as(
        "SELECT * FROM lineage_events WHERE job_name = 'my_job' AND job_namespace = 'my_ns' LIMIT 1",
    )
    .fetch_one(&db.pool)
    .await
    .expect("select lineage_event");

    assert_eq!(row.job_name.as_deref(), Some("my_job"));
    assert_eq!(row.job_namespace.as_deref(), Some("my_ns"));
    assert_eq!(row.event_type.as_deref(), Some("COMPLETE"));
    assert_eq!(row.producer.as_deref(), Some("my_producer"));
}
