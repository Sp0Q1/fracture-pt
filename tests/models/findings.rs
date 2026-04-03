use fracture_pt::app::App;
use fracture_pt::models::{
    engagements, findings, organizations, services,
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
            subject: format!("test-finding-{suffix}"),
            email: format!("finding-{suffix}@example.com"),
            name: Some(format!("Finding User {suffix}")),
        },
    )
    .await
    .expect("Failed to create test user")
}

async fn setup_engagement(
    db: &sea_orm::DatabaseConnection,
    suffix: &str,
) -> (users::Model, i32, engagements::Model) {
    let user = create_test_user(db, suffix).await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id).await;
    let org_id = orgs[0].id;
    let svc = services::ActiveModel {
        name: Set(format!("Svc {suffix}")),
        slug: Set(format!("finding-svc-{suffix}")),
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
        org_id: Set(org_id),
        service_id: Set(svc.id),
        title: Set(format!("Engagement {suffix}")),
        status: Set("in_progress".to_string()),
        target_systems: Set("api.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    (user, org_id, eng)
}

async fn create_finding(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    engagement_id: i32,
    user_id: i32,
    title: &str,
    severity: &str,
    category: &str,
) -> findings::Model {
    findings::ActiveModel {
        org_id: Set(org_id),
        engagement_id: Set(Some(engagement_id)),
        created_by_user_id: Set(Some(user_id)),
        title: Set(title.to_string()),
        description: Set(format!("Description for {title}")),
        severity: Set(severity.to_string()),
        category: Set(category.to_string()),
        status: Set("open".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("Failed to create finding")
}

#[tokio::test]
#[serial]
async fn test_create_finding() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "create").await;
    let f = create_finding(
        db,
        org_id,
        eng.id,
        user.id,
        "SQL Injection",
        "extreme",
        "injection",
    )
    .await;

    assert_eq!(f.title, "SQL Injection");
    assert_eq!(f.severity, "extreme");
    assert_eq!(f.category, "injection");
    assert_eq!(f.status, "open");
    assert_eq!(f.engagement_id, Some(eng.id));
    assert_eq!(f.created_by_user_id, Some(user.id));
}

#[tokio::test]
#[serial]
async fn test_finding_sets_pid_on_insert() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "pid").await;
    let f = create_finding(db, org_id, eng.id, user.id, "PID Test", "high", "xss").await;
    assert!(!f.pid.is_nil());
}

#[tokio::test]
#[serial]
async fn test_find_findings_by_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "byorg").await;
    create_finding(
        db,
        org_id,
        eng.id,
        user.id,
        "Finding A",
        "extreme",
        "injection",
    )
    .await;
    create_finding(db, org_id, eng.id, user.id, "Finding B", "low", "misconfig").await;

    let items = findings::Model::find_by_org(db, org_id).await;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Finding B");
    assert_eq!(items[1].title, "Finding A");
}

#[tokio::test]
#[serial]
async fn test_find_findings_by_engagement() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "byeng").await;
    create_finding(db, org_id, eng.id, user.id, "Eng Finding", "high", "xss").await;

    let items = findings::Model::find_by_engagement(db, eng.id).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Eng Finding");

    let empty = findings::Model::find_by_engagement(db, eng.id + 999).await;
    assert!(empty.is_empty());
}

#[tokio::test]
#[serial]
async fn test_find_findings_by_severity() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "bysev").await;
    create_finding(
        db,
        org_id,
        eng.id,
        user.id,
        "Extreme 1",
        "extreme",
        "injection",
    )
    .await;
    create_finding(db, org_id, eng.id, user.id, "Low 1", "low", "misconfig").await;
    create_finding(db, org_id, eng.id, user.id, "Extreme 2", "extreme", "xss").await;

    let extremes = findings::Model::find_by_severity(db, org_id, "extreme").await;
    assert_eq!(extremes.len(), 2);
    assert!(extremes.iter().all(|f| f.severity == "extreme"));

    let lows = findings::Model::find_by_severity(db, org_id, "low").await;
    assert_eq!(lows.len(), 1);
}

#[tokio::test]
#[serial]
async fn test_find_finding_by_pid_and_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "bypidorg").await;
    let f = create_finding(db, org_id, eng.id, user.id, "Scoped", "high", "idor").await;

    let found = findings::Model::find_by_pid_and_org(db, &f.pid.to_string(), org_id).await;
    assert!(found.is_some());
    assert_eq!(found.as_ref().unwrap().title, "Scoped");

    let not_found =
        findings::Model::find_by_pid_and_org(db, &f.pid.to_string(), org_id + 999).await;
    assert!(not_found.is_none());
}

#[tokio::test]
#[serial]
async fn test_find_finding_by_pid_and_engagement() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "bypideng").await;
    let f = create_finding(db, org_id, eng.id, user.id, "EngScoped", "elevated", "xss").await;

    let found = findings::Model::find_by_pid_and_engagement(db, &f.pid.to_string(), eng.id).await;
    assert!(found.is_some());
    assert_eq!(found.as_ref().unwrap().title, "EngScoped");

    let not_found =
        findings::Model::find_by_pid_and_engagement(db, &f.pid.to_string(), eng.id + 999).await;
    assert!(not_found.is_none());
}

#[tokio::test]
#[serial]
async fn test_finding_cross_org_isolation() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (alice, alice_org_id, alice_eng) = setup_engagement(db, "iso-alice").await;
    let (bob, bob_org_id, bob_eng) = setup_engagement(db, "iso-bob").await;

    let alice_finding = create_finding(
        db,
        alice_org_id,
        alice_eng.id,
        alice.id,
        "Alice Finding",
        "extreme",
        "injection",
    )
    .await;
    create_finding(
        db,
        bob_org_id,
        bob_eng.id,
        bob.id,
        "Bob Finding",
        "high",
        "xss",
    )
    .await;

    let alice_items = findings::Model::find_by_org(db, alice_org_id).await;
    assert_eq!(alice_items.len(), 1);
    assert_eq!(alice_items[0].title, "Alice Finding");

    let cross =
        findings::Model::find_by_pid_and_org(db, &alice_finding.pid.to_string(), bob_org_id).await;
    assert!(cross.is_none());
}

#[tokio::test]
#[serial]
async fn test_finding_with_all_fields_populated() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "allfields").await;

    let f = findings::ActiveModel {
        org_id: Set(org_id),
        engagement_id: Set(Some(eng.id)),
        created_by_user_id: Set(Some(user.id)),
        title: Set("SQL Injection in Login Form".to_string()),
        description: Set("The login form is vulnerable to SQL injection.".to_string()),
        technical_description: Set(Some(
            "Parameter 'username' passed to SQL without parameterization.".to_string(),
        )),
        impact: Set(Some(
            "Full database access and data exfiltration.".to_string(),
        )),
        recommendation: Set(Some("Use parameterized queries.".to_string())),
        severity: Set("extreme".to_string()),
        cve_id: Set(Some("CVE-2024-00001".to_string())),
        category: Set("injection".to_string()),
        evidence: Set(Some("POST /login Body: username=admin'--".to_string())),
        affected_asset: Set(Some("https://app.example.com/login".to_string())),
        status: Set("open".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    assert_eq!(f.cve_id, Some("CVE-2024-00001".to_string()));
    assert!(f
        .technical_description
        .unwrap()
        .contains("parameterization"));
    assert_eq!(
        f.affected_asset,
        Some("https://app.example.com/login".to_string())
    );
}

#[tokio::test]
#[serial]
async fn test_finding_severity_values() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "severity").await;

    for severity in &["extreme", "high", "elevated", "moderate", "low"] {
        let f = create_finding(
            db,
            org_id,
            eng.id,
            user.id,
            &format!("{severity} finding"),
            severity,
            "misconfig",
        )
        .await;
        assert_eq!(f.severity, *severity);
    }
}

#[tokio::test]
#[serial]
async fn test_finding_category_values() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "category").await;

    for category in &[
        "injection",
        "broken_auth",
        "xss",
        "idor",
        "misconfig",
        "sensitive_data_exposure",
        "broken_access_control",
    ] {
        let f = create_finding(
            db,
            org_id,
            eng.id,
            user.id,
            &format!("{category} finding"),
            "moderate",
            category,
        )
        .await;
        assert_eq!(f.category, *category);
    }
}

#[tokio::test]
#[serial]
async fn test_finding_update_preserves_timestamps() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (user, org_id, eng) = setup_engagement(db, "update").await;
    let f = create_finding(
        db,
        org_id,
        eng.id,
        user.id,
        "Will Update",
        "low",
        "misconfig",
    )
    .await;
    let original_updated = f.updated_at;

    let mut active: findings::ActiveModel = f.into();
    active.severity = Set("high".to_string());
    let updated = active.update(db).await.unwrap();

    assert_eq!(updated.severity, "high");
    assert!(updated.updated_at >= original_updated);
}
