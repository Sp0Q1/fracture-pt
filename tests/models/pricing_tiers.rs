use fracture_pt::app::App;
use fracture_pt::models::{pricing_tiers, services};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serial_test::serial;

async fn create_service(db: &sea_orm::DatabaseConnection, slug: &str) -> services::Model {
    services::ActiveModel {
        name: Set(format!("Service {slug}")),
        slug: Set(slug.to_string()),
        category: Set("pentest".to_string()),
        description: Set("Test service".to_string()),
        is_automated: Set(false),
        is_active: Set(true),
        sort_order: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("Failed to create service")
}

async fn create_tier(
    db: &sea_orm::DatabaseConnection,
    service_id: i32,
    slug: &str,
    price: i32,
    sort: i32,
) -> pricing_tiers::Model {
    pricing_tiers::ActiveModel {
        service_id: Set(service_id),
        name: Set(format!("Tier {slug}")),
        slug: Set(slug.to_string()),
        price_cents: Set(price),
        billing_period: Set("monthly".to_string()),
        max_targets: Set(5),
        max_scans_per_month: Set(10),
        features: Set("[]".to_string()),
        is_active: Set(true),
        sort_order: Set(sort),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("Failed to create tier")
}

#[tokio::test]
#[serial]
async fn test_create_pricing_tier() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "tier-svc").await;
    let tier = create_tier(db, svc.id, "starter", 4999, 0).await;

    assert_eq!(tier.name, "Tier starter");
    assert_eq!(tier.price_cents, 4999);
    assert_eq!(tier.max_scans_per_month, 10);
    assert!(tier.is_active);
}

#[tokio::test]
#[serial]
async fn test_tier_sets_pid_on_insert() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "tier-pid-svc").await;
    let tier = create_tier(db, svc.id, "pid-tier", 0, 0).await;
    assert!(!tier.pid.is_nil());
}

#[tokio::test]
#[serial]
async fn test_find_tiers_by_service() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "multi-tier-svc").await;
    create_tier(db, svc.id, "basic", 0, 0).await;
    create_tier(db, svc.id, "pro", 9999, 1).await;
    create_tier(db, svc.id, "enterprise", 29999, 2).await;

    let tiers = pricing_tiers::Model::find_by_service(db, svc.id).await;
    assert_eq!(tiers.len(), 3);
    // Should be ordered by sort_order
    assert_eq!(tiers[0].slug, "basic");
    assert_eq!(tiers[1].slug, "pro");
    assert_eq!(tiers[2].slug, "enterprise");
}

#[tokio::test]
#[serial]
async fn test_find_active_tiers_by_service() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "active-tier-svc").await;
    create_tier(db, svc.id, "active-tier", 4999, 0).await;

    // Create an inactive tier
    pricing_tiers::ActiveModel {
        service_id: Set(svc.id),
        name: Set("Inactive".to_string()),
        slug: Set("inactive-tier".to_string()),
        price_cents: Set(9999),
        billing_period: Set("monthly".to_string()),
        max_targets: Set(5),
        max_scans_per_month: Set(10),
        features: Set("[]".to_string()),
        is_active: Set(false),
        sort_order: Set(1),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let active = pricing_tiers::Model::find_active_by_service(db, svc.id).await;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].slug, "active-tier");
}

#[tokio::test]
#[serial]
async fn test_free_tier_with_zero_price() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let svc = create_service(db, "free-tier-svc").await;
    let tier = create_tier(db, svc.id, "free", 0, 0).await;
    assert_eq!(tier.price_cents, 0);
}
