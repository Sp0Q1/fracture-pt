use gethacked::app::App;
use gethacked::models::services;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serial_test::serial;

async fn create_service(db: &sea_orm::DatabaseConnection, slug: &str) -> services::Model {
    services::ActiveModel {
        name: Set(format!("Service {slug}")),
        slug: Set(slug.to_string()),
        category: Set("pentest".to_string()),
        description: Set(format!("Description for {slug}")),
        is_automated: Set(false),
        is_active: Set(true),
        sort_order: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("Failed to create service")
}

#[tokio::test]
#[serial]
async fn test_create_service() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "web-app-pentest").await;
    assert_eq!(svc.name, "Service web-app-pentest");
    assert_eq!(svc.slug, "web-app-pentest");
    assert_eq!(svc.category, "pentest");
    assert!(svc.is_active);
}

#[tokio::test]
#[serial]
async fn test_service_sets_pid_on_insert() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "pid-test").await;
    assert!(!svc.pid.is_nil());
}

#[tokio::test]
#[serial]
async fn test_find_service_by_slug() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    create_service(db, "find-by-slug").await;
    let found = services::Model::find_by_slug(db, "find-by-slug").await;
    assert!(found.is_some());
    assert_eq!(found.unwrap().slug, "find-by-slug");

    let not_found = services::Model::find_by_slug(db, "nonexistent").await;
    assert!(not_found.is_none());
}

#[tokio::test]
#[serial]
async fn test_find_active_services() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    create_service(db, "active-svc").await;

    // Create an inactive service
    services::ActiveModel {
        name: Set("Inactive".to_string()),
        slug: Set("inactive-svc".to_string()),
        category: Set("pentest".to_string()),
        description: Set("Inactive service".to_string()),
        is_automated: Set(false),
        is_active: Set(false),
        sort_order: Set(1),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let active = services::Model::find_active(db).await;
    assert!(active.iter().all(|s| s.is_active));
    assert!(active.iter().any(|s| s.slug == "active-svc"));
    assert!(!active.iter().any(|s| s.slug == "inactive-svc"));
}

#[tokio::test]
#[serial]
async fn test_service_slug_unique_constraint() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    create_service(db, "unique-slug").await;

    let result = services::ActiveModel {
        name: Set("Duplicate".to_string()),
        slug: Set("unique-slug".to_string()),
        category: Set("pentest".to_string()),
        description: Set("Duplicate".to_string()),
        is_automated: Set(false),
        is_active: Set(true),
        sort_order: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await;

    assert!(result.is_err(), "Duplicate slug should be rejected");
}

#[tokio::test]
#[serial]
async fn test_find_service_by_pid() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "by-pid").await;
    let found = services::Model::find_by_pid(db, &svc.pid.to_string()).await;
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, svc.id);

    let not_found = services::Model::find_by_pid(db, "00000000-0000-0000-0000-000000000000").await;
    assert!(not_found.is_none());
}
