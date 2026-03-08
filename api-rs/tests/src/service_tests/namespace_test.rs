use crate::common::TestDb;
use crate::generators;
use marquez_api::service::namespace::NamespaceService;

#[tokio::test]
async fn create_or_update() {
    let db = TestDb::new().await;
    let svc = NamespaceService::new(db.pool.clone());
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let ns = svc.create_or_update(&name, &owner, None).await.unwrap();
    assert_eq!(ns.name, name);
    assert_eq!(ns.owner_name, owner);
}

#[tokio::test]
async fn create_or_update_with_description() {
    let db = TestDb::new().await;
    let svc = NamespaceService::new(db.pool.clone());
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    let ns = svc
        .create_or_update(&name, &owner, Some("desc"))
        .await
        .unwrap();
    assert_eq!(ns.description, Some("desc".to_string()));
}

#[tokio::test]
async fn get_found() {
    let db = TestDb::new().await;
    let svc = NamespaceService::new(db.pool.clone());
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    svc.create_or_update(&name, &owner, None).await.unwrap();
    let ns = svc.get(&name).await.unwrap();
    assert_eq!(ns.name, name);
}

#[tokio::test]
async fn get_not_found() {
    let db = TestDb::new().await;
    let svc = NamespaceService::new(db.pool.clone());
    let result = svc.get("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list() {
    let db = TestDb::new().await;
    let svc = NamespaceService::new(db.pool.clone());
    let name_a = generators::new_namespace_name();
    let name_b = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    svc.create_or_update(&name_a, &owner, None).await.unwrap();
    svc.create_or_update(&name_b, &owner, None).await.unwrap();
    let list = svc.list(10000, 0).await.unwrap();
    assert!(list.len() >= 2);
}

#[tokio::test]
async fn delete() {
    let db = TestDb::new().await;
    let svc = NamespaceService::new(db.pool.clone());
    let name = generators::new_namespace_name();
    let owner = generators::new_owner_name();
    svc.create_or_update(&name, &owner, None).await.unwrap();
    svc.delete(&name).await.unwrap();
    // After delete, namespace should be hidden (soft-delete)
}
