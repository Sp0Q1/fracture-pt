use gethacked::app::App;
use gethacked::models::{
    engagements, organizations, services,
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
            subject: format!("test-eng-{suffix}"),
            email: format!("eng-{suffix}@example.com"),
            name: Some(format!("Eng User {suffix}")),
        },
    )
    .await
    .expect("Failed to create test user")
}

async fn create_service(db: &sea_orm::DatabaseConnection, slug: &str) -> services::Model {
    services::ActiveModel {
        name: Set(format!("Svc {slug}")),
        slug: Set(slug.to_string()),
        category: Set("pentest".to_string()),
        description: Set("Test".to_string()),
        is_automated: Set(false),
        is_active: Set(true),
        sort_order: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

async fn create_engagement(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    service_id: i32,
    title: &str,
    status: &str,
) -> engagements::Model {
    engagements::ActiveModel {
        org_id: Set(org_id),
        service_id: Set(service_id),
        title: Set(title.to_string()),
        status: Set(status.to_string()),
        target_systems: Set("api.example.com".to_string()),
        contact_name: Set("Test Contact".to_string()),
        contact_email: Set("contact@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("Failed to create engagement")
}

#[tokio::test]
#[serial]
async fn test_create_engagement() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "createeng").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "eng-svc").await;
    let eng = create_engagement(db, orgs[0].id, svc.id, "Test Pentest", "requested").await;

    assert_eq!(eng.title, "Test Pentest");
    assert_eq!(eng.status, "requested");
    assert_eq!(eng.org_id, orgs[0].id);
    assert_eq!(eng.service_id, svc.id);
}

#[tokio::test]
#[serial]
async fn test_engagement_sets_pid_on_insert() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "engpid").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "engpid-svc").await;
    let eng = create_engagement(db, orgs[0].id, svc.id, "PID Test", "requested").await;
    assert!(!eng.pid.is_nil());
}

#[tokio::test]
#[serial]
async fn test_find_engagements_by_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "engbyorg").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "engbyorg-svc").await;

    create_engagement(db, orgs[0].id, svc.id, "Eng 1", "requested").await;
    create_engagement(db, orgs[0].id, svc.id, "Eng 2", "in_progress").await;

    let items = engagements::Model::find_by_org(db, orgs[0].id).await;
    assert_eq!(items.len(), 2);
    // Ordered by id DESC
    assert_eq!(items[0].title, "Eng 2");
    assert_eq!(items[1].title, "Eng 1");
}

#[tokio::test]
#[serial]
async fn test_find_engagement_by_pid_and_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "engpidorg").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "engpidorg-svc").await;
    let eng = create_engagement(db, orgs[0].id, svc.id, "Find Me", "requested").await;

    let found = engagements::Model::find_by_pid_and_org(db, &eng.pid.to_string(), orgs[0].id).await;
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Find Me");

    // Wrong org
    let not_found =
        engagements::Model::find_by_pid_and_org(db, &eng.pid.to_string(), orgs[0].id + 999).await;
    assert!(not_found.is_none());
}

#[tokio::test]
#[serial]
async fn test_engagement_cross_org_isolation() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let alice = create_test_user(db, "eng-iso-alice").await;
    let bob = create_test_user(db, "eng-iso-bob").await;
    let alice_orgs = organizations::Model::find_orgs_for_user(db, alice.id).await;
    let bob_orgs = organizations::Model::find_orgs_for_user(db, bob.id).await;
    let svc = create_service(db, "eng-iso-svc").await;

    let alice_eng = create_engagement(db, alice_orgs[0].id, svc.id, "Alice Eng", "requested").await;
    create_engagement(db, bob_orgs[0].id, svc.id, "Bob Eng", "requested").await;

    // Alice's org should only see Alice's engagement
    let alice_items = engagements::Model::find_by_org(db, alice_orgs[0].id).await;
    assert_eq!(alice_items.len(), 1);
    assert_eq!(alice_items[0].title, "Alice Eng");

    // Cross-org pid lookup should fail
    let cross_org =
        engagements::Model::find_by_pid_and_org(db, &alice_eng.pid.to_string(), bob_orgs[0].id)
            .await;
    assert!(cross_org.is_none());
}

#[tokio::test]
#[serial]
async fn test_find_all_pending_engagements() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "pending").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "pending-svc").await;

    create_engagement(db, orgs[0].id, svc.id, "Pending", "requested").await;
    create_engagement(db, orgs[0].id, svc.id, "Active", "in_progress").await;

    let pending = engagements::Model::find_all_pending(db).await;
    assert!(pending.iter().all(|e| e.status == "requested"));
    assert!(pending.iter().any(|e| e.title == "Pending"));
    assert!(!pending.iter().any(|e| e.title == "Active"));
}

#[tokio::test]
#[serial]
async fn test_find_engagements_by_status() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "bystatus").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "bystatus-svc").await;

    create_engagement(db, orgs[0].id, svc.id, "Req1", "requested").await;
    create_engagement(db, orgs[0].id, svc.id, "InProg1", "in_progress").await;

    let requested = engagements::Model::find_by_status(db, "requested").await;
    assert!(requested.iter().all(|e| e.status == "requested"));

    let in_progress = engagements::Model::find_by_status(db, "in_progress").await;
    assert!(in_progress.iter().all(|e| e.status == "in_progress"));
}

#[tokio::test]
#[serial]
async fn test_engagement_scope_fields() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_test_user(db, "scope").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "scope-svc").await;

    let eng = engagements::ActiveModel {
        org_id: Set(orgs[0].id),
        service_id: Set(svc.id),
        title: Set("Full Scope".to_string()),
        status: Set("requested".to_string()),
        target_systems: Set("api.example.com, web.example.com".to_string()),
        ip_ranges: Set(Some("203.0.113.0/24".to_string())),
        domains: Set(Some("example.com".to_string())),
        exclusions: Set(Some("billing.example.com".to_string())),
        contact_name: Set("Jane CTO".to_string()),
        contact_email: Set("jane@example.com".to_string()),
        contact_phone: Set(Some("+31612345678".to_string())),
        rules_of_engagement: Set(Some("No DoS testing".to_string())),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    assert_eq!(eng.ip_ranges, Some("203.0.113.0/24".to_string()));
    assert_eq!(eng.exclusions, Some("billing.example.com".to_string()));
    assert_eq!(eng.contact_name, "Jane CTO");
    assert_eq!(eng.contact_phone, Some("+31612345678".to_string()));
    assert_eq!(eng.rules_of_engagement, Some("No DoS testing".to_string()));
}
