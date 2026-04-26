//! Tests for the tiered scan authorization gate.
//!
//! These tests are the contract that every per-tool integration relies on.
//! If any of them regress, "active scans against unverified targets" or
//! similar safety rules break.

use chrono::{Duration, Utc};
use fracture_pt::app::App;
use fracture_pt::models::{
    engagement_targets, engagements, organizations, scan_targets, services,
    users::{self, OidcUserInfo},
};
use fracture_pt::services::scan_authz::{
    evaluate_scan_auth, DenialReason, ScanCaller, ScanMode, UnlockReason,
};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serial_test::serial;

async fn user(db: &sea_orm::DatabaseConnection, suffix: &str) -> users::Model {
    users::Model::find_or_create_from_oidc(
        db,
        &OidcUserInfo {
            provider: "test".to_string(),
            subject: format!("test-az-{suffix}"),
            email: format!("az-{suffix}@example.com"),
            name: Some(format!("AZ User {suffix}")),
        },
    )
    .await
    .expect("create user")
}

async fn org(db: &sea_orm::DatabaseConnection, user_id: i32) -> organizations::Model {
    organizations::Model::find_orgs_for_user(db, user_id)
        .await
        .expect("orgs for user")
        .into_iter()
        .next()
        .expect("personal org")
}

async fn target(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    hostname: &str,
    verified: bool,
) -> scan_targets::Model {
    let mut am = scan_targets::ActiveModel {
        org_id: Set(org_id),
        hostname: Set(Some(hostname.to_string())),
        target_type: Set("domain".to_string()),
        ..Default::default()
    };
    if verified {
        am.verified_at = Set(Some(Utc::now().into()));
        am.verification_method = Set(Some("dns_txt".to_string()));
    }
    am.insert(db).await.expect("create scan target")
}

async fn service(db: &sea_orm::DatabaseConnection, slug: &str) -> services::Model {
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
    .expect("create service")
}

async fn engagement(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    service_id: i32,
    status: &str,
    test_window: Option<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)>,
) -> engagements::Model {
    let mut am = engagements::ActiveModel {
        org_id: Set(org_id),
        service_id: Set(service_id),
        title: Set("Test eng".to_string()),
        status: Set(status.to_string()),
        target_systems: Set("api.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("c@e.com".to_string()),
        requested_at: Set(Utc::now().into()),
        ..Default::default()
    };
    if let Some((s, e)) = test_window {
        am.test_window_start = Set(Some(s.into()));
        am.test_window_end = Set(Some(e.into()));
    }
    am.insert(db).await.expect("create engagement")
}

async fn link_target(
    db: &sea_orm::DatabaseConnection,
    engagement_id: i32,
    scan_target_id: i32,
) -> engagement_targets::Model {
    engagement_targets::ActiveModel {
        engagement_id: Set(engagement_id),
        scan_target_id: Set(scan_target_id),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("link target")
}

const MEMBER: ScanCaller = ScanCaller {
    has_member_role: true,
    is_platform_admin: false,
};

const ADMIN: ScanCaller = ScanCaller {
    has_member_role: true,
    is_platform_admin: true,
};

const NO_ROLE: ScanCaller = ScanCaller {
    has_member_role: false,
    is_platform_admin: false,
};

#[tokio::test]
#[serial]
async fn no_role_denies_all() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "no-role").await;
    let o = org(db, u.id).await;
    let t = target(db, o.id, "no-role.example.com", false).await;

    let auth = evaluate_scan_auth(db, &t, NO_ROLE).await.unwrap();

    assert!(!auth.allows(ScanMode::Passive));
    assert!(!auth.allows(ScanMode::Active));
    assert_eq!(auth.active_denial, Some(DenialReason::InsufficientRole));
}

#[tokio::test]
#[serial]
async fn member_with_unverified_target_no_engagement_passive_only() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "passive").await;
    let o = org(db, u.id).await;
    let t = target(db, o.id, "passive.example.com", false).await;

    let auth = evaluate_scan_auth(db, &t, MEMBER).await.unwrap();

    assert!(auth.allows(ScanMode::Passive));
    assert!(!auth.allows(ScanMode::Active));
    assert_eq!(
        auth.active_denial,
        Some(DenialReason::NotVerifiedNoEngagement)
    );
    assert!(auth.unlock_reasons.is_empty());
}

#[tokio::test]
#[serial]
async fn verified_target_unlocks_active() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "verified").await;
    let o = org(db, u.id).await;
    let t = target(db, o.id, "verified.example.com", true).await;

    let auth = evaluate_scan_auth(db, &t, MEMBER).await.unwrap();

    assert!(auth.allows(ScanMode::Active));
    assert!(auth.unlock_reasons.contains(&UnlockReason::Verification));
    assert!(auth.active_denial.is_none());
}

#[tokio::test]
#[serial]
async fn signed_engagement_in_window_unlocks_active() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "engaged").await;
    let o = org(db, u.id).await;
    let svc = service(db, "engaged-svc").await;
    let t = target(db, o.id, "engaged.example.com", false).await;

    let now = Utc::now();
    let eng = engagement(
        db,
        o.id,
        svc.id,
        "accepted",
        Some((now - Duration::days(1), now + Duration::days(7))),
    )
    .await;
    link_target(db, eng.id, t.id).await;

    let auth = evaluate_scan_auth(db, &t, MEMBER).await.unwrap();

    assert!(auth.allows(ScanMode::Active));
    assert!(auth
        .unlock_reasons
        .contains(&UnlockReason::SignedEngagement));
}

#[tokio::test]
#[serial]
async fn pending_engagement_does_not_unlock() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "pending").await;
    let o = org(db, u.id).await;
    let svc = service(db, "pending-svc").await;
    let t = target(db, o.id, "pending.example.com", false).await;

    let now = Utc::now();
    let eng = engagement(
        db,
        o.id,
        svc.id,
        "requested",
        Some((now - Duration::days(1), now + Duration::days(7))),
    )
    .await;
    link_target(db, eng.id, t.id).await;

    let auth = evaluate_scan_auth(db, &t, MEMBER).await.unwrap();

    assert!(!auth.allows(ScanMode::Active));
}

#[tokio::test]
#[serial]
async fn expired_engagement_window_does_not_unlock() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "expired").await;
    let o = org(db, u.id).await;
    let svc = service(db, "expired-svc").await;
    let t = target(db, o.id, "expired.example.com", false).await;

    let now = Utc::now();
    let eng = engagement(
        db,
        o.id,
        svc.id,
        "accepted",
        Some((now - Duration::days(30), now - Duration::days(1))),
    )
    .await;
    link_target(db, eng.id, t.id).await;

    let auth = evaluate_scan_auth(db, &t, MEMBER).await.unwrap();

    assert!(!auth.allows(ScanMode::Active));
}

#[tokio::test]
#[serial]
async fn future_engagement_window_does_not_unlock_yet() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "future").await;
    let o = org(db, u.id).await;
    let svc = service(db, "future-svc").await;
    let t = target(db, o.id, "future.example.com", false).await;

    let now = Utc::now();
    let eng = engagement(
        db,
        o.id,
        svc.id,
        "accepted",
        Some((now + Duration::days(7), now + Duration::days(14))),
    )
    .await;
    link_target(db, eng.id, t.id).await;

    let auth = evaluate_scan_auth(db, &t, MEMBER).await.unwrap();

    assert!(!auth.allows(ScanMode::Active));
}

#[tokio::test]
#[serial]
async fn platform_admin_override_unlocks_active() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "admin-override").await;
    let o = org(db, u.id).await;
    let t = target(db, o.id, "override.example.com", false).await;

    let auth = evaluate_scan_auth(db, &t, ADMIN).await.unwrap();

    assert!(auth.allows(ScanMode::Active));
    assert!(auth
        .unlock_reasons
        .contains(&UnlockReason::PlatformAdminOverride));
}

#[tokio::test]
#[serial]
async fn multiple_unlocks_are_recorded_together() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let u = user(db, "multi").await;
    let o = org(db, u.id).await;
    let svc = service(db, "multi-svc").await;
    // Verified AND in a signed engagement
    let t = target(db, o.id, "multi.example.com", true).await;

    let now = Utc::now();
    let eng = engagement(
        db,
        o.id,
        svc.id,
        "in_progress",
        Some((now - Duration::days(1), now + Duration::days(7))),
    )
    .await;
    link_target(db, eng.id, t.id).await;

    let auth = evaluate_scan_auth(db, &t, ADMIN).await.unwrap();

    assert!(auth.allows(ScanMode::Active));
    assert!(auth.unlock_reasons.contains(&UnlockReason::Verification));
    assert!(auth
        .unlock_reasons
        .contains(&UnlockReason::SignedEngagement));
    assert!(auth
        .unlock_reasons
        .contains(&UnlockReason::PlatformAdminOverride));
}
