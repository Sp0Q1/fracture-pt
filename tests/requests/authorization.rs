//! Authorization and security tests at the model layer.
//!
//! These tests verify that:
//! - Org-scoped queries never leak cross-org data
//! - Pentester assignment checks are enforced
//! - Platform admin checks work correctly
//! - IDOR prevention via pid+org scoping works
//!
//! Note: HTTP-level authorization tests (cookie-based auth, OIDC sessions)
//! require a running OIDC provider and are tested via `dev/ci.sh`.
//! These tests cover the data-layer security guarantees.

use fracture_pt::app::App;
use fracture_pt::models::{
    engagements, findings, organizations, pentester_assignments, pricing_tiers, reports,
    scan_targets, services, subscriptions,
    users::{self, OidcUserInfo},
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serial_test::serial;

async fn create_user(db: &sea_orm::DatabaseConnection, suffix: &str) -> users::Model {
    users::Model::find_or_create_from_oidc(
        db,
        &OidcUserInfo {
            provider: "test".to_string(),
            subject: format!("test-auth-{suffix}"),
            email: format!("auth-{suffix}@example.com"),
            name: Some(format!("Auth User {suffix}")),
        },
    )
    .await
    .expect("Failed to create test user")
}

async fn create_service(db: &sea_orm::DatabaseConnection, suffix: &str) -> services::Model {
    services::ActiveModel {
        name: Set(format!("Svc {suffix}")),
        slug: Set(format!("auth-svc-{suffix}")),
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

/// Two orgs, each with an engagement, findings, reports, and targets.
/// Returns (org_a_id, org_b_id, eng_a, eng_b, finding_a, report_a, target_a).
async fn setup_two_orgs(
    db: &sea_orm::DatabaseConnection,
) -> (
    i32,
    i32,
    engagements::Model,
    engagements::Model,
    findings::Model,
    reports::Model,
    scan_targets::Model,
) {
    let alice = create_user(db, "alice").await;
    let bob = create_user(db, "bob").await;
    let alice_orgs = organizations::Model::find_orgs_for_user(db, alice.id).await;
    let bob_orgs = organizations::Model::find_orgs_for_user(db, bob.id).await;
    let svc = create_service(db, "iso").await;

    let eng_a = engagements::ActiveModel {
        org_id: Set(alice_orgs[0].id),
        service_id: Set(svc.id),
        title: Set("Alice Eng".to_string()),
        status: Set("in_progress".to_string()),
        target_systems: Set("alice.example.com".to_string()),
        contact_name: Set("Alice".to_string()),
        contact_email: Set("alice@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let eng_b = engagements::ActiveModel {
        org_id: Set(bob_orgs[0].id),
        service_id: Set(svc.id),
        title: Set("Bob Eng".to_string()),
        status: Set("in_progress".to_string()),
        target_systems: Set("bob.example.com".to_string()),
        contact_name: Set("Bob".to_string()),
        contact_email: Set("bob@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let finding_a = findings::ActiveModel {
        org_id: Set(alice_orgs[0].id),
        engagement_id: Set(Some(eng_a.id)),
        created_by_user_id: Set(Some(alice.id)),
        title: Set("Alice Finding".to_string()),
        description: Set("Alice's finding".to_string()),
        severity: Set("extreme".to_string()),
        category: Set("injection".to_string()),
        status: Set("open".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let report_a = reports::ActiveModel {
        org_id: Set(alice_orgs[0].id),
        engagement_id: Set(Some(eng_a.id)),
        title: Set("Alice Report".to_string()),
        report_type: Set("pentest_report".to_string()),
        format: Set("pdf".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let target_a = scan_targets::ActiveModel {
        org_id: Set(alice_orgs[0].id),
        hostname: Set(Some("alice.example.com".to_string())),
        target_type: Set("domain".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    (
        alice_orgs[0].id,
        bob_orgs[0].id,
        eng_a,
        eng_b,
        finding_a,
        report_a,
        target_a,
    )
}

// ====== Cross-Org Isolation Tests ======

#[tokio::test]
#[serial]
async fn test_org_a_cannot_list_org_b_engagements() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (org_a, org_b, _, _, _, _, _) = setup_two_orgs(db).await;

    let a_list = engagements::Model::find_by_org(db, org_a).await;
    let b_list = engagements::Model::find_by_org(db, org_b).await;

    assert_eq!(a_list.len(), 1);
    assert_eq!(a_list[0].title, "Alice Eng");
    assert_eq!(b_list.len(), 1);
    assert_eq!(b_list[0].title, "Bob Eng");
}

#[tokio::test]
#[serial]
async fn test_org_a_cannot_list_org_b_findings() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (org_a, org_b, _, _, _, _, _) = setup_two_orgs(db).await;

    let a_findings = findings::Model::find_by_org(db, org_a).await;
    let b_findings = findings::Model::find_by_org(db, org_b).await;

    assert_eq!(a_findings.len(), 1);
    assert!(b_findings.is_empty());
}

#[tokio::test]
#[serial]
async fn test_org_a_cannot_list_org_b_reports() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (org_a, org_b, _, _, _, _, _) = setup_two_orgs(db).await;

    let a_reports = reports::Model::find_by_org(db, org_a).await;
    let b_reports = reports::Model::find_by_org(db, org_b).await;

    assert_eq!(a_reports.len(), 1);
    assert!(b_reports.is_empty());
}

#[tokio::test]
#[serial]
async fn test_org_a_cannot_list_org_b_targets() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (org_a, org_b, _, _, _, _, _) = setup_two_orgs(db).await;

    let a_targets = scan_targets::Model::find_by_org(db, org_a).await;
    let b_targets = scan_targets::Model::find_by_org(db, org_b).await;

    assert_eq!(a_targets.len(), 1);
    assert!(b_targets.is_empty());
}

// ====== IDOR Prevention Tests ======

#[tokio::test]
#[serial]
async fn test_idor_engagement_by_pid() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (_, org_b, eng_a, _, _, _, _) = setup_two_orgs(db).await;

    // Valid pid but wrong org -> None (not found, not leaked)
    let result = engagements::Model::find_by_pid_and_org(db, &eng_a.pid.to_string(), org_b).await;
    assert!(
        result.is_none(),
        "IDOR: Eng should not be visible to wrong org"
    );
}

#[tokio::test]
#[serial]
async fn test_idor_finding_by_pid() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (_, org_b, _, _, finding_a, _, _) = setup_two_orgs(db).await;

    let result = findings::Model::find_by_pid_and_org(db, &finding_a.pid.to_string(), org_b).await;
    assert!(
        result.is_none(),
        "IDOR: Finding should not be visible to wrong org"
    );
}

#[tokio::test]
#[serial]
async fn test_idor_report_by_pid() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (_, org_b, _, _, _, report_a, _) = setup_two_orgs(db).await;

    let result = reports::Model::find_by_pid_and_org(db, &report_a.pid.to_string(), org_b).await;
    assert!(
        result.is_none(),
        "IDOR: Report should not be visible to wrong org"
    );
}

#[tokio::test]
#[serial]
async fn test_idor_target_by_pid() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let (_, org_b, _, _, _, _, target_a) = setup_two_orgs(db).await;

    let result =
        scan_targets::Model::find_by_pid_and_org(db, &target_a.pid.to_string(), org_b).await;
    assert!(
        result.is_none(),
        "IDOR: Target should not be visible to wrong org"
    );
}

#[tokio::test]
#[serial]
async fn test_idor_uuid_guessing_returns_none() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_user(db, "uuid-guess").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let org_id = orgs[0].id;

    // Random valid UUID that doesn't exist
    let fake_pid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    assert!(
        engagements::Model::find_by_pid_and_org(db, fake_pid, org_id)
            .await
            .is_none()
    );
    assert!(findings::Model::find_by_pid_and_org(db, fake_pid, org_id)
        .await
        .is_none());
    assert!(reports::Model::find_by_pid_and_org(db, fake_pid, org_id)
        .await
        .is_none());
    assert!(
        scan_targets::Model::find_by_pid_and_org(db, fake_pid, org_id)
            .await
            .is_none()
    );
}

#[tokio::test]
#[serial]
async fn test_idor_invalid_uuid_returns_none() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_user(db, "bad-uuid").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let org_id = orgs[0].id;

    // Invalid UUID string
    assert!(
        engagements::Model::find_by_pid_and_org(db, "not-a-uuid", org_id)
            .await
            .is_none()
    );
    assert!(
        findings::Model::find_by_pid_and_org(db, "'; DROP TABLE--", org_id)
            .await
            .is_none()
    );
    assert!(
        reports::Model::find_by_pid_and_org(db, "../../../etc/passwd", org_id)
            .await
            .is_none()
    );
}

// ====== Pentester Isolation Tests ======

#[tokio::test]
#[serial]
async fn test_pentester_sees_only_assigned_engagements() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let admin = create_user(db, "pt-admin").await;
    let pentester = create_user(db, "pt-tester").await;

    // Create two engagements in different orgs
    let user1 = create_user(db, "pt-org1").await;
    let user2 = create_user(db, "pt-org2").await;
    let orgs1 = organizations::Model::find_orgs_for_user(db, user1.id).await;
    let orgs2 = organizations::Model::find_orgs_for_user(db, user2.id).await;
    let svc = create_service(db, "pt-iso").await;

    let eng1 = engagements::ActiveModel {
        org_id: Set(orgs1[0].id),
        service_id: Set(svc.id),
        title: Set("Assigned Eng".to_string()),
        status: Set("in_progress".to_string()),
        target_systems: Set("test1.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let eng2 = engagements::ActiveModel {
        org_id: Set(orgs2[0].id),
        service_id: Set(svc.id),
        title: Set("Unassigned Eng".to_string()),
        status: Set("in_progress".to_string()),
        target_systems: Set("test2.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    // Assign pentester to eng1 only
    pentester_assignments::ActiveModel {
        engagement_id: Set(eng1.id),
        user_id: Set(pentester.id),
        assigned_by_user_id: Set(Some(admin.id)),
        role: Set("member".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    // Pentester sees only assigned engagement
    let visible = engagements::Model::find_by_pentester(db, pentester.id).await;
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, eng1.id);

    // Assignment checks
    assert!(pentester_assignments::Model::is_assigned(db, pentester.id, eng1.id).await);
    assert!(!pentester_assignments::Model::is_assigned(db, pentester.id, eng2.id).await);
}

#[tokio::test]
#[serial]
async fn test_pentester_finding_scoped_to_engagement() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = create_user(db, "pt-finding").await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let svc = create_service(db, "pt-finding-svc").await;

    let eng1 = engagements::ActiveModel {
        org_id: Set(orgs[0].id),
        service_id: Set(svc.id),
        title: Set("Eng 1".to_string()),
        status: Set("in_progress".to_string()),
        target_systems: Set("test.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let eng2 = engagements::ActiveModel {
        org_id: Set(orgs[0].id),
        service_id: Set(svc.id),
        title: Set("Eng 2".to_string()),
        status: Set("in_progress".to_string()),
        target_systems: Set("other.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    // Create finding in eng1
    let finding = findings::ActiveModel {
        org_id: Set(orgs[0].id),
        engagement_id: Set(Some(eng1.id)),
        created_by_user_id: Set(Some(user.id)),
        title: Set("Eng1 Finding".to_string()),
        description: Set("Found in eng1".to_string()),
        severity: Set("high".to_string()),
        category: Set("xss".to_string()),
        status: Set("open".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    // Finding visible via eng1 scope
    let found =
        findings::Model::find_by_pid_and_engagement(db, &finding.pid.to_string(), eng1.id).await;
    assert!(found.is_some());

    // Finding NOT visible via eng2 scope (pentester of eng2 cannot see eng1's findings)
    let not_found =
        findings::Model::find_by_pid_and_engagement(db, &finding.pid.to_string(), eng2.id).await;
    assert!(not_found.is_none());
}

// ====== Subscription Isolation ======

#[tokio::test]
#[serial]
async fn test_subscription_cross_org_isolation() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let alice = create_user(db, "sub-alice").await;
    let bob = create_user(db, "sub-bob").await;
    let alice_orgs = organizations::Model::find_orgs_for_user(db, alice.id).await;
    let bob_orgs = organizations::Model::find_orgs_for_user(db, bob.id).await;

    let svc = create_service(db, "sub-iso").await;
    let tier = pricing_tiers::ActiveModel {
        service_id: Set(svc.id),
        name: Set("Pro".to_string()),
        slug: Set("pro-sub-iso".to_string()),
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

    // Alice subscribes
    let alice_sub = subscriptions::ActiveModel {
        org_id: Set(alice_orgs[0].id),
        tier_id: Set(tier.id),
        status: Set("active".to_string()),
        starts_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    // Bob has no subscription
    let bob_subs = subscriptions::Model::find_by_org(db, bob_orgs[0].id).await;
    assert!(bob_subs.is_empty());

    // Cross-org pid lookup fails
    let cross =
        subscriptions::Model::find_by_pid_and_org(db, &alice_sub.pid.to_string(), bob_orgs[0].id)
            .await;
    assert!(cross.is_none());
}

// ====== Admin Cross-Org Access Tests ======

#[tokio::test]
#[serial]
async fn test_admin_find_by_pid_sees_all_engagements() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user1 = create_user(db, "admin-all-1").await;
    let user2 = create_user(db, "admin-all-2").await;
    let orgs1 = organizations::Model::find_orgs_for_user(db, user1.id).await;
    let orgs2 = organizations::Model::find_orgs_for_user(db, user2.id).await;
    let svc = create_service(db, "admin-all").await;

    let eng1 = engagements::ActiveModel {
        org_id: Set(orgs1[0].id),
        service_id: Set(svc.id),
        title: Set("Org1 Eng".to_string()),
        status: Set("requested".to_string()),
        target_systems: Set("test.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let eng2 = engagements::ActiveModel {
        org_id: Set(orgs2[0].id),
        service_id: Set(svc.id),
        title: Set("Org2 Eng".to_string()),
        status: Set("requested".to_string()),
        target_systems: Set("other.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    // Admin's find_by_pid (no org scope) sees both
    let found1 = engagements::Model::find_by_pid(db, &eng1.pid.to_string()).await;
    assert!(found1.is_some());
    assert_eq!(found1.unwrap().title, "Org1 Eng");

    let found2 = engagements::Model::find_by_pid(db, &eng2.pid.to_string()).await;
    assert!(found2.is_some());
    assert_eq!(found2.unwrap().title, "Org2 Eng");

    // find_all_pending sees both
    let pending = engagements::Model::find_all_pending(db).await;
    assert!(pending.len() >= 2);
}
