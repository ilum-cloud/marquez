use crate::common::TestDb;
use marquez_api::service::source::SourceService;

#[tokio::test]
async fn create_or_update() {
    let db = TestDb::new().await;
    let svc = SourceService::new(db.pool.clone());
    let src = svc
        .create_or_update(
            "POSTGRESQL",
            "my-src",
            "jdbc:postgresql://localhost/db",
            None,
        )
        .await
        .unwrap();
    assert_eq!(src.name, "my-src");
    assert_eq!(src.type_, "POSTGRESQL");
}

#[tokio::test]
async fn get_found() {
    let db = TestDb::new().await;
    let svc = SourceService::new(db.pool.clone());
    svc.create_or_update(
        "POSTGRESQL",
        "my-src",
        "jdbc:postgresql://localhost/db",
        None,
    )
    .await
    .unwrap();
    let src = svc.get("my-src").await.unwrap();
    assert_eq!(src.name, "my-src");
}

#[tokio::test]
async fn get_not_found() {
    let db = TestDb::new().await;
    let svc = SourceService::new(db.pool.clone());
    assert!(svc.get("nope").await.is_err());
}

#[tokio::test]
async fn list() {
    let db = TestDb::new().await;
    let svc = SourceService::new(db.pool.clone());
    svc.create_or_update("POSTGRESQL", "src-a", "url-a", None)
        .await
        .unwrap();
    svc.create_or_update("POSTGRESQL", "src-b", "url-b", None)
        .await
        .unwrap();
    let list = svc.list(10, 0).await.unwrap();
    assert!(list.len() >= 2);
}
