use gethacked::app::App;
use gethacked::models::{
    engagement_offers, engagements, organizations, services,
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
            subject: format!("test-offer-{suffix}"),
            email: format!("offer-{suffix}@example.com"),
            name: Some(format!("Offer User {suffix}")),
        },
    )
    .await
    .expect("Failed to create test user")
}

async fn setup_engagement(
    db: &sea_orm::DatabaseConnection,
    suffix: &str,
) -> (users::Model, engagements::Model) {
    let user = create_test_user(db, suffix).await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = services::ActiveModel {
        name: Set(format!("Svc {suffix}")),
        slug: Set(format!("offer-svc-{suffix}")),
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

    let eng = engagements::ActiveModel {
        org_id: Set(orgs[0].id),
        service_id: Set(svc.id),
        title: Set(format!("Engagement {suffix}")),
        status: Set("requested".to_string()),
        target_systems: Set("api.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    (user, eng)
}

#[tokio::test]
#[serial]
async fn test_create_offer() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, eng) = setup_engagement(db, "create").await;

    let offer = engagement_offers::ActiveModel {
        engagement_id: Set(eng.id),
        created_by_user_id: Set(Some(user.id)),
        amount_cents: Set(50000),
        currency: Set("EUR".to_string()),
        timeline_days: Set(14),
        deliverables: Set("Full pentest report".to_string()),
        terms: Set(Some("Standard T&C".to_string())),
        valid_until: Set((chrono::Utc::now() + chrono::Duration::days(30)).into()),
        status: Set("pending".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    assert_eq!(offer.amount_cents, 50000);
    assert_eq!(offer.currency, "EUR");
    assert_eq!(offer.timeline_days, 14);
    assert_eq!(offer.status, "pending");
    assert!(!offer.pid.is_nil());
}

#[tokio::test]
#[serial]
async fn test_find_offers_by_engagement() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, eng) = setup_engagement(db, "findoffers").await;

    engagement_offers::ActiveModel {
        engagement_id: Set(eng.id),
        created_by_user_id: Set(Some(user.id)),
        amount_cents: Set(30000),
        currency: Set("EUR".to_string()),
        timeline_days: Set(7),
        deliverables: Set("Quick scan".to_string()),
        valid_until: Set((chrono::Utc::now() + chrono::Duration::days(14)).into()),
        status: Set("superseded".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    engagement_offers::ActiveModel {
        engagement_id: Set(eng.id),
        created_by_user_id: Set(Some(user.id)),
        amount_cents: Set(50000),
        currency: Set("EUR".to_string()),
        timeline_days: Set(14),
        deliverables: Set("Full report".to_string()),
        valid_until: Set((chrono::Utc::now() + chrono::Duration::days(30)).into()),
        status: Set("pending".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let offers = engagement_offers::Model::find_by_engagement(db, eng.id).await;
    assert_eq!(offers.len(), 2);
}

#[tokio::test]
#[serial]
async fn test_find_latest_offer_by_engagement() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, eng) = setup_engagement(db, "latest").await;

    engagement_offers::ActiveModel {
        engagement_id: Set(eng.id),
        created_by_user_id: Set(Some(user.id)),
        amount_cents: Set(30000),
        currency: Set("EUR".to_string()),
        timeline_days: Set(7),
        deliverables: Set("Old offer".to_string()),
        valid_until: Set((chrono::Utc::now() + chrono::Duration::days(14)).into()),
        status: Set("superseded".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    engagement_offers::ActiveModel {
        engagement_id: Set(eng.id),
        created_by_user_id: Set(Some(user.id)),
        amount_cents: Set(50000),
        currency: Set("EUR".to_string()),
        timeline_days: Set(14),
        deliverables: Set("Latest offer".to_string()),
        valid_until: Set((chrono::Utc::now() + chrono::Duration::days(30)).into()),
        status: Set("pending".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let latest = engagement_offers::Model::find_latest_by_engagement(db, eng.id).await;
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().deliverables, "Latest offer");
}

#[tokio::test]
#[serial]
async fn test_no_offers_returns_none() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (_user, eng) = setup_engagement(db, "nooffer").await;

    let latest = engagement_offers::Model::find_latest_by_engagement(db, eng.id).await;
    assert!(latest.is_none());

    let all = engagement_offers::Model::find_by_engagement(db, eng.id).await;
    assert!(all.is_empty());
}
