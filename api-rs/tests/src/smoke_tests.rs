#[cfg(test)]
mod tests {
    use crate::common::TestDb;
    use crate::generators;
    use crate::TestApp;

    #[tokio::test]
    async fn test_db_starts_and_migrates() {
        let db = TestDb::new().await;
        // If we got here, the DB started and migrations ran successfully
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
        )
        .fetch_one(&db.pool)
        .await
        .expect("query failed");
        assert!(result > 0, "Expected tables to exist after migration");
    }

    #[tokio::test]
    async fn test_generators_produce_valid_data() {
        let ns = generators::new_namespace_name();
        assert!(!ns.is_empty());
        assert!(ns.starts_with("s3://test_namespace"));

        let ds = generators::new_dataset_name();
        assert!(!ds.is_empty());
        assert!(ds.starts_with("test_dataset"));

        let job = generators::new_job_name();
        assert!(!job.is_empty());
        assert!(job.starts_with("test_job"));

        let run_id = generators::new_run_id();
        assert!(!run_id.is_nil());

        let url = generators::new_connection_url();
        assert!(url.starts_with("postgresql://"));

        let desc = generators::new_description();
        assert!(!desc.is_empty());

        let owner = generators::new_owner_name();
        assert!(owner.starts_with("test_owner"));

        let source = generators::new_source_name();
        assert!(source.starts_with("test_source"));

        let field = generators::new_field_name();
        assert!(field.starts_with("test_field"));

        let tag = generators::new_tag_name();
        assert!(tag.starts_with("test_tag"));
    }

    #[tokio::test]
    async fn test_app_starts_and_responds() {
        let app = TestApp::new().await;

        // The app should respond to HTTP requests (even if with 404 since no routes are registered yet)
        let resp = app
            .client
            .get(&format!("{}/api/v1/namespaces", app.base_url))
            .send()
            .await
            .expect("request failed");

        // We expect either 200 or 404 since routes may not be implemented yet
        assert!(
            resp.status().is_success() || resp.status().as_u16() == 404,
            "Expected success or 404, got {}",
            resp.status()
        );
    }
}
