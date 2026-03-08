use crate::common::TestDb;
use crate::generators;
use marquez_api::service::tag::TagService;

#[tokio::test]
async fn create_or_update() {
    let db = TestDb::new().await;
    let svc = TagService::new(db.pool.clone());
    let name = generators::new_tag_name();
    let tag = svc
        .create_or_update(&name, Some("Personal data"))
        .await
        .unwrap();
    assert_eq!(tag.name, name);
    assert_eq!(tag.description, Some("Personal data".to_string()));
}

#[tokio::test]
async fn list() {
    let db = TestDb::new().await;
    let svc = TagService::new(db.pool.clone());
    let name_a = generators::new_tag_name();
    let name_b = generators::new_tag_name();
    svc.create_or_update(&name_a, None).await.unwrap();
    svc.create_or_update(&name_b, None).await.unwrap();
    let list = svc.list(10000, 0).await.unwrap();
    assert!(list.len() >= 2);
}
