use gethacked::app::App;
use gethacked::models::{
    organizations, pricing_tiers, services, subscriptions,
    users::{self, OidcUserInfo},
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serial_test::serial;

async fn create_test_user(db: &sea_orm::DatabaseConnection, suffix: &str) -> users::Model {
    users::Model::find_or_create_from_oidc(
        db,
        &OidcUserInfo {
            provider: "test".to_string(),
            subject: format!("test-sub-{suffix}"),
            email: format!("sub-{suffix}@example.com"),
            name: Some(format!("Sub User {suffix}")),
        },
    )
    .await
    .expect("Failed to create test user")
}

async fn setup_service_and_tier(
    db: &sea_orm::DatabaseConnection,
    suffix: &str,
) -> (services::Model, pricing_tiers::Model) {
    let svc = services::ActiveModel {
        name: Set(format!("Svc {suffix}")),
        slug: Set(format!("svc-{suffix}")),
        category: Set("pentest".to_string()),
        description: Set("Test".to_string()),
        is_automated: Set(false),
        is_active: Set(true),
        sort_order: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let tier = pricing_tiers::ActiveModel {
        service_id: Set(svc.id),
        name: Set("Pro".to_string()),
        slug: Set(format!("pro-{suffix}")),
        price_cents: Set(9999),
        billing_period: Set("monthly".to_string()),
        max_targets: Set(10),
        max_scans_per_month: Set(50),
        features: Set("[]".to_string()),
        is_active: Set(true),
        sort_order: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    (svc, tier)
}

#[tokio::test]
#[serial]
async fn test_create_subscription() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "createsub").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let org = &orgs[0];
    let (_svc, tier) = setup_service_and_tier(db, "createsub").await;

    let sub = subscriptions::ActiveModel {
        org_id: Set(org.id),
        tier_id: Set(tier.id),
        status: Set("active".to_string()),
        starts_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    assert_eq!(sub.org_id, org.id);
    assert_eq!(sub.tier_id, tier.id);
    assert_eq!(sub.status, "active");
}

#[tokio::test]
#[serial]
async fn test_subscription_sets_pid() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "subpid").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let (_svc, tier) = setup_service_and_tier(db, "subpid").await;

    let sub = subscriptions::ActiveModel {
        org_id: Set(orgs[0].id),
        tier_id: Set(tier.id),
        status: Set("active".to_string()),
        starts_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    assert!(!sub.pid.is_nil());
}

#[tokio::test]
#[serial]
async fn test_find_active_subscription_for_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "findactive").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let org = &orgs[0];
    let (_svc, tier) = setup_service_and_tier(db, "findactive").await;

    // No subscription yet
    let active = subscriptions::Model::find_active_by_org(db, org.id).await;
    assert!(active.is_empty());

    // Create one
    subscriptions::ActiveModel {
        org_id: Set(org.id),
        tier_id: Set(tier.id),
        status: Set("active".to_string()),
        starts_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let active = subscriptions::Model::find_active_by_org(db, org.id).await;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].status, "active");
}

#[tokio::test]
#[serial]
async fn test_find_subscription_by_pid_and_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "bypidorg").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let org = &orgs[0];
    let (_svc, tier) = setup_service_and_tier(db, "bypidorg").await;

    let sub = subscriptions::ActiveModel {
        org_id: Set(org.id),
        tier_id: Set(tier.id),
        status: Set("active".to_string()),
        starts_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let found = subscriptions::Model::find_by_pid_and_org(db, &sub.pid.to_string(), org.id).await;
    assert!(found.is_some());

    // Wrong org
    let not_found =
        subscriptions::Model::find_by_pid_and_org(db, &sub.pid.to_string(), org.id + 999).await;
    assert!(not_found.is_none());
}

#[tokio::test]
#[serial]
async fn test_subscription_cross_org_isolation() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let alice = create_test_user(db, "sub-iso-alice").await;
    let bob = create_test_user(db, "sub-iso-bob").await;
    let alice_orgs = organizations::Model::find_orgs_for_user(db, alice.id).await;
    let bob_orgs = organizations::Model::find_orgs_for_user(db, bob.id).await;
    let (_svc, tier) = setup_service_and_tier(db, "sub-iso").await;

    subscriptions::ActiveModel {
        org_id: Set(alice_orgs[0].id),
        tier_id: Set(tier.id),
        status: Set("active".to_string()),
        starts_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    // Bob's org should have no subscriptions
    let bob_subs = subscriptions::Model::find_by_org(db, bob_orgs[0].id).await;
    assert!(bob_subs.is_empty());

    // Alice's org should have one
    let alice_subs = subscriptions::Model::find_by_org(db, alice_orgs[0].id).await;
    assert_eq!(alice_subs.len(), 1);
}
