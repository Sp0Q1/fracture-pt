use fracture_pt::app::App;
use fracture_pt::models::{
    engagements, organizations, reports, services,
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
            subject: format!("test-report-{suffix}"),
            email: format!("report-{suffix}@example.com"),
            name: Some(format!("Report User {suffix}")),
        },
    )
    .await
    .expect("Failed to create test user")
}

async fn setup_engagement(
    db: &sea_orm::DatabaseConnection,
    suffix: &str,
) -> (i32, engagements::Model) {
    let user = create_test_user(db, suffix).await;
    let orgs = organizations::Model::find_orgs_for_user(db, user.id)
        .await
        .unwrap();
    let org_id = orgs[0].id;
    let svc = services::ActiveModel {
        name: Set(format!("Svc {suffix}")),
        slug: Set(format!("report-svc-{suffix}")),
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
        status: Set("completed".to_string()),
        target_systems: Set("api.example.com".to_string()),
        contact_name: Set("Test".to_string()),
        contact_email: Set("test@example.com".to_string()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    (org_id, eng)
}

async fn create_report(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    engagement_id: i32,
    title: &str,
) -> reports::Model {
    reports::ActiveModel {
        org_id: Set(org_id),
        engagement_id: Set(Some(engagement_id)),
        title: Set(title.to_string()),
        report_type: Set("pentest_report".to_string()),
        format: Set("pdf".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("Failed to create report")
}

#[tokio::test]
#[serial]
async fn test_create_report() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (org_id, eng) = setup_engagement(db, "create").await;
    let r = create_report(db, org_id, eng.id, "Q1 Pentest Report").await;

    assert_eq!(r.title, "Q1 Pentest Report");
    assert_eq!(r.report_type, "pentest_report");
    assert_eq!(r.format, "pdf");
    assert_eq!(r.org_id, org_id);
    assert_eq!(r.engagement_id, Some(eng.id));
}

#[tokio::test]
#[serial]
async fn test_report_sets_pid_on_insert() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (org_id, eng) = setup_engagement(db, "pid").await;
    let r = create_report(db, org_id, eng.id, "PID Report").await;
    assert!(!r.pid.is_nil());
}

#[tokio::test]
#[serial]
async fn test_find_reports_by_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (org_id, eng) = setup_engagement(db, "byorg").await;
    create_report(db, org_id, eng.id, "Report A").await;
    create_report(db, org_id, eng.id, "Report B").await;

    let items = reports::Model::find_by_org(db, org_id).await;
    assert_eq!(items.len(), 2);
    // Ordered by id DESC
    assert_eq!(items[0].title, "Report B");
    assert_eq!(items[1].title, "Report A");
}

#[tokio::test]
#[serial]
async fn test_find_reports_by_engagement() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (org_id, eng) = setup_engagement(db, "byeng").await;
    create_report(db, org_id, eng.id, "Eng Report").await;

    let items = reports::Model::find_by_engagement(db, eng.id).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Eng Report");

    let empty = reports::Model::find_by_engagement(db, eng.id + 999).await;
    assert!(empty.is_empty());
}

#[tokio::test]
#[serial]
async fn test_find_report_by_pid_and_org() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (org_id, eng) = setup_engagement(db, "bypidorg").await;
    let r = create_report(db, org_id, eng.id, "Scoped Report").await;

    let found = reports::Model::find_by_pid_and_org(db, &r.pid.to_string(), org_id).await;
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Scoped Report");

    // Wrong org returns None (IDOR prevention)
    let not_found = reports::Model::find_by_pid_and_org(db, &r.pid.to_string(), org_id + 999).await;
    assert!(not_found.is_none());
}

#[tokio::test]
#[serial]
async fn test_report_cross_org_isolation() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (alice_org_id, alice_eng) = setup_engagement(db, "iso-alice").await;
    let (bob_org_id, bob_eng) = setup_engagement(db, "iso-bob").await;

    let alice_report = create_report(db, alice_org_id, alice_eng.id, "Alice Report").await;
    create_report(db, bob_org_id, bob_eng.id, "Bob Report").await;

    // Alice's org only sees Alice's reports
    let alice_items = reports::Model::find_by_org(db, alice_org_id).await;
    assert_eq!(alice_items.len(), 1);
    assert_eq!(alice_items[0].title, "Alice Report");

    // Cross-org pid lookup fails
    let cross =
        reports::Model::find_by_pid_and_org(db, &alice_report.pid.to_string(), bob_org_id).await;
    assert!(cross.is_none());
}

#[tokio::test]
#[serial]
async fn test_report_with_storage_path() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let (org_id, eng) = setup_engagement(db, "storage").await;

    let r = reports::ActiveModel {
        org_id: Set(org_id),
        engagement_id: Set(Some(eng.id)),
        title: Set("Ready Report".to_string()),
        report_type: Set("pentest_report".to_string()),
        format: Set("pdf".to_string()),
        storage_path: Set(Some("/reports/2026/q1-pentest.pdf".to_string())),
        generated_at: Set(Some(chrono::Utc::now().into())),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    assert_eq!(
        r.storage_path,
        Some("/reports/2026/q1-pentest.pdf".to_string())
    );
    assert!(r.generated_at.is_some());
}
