// SPDX-License-Identifier: Apache-2.0

//! DAO-level test helpers that create fully-formed rows using the actual
//! DAO functions and random test data from generators.

use chrono::Utc;
use marquez_api::db::{
    dataset, dataset_field, dataset_version, job, job_version, namespace, run, source,
};
use marquez_api::models::db::{
    DatasetFieldRow, DatasetRow, DatasetSchemaVersionRow, DatasetSymlinkRow, DatasetVersionRow,
    JobRow, JobVersionRow, NamespaceRow, RunRow, SourceRow,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::generators;

/// Create a namespace with random test data via the namespace DAO.
pub async fn create_namespace(pool: &PgPool) -> NamespaceRow {
    namespace::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_namespace_name(),
        &generators::new_owner_name(),
        None,
    )
    .await
    .expect("fixture: create_namespace failed")
}

/// Create a source with random test data via the source DAO.
pub async fn create_source(pool: &PgPool) -> SourceRow {
    source::upsert(
        pool,
        Uuid::new_v4(),
        &generators::new_source_type(),
        Utc::now(),
        &generators::new_source_name(),
        &generators::new_connection_url(),
        None,
    )
    .await
    .expect("fixture: create_source failed")
}

/// Create a dataset with random test data.
///
/// Also creates a primary symlink so the dataset appears in `datasets_view`.
pub async fn create_dataset(pool: &PgPool, ns: &NamespaceRow, src: &SourceRow) -> DatasetRow {
    let uuid = Uuid::new_v4();
    let name = generators::new_dataset_name();
    let physical_name = generators::new_physical_name();
    let now = Utc::now();

    let row = dataset::upsert(
        pool,
        uuid,
        &generators::new_dataset_type(),
        now,
        ns.uuid,
        &ns.name,
        src.uuid,
        &src.name,
        &name,
        &physical_name,
        None,
        false,
    )
    .await
    .expect("fixture: create_dataset failed");

    // Create the primary symlink so the dataset appears in datasets_view.
    dataset_version::upsert_symlink(pool, uuid, &name, ns.uuid, None, true, now)
        .await
        .expect("fixture: create_dataset symlink failed");

    row
}

/// Create a dataset field with random test data.
pub async fn create_dataset_field(pool: &PgPool, dataset: &DatasetRow) -> DatasetFieldRow {
    dataset_field::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_field_name(),
        Some("VARCHAR"),
        None,
        dataset.uuid,
    )
    .await
    .expect("fixture: create_dataset_field failed")
}

/// Create a dataset version with random test data.
pub async fn create_dataset_version(
    pool: &PgPool,
    dataset: &DatasetRow,
    run_uuid: Option<Uuid>,
) -> DatasetVersionRow {
    dataset_version::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        dataset.uuid,
        generators::new_version(),
        None,
        run_uuid,
        None,
        dataset.namespace_name.as_deref().unwrap_or("default"),
        &dataset.name,
        None,
    )
    .await
    .expect("fixture: create_dataset_version failed")
}

/// Create a dataset symlink with random test data.
pub async fn create_symlink(pool: &PgPool, name: &str, ns_uuid: Uuid) -> DatasetSymlinkRow {
    dataset_version::upsert_symlink(pool, Uuid::new_v4(), name, ns_uuid, None, false, Utc::now())
        .await
        .expect("fixture: create_symlink failed")
}

/// Create a dataset schema version.
pub async fn create_schema_version(pool: &PgPool, dataset: &DatasetRow) -> DatasetSchemaVersionRow {
    dataset_version::upsert_schema_version(pool, Uuid::new_v4(), dataset.uuid, Utc::now())
        .await
        .expect("fixture: create_schema_version failed")
        .expect("fixture: create_schema_version returned None")
}

/// Create a job with random test data via the job DAO (through `jobs_view`).
pub async fn create_job(pool: &PgPool, ns: &NamespaceRow) -> JobRow {
    let name = generators::new_job_name();
    job::upsert(
        pool,
        Uuid::new_v4(),
        &generators::new_job_type(),
        Utc::now(),
        ns.uuid,
        &ns.name,
        &name,
        None,
        None,
        None,
        Some(&name),
        None,
        None,
    )
    .await
    .expect("fixture: create_job failed")
}

/// Create a run with random test data via the run DAO.
///
/// Creates a minimal run associated with the given job.
pub async fn create_run(pool: &PgPool, job: &JobRow) -> RunRow {
    run::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        Some(job.uuid),
        None,
        None,
        None,
        None,
        None,
        Some("NEW"),
        None,
        None,
        None,
        None,
        job.namespace_name.as_deref().unwrap_or("default"),
        &job.name,
        None,
        None,
    )
    .await
    .expect("fixture: create_run failed")
}

/// Create a job version with random test data via the job_version DAO.
pub async fn create_job_version(pool: &PgPool, job: &JobRow) -> JobVersionRow {
    job_version::upsert(
        pool,
        Uuid::new_v4(),
        Utc::now(),
        job.uuid,
        None,
        generators::new_version(),
        None,
        job.namespace_uuid,
        job.namespace_name.as_deref().unwrap_or("default"),
        &job.name,
    )
    .await
    .expect("fixture: create_job_version failed")
}
