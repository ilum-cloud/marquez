// SPDX-License-Identifier: Apache-2.0

use crate::common::TestDb;
use crate::fixtures;
use crate::generators;
use chrono::Utc;
use marquez_api::db::{job, tag};
use sqlx::Executor;
use uuid::Uuid;

#[tokio::test]
async fn upsert_creates_via_jobs_view() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    let uuid = Uuid::new_v4();
    let name = generators::new_job_name();
    let now = Utc::now();

    let row = job::upsert(
        &db.pool,
        uuid,
        "BATCH",
        now,
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
    .unwrap();

    assert_eq!(row.type_, "BATCH");
    assert_eq!(row.name, name);
    assert_eq!(row.namespace_name.as_deref(), Some(ns.name.as_str()));
    assert_eq!(row.simple_name.as_deref(), Some(name.as_str()));
    assert_eq!(row.is_hidden, Some(false));
}

#[tokio::test]
async fn upsert_updates_on_conflict() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    let name = generators::new_job_name();
    let now = Utc::now();

    let row1 = job::upsert(
        &db.pool,
        Uuid::new_v4(),
        "BATCH",
        now,
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
    .unwrap();

    // Upsert again with same name/namespace but different type.
    let row2 = job::upsert(
        &db.pool,
        Uuid::new_v4(),
        "STREAM",
        Utc::now(),
        ns.uuid,
        &ns.name,
        &name,
        Some("updated description"),
        Some("https://new-location"),
        None,
        Some(&name),
        None,
        None,
    )
    .await
    .unwrap();

    // UUID should be the same (conflict resolved).
    assert_eq!(row1.uuid, row2.uuid);
    assert_eq!(row2.type_, "STREAM");
    assert_eq!(row2.description.as_deref(), Some("updated description"));
    assert_eq!(
        row2.current_location.as_deref(),
        Some("https://new-location")
    );
}

#[tokio::test]
async fn upsert_with_description() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let desc = generators::new_description();
    let name = generators::new_job_name();

    let row = job::upsert(
        &db.pool,
        Uuid::new_v4(),
        "BATCH",
        Utc::now(),
        ns.uuid,
        &ns.name,
        &name,
        Some(&desc),
        None,
        None,
        Some(&name),
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(row.description.as_deref(), Some(desc.as_str()));
}

#[tokio::test]
async fn exists_true() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let j = fixtures::create_job(&db.pool, &ns).await;

    assert!(job::exists(&db.pool, &ns.name, &j.name).await.unwrap());
}

#[tokio::test]
async fn exists_false() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    assert!(!job::exists(&db.pool, &ns.name, "nonexistent_job")
        .await
        .unwrap());
}

#[tokio::test]
async fn find_by_name() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let j = fixtures::create_job(&db.pool, &ns).await;

    let found = job::find_by_name(&db.pool, &ns.name, &j.name)
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().uuid, j.uuid);
}

#[tokio::test]
async fn find_by_name_not_found() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    let found = job::find_by_name(&db.pool, &ns.name, "nonexistent_job")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn find_by_name_as_row() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let j = fixtures::create_job(&db.pool, &ns).await;

    let found = job::find_by_name_as_row(&db.pool, &ns.name, &j.name)
        .await
        .unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.uuid, j.uuid);
    assert_eq!(found.name, j.name);
}

#[tokio::test]
async fn find_all_pagination() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    for _ in 0..3 {
        fixtures::create_job(&db.pool, &ns).await;
    }

    let page1 = job::find_all(&db.pool, &ns.name, 2, 0, &[]).await.unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = job::find_all(&db.pool, &ns.name, 2, 2, &[]).await.unwrap();
    assert_eq!(page2.len(), 1);
}

#[tokio::test]
async fn count() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    fixtures::create_job(&db.pool, &ns).await;
    fixtures::create_job(&db.pool, &ns).await;

    let n = job::count(&db.pool, &ns.name).await.unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn count_empty() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;

    let n = job::count(&db.pool, &ns.name).await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn delete_soft_delete() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let j = fixtures::create_job(&db.pool, &ns).await;

    // Verify it exists before delete.
    assert!(job::exists(&db.pool, &ns.name, &j.name).await.unwrap());

    job::delete(&db.pool, &ns.name, &j.name).await.unwrap();

    // Should no longer appear in jobs_view (is_hidden = true).
    assert!(!job::exists(&db.pool, &ns.name, &j.name).await.unwrap());
}

#[tokio::test]
async fn update_version() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let j = fixtures::create_job(&db.pool, &ns).await;
    let version_uuid = Uuid::new_v4();

    job::update_version(&db.pool, j.uuid, Utc::now(), version_uuid)
        .await
        .unwrap();

    let found = job::find_by_name_as_row(&db.pool, &ns.name, &j.name)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.current_version_uuid, Some(version_uuid));
}

#[tokio::test]
async fn update_job_tag() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let j = fixtures::create_job(&db.pool, &ns).await;
    let t = tag::upsert(
        &db.pool,
        Uuid::new_v4(),
        Utc::now(),
        &generators::new_tag_name(),
        None,
    )
    .await
    .unwrap();

    job::update_job_tag(&db.pool, j.uuid, t.uuid, Utc::now())
        .await
        .unwrap();

    // Tagging again should not error (ON CONFLICT DO NOTHING).
    job::update_job_tag(&db.pool, j.uuid, t.uuid, Utc::now())
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Bug 43: Job upsert sets current_run_uuid
// ---------------------------------------------------------------------------

/// Bug 43: Verify that job::upsert stores current_run_uuid.
#[tokio::test]
async fn upsert_with_current_run_uuid() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let name = generators::new_job_name();
    let run_uuid = Uuid::new_v4();

    job::upsert(
        &db.pool,
        Uuid::new_v4(),
        "BATCH",
        Utc::now(),
        ns.uuid,
        &ns.name,
        &name,
        None,
        None,
        None,
        Some(&name),
        None,
        Some(run_uuid),
    )
    .await
    .unwrap();

    // Check the raw column
    let row: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT current_run_uuid FROM jobs WHERE name = $1 AND namespace_name = $2")
            .bind(&name)
            .bind(&ns.name)
            .fetch_optional(&db.pool)
            .await
            .unwrap();

    assert!(row.is_some());
    assert_eq!(
        row.unwrap().0,
        Some(run_uuid),
        "current_run_uuid should be stored (Bug 43)"
    );
}

// ---------------------------------------------------------------------------
// Bug 58: Job list ordered by updated_at DESC (reverts Bug 52)
// ---------------------------------------------------------------------------

/// Bug 58: Verify that find_all returns jobs ordered by updated_at DESC.
/// Java's JobDao.findAll() uses `ORDER BY j.updated_at DESC`.
#[tokio::test]
async fn find_all_order_by_updated_at_desc() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let prefix = format!("order_{}", rand::random_range(0..u32::MAX));

    // Create jobs with names that sort alphabetically opposite to creation order
    // so we can distinguish between name ASC and updated_at DESC ordering.
    let mut created_names = Vec::new();
    for suffix in ["ccc", "aaa", "bbb"] {
        let name = format!("{}_{}", prefix, suffix);
        job::upsert(
            &db.pool,
            Uuid::new_v4(),
            "BATCH",
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
        .unwrap();
        created_names.push(name);
    }

    let jobs = job::find_all(&db.pool, &ns.name, 100, 0, &[])
        .await
        .unwrap();
    let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();

    // Verify ordering is by updated_at DESC (most recently created/updated first).
    // Jobs were created in order ccc, aaa, bbb so bbb has the latest updated_at.
    assert_eq!(
        names.first().copied(),
        Some(format!("{}_{}", prefix, "bbb").as_str()),
        "Most recently updated job should be first (Bug 58: ORDER BY updated_at DESC)"
    );
}

// ---------------------------------------------------------------------------
// Bug: Job tags missing when querying by alias
// ---------------------------------------------------------------------------

/// Verify that tags are returned when a job is queried by alias name,
/// not just by its primary name. Previously the job_tags CTE filtered on
/// `simple_name` only, which doesn't match aliases.
#[tokio::test]
async fn find_job_by_alias_returns_tags() {
    let db = TestDb::new().await;
    let ns = fixtures::create_namespace(&db.pool).await;
    let name = generators::new_job_name();

    // Create a job.
    let job_row = job::upsert(
        &db.pool,
        Uuid::new_v4(),
        "BATCH",
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
    .unwrap();

    // Tag the job.
    let tag_name = generators::new_tag_name();
    tag::upsert(&db.pool, Uuid::new_v4(), Utc::now(), &tag_name, None)
        .await
        .unwrap();
    job::update_job_tags_now(
        &db.pool,
        &ns.name,
        &name,
        &tag_name,
        Utc::now(),
        Uuid::new_v4(),
    )
    .await
    .unwrap();

    // Manually set an alias on the job (normally done by the jobs_view trigger).
    let alias = format!("alias_{}", generators::new_job_name());
    sqlx::query("UPDATE jobs SET aliases = ARRAY[$1::text] WHERE uuid = $2")
        .bind(&alias)
        .bind(job_row.uuid)
        .execute(&db.pool)
        .await
        .unwrap();

    // Query by alias name — tags should still be present.
    let result = job::find_job_by_name_with_facets(&db.pool, &ns.name, &alias)
        .await
        .unwrap();
    assert!(result.is_some(), "should find job by alias name");

    let row = result.unwrap();
    let tags = row.tags.unwrap_or_default();
    assert!(
        tags.contains(&tag_name),
        "tags should include '{}' when queried by alias, got {:?}",
        tag_name,
        tags
    );
}
