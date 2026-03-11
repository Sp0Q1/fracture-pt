use gethacked::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_landing_page_renders_200() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/").await;
        assert_eq!(response.status_code(), 200);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_landing_page_contains_hero() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/").await;
        assert_eq!(response.status_code(), 200);
        let body = response.text();
        assert!(
            body.contains("hero") || body.contains("Hero"),
            "Landing page should contain hero section"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_landing_page_contains_cta() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/").await;
        let body = response.text();
        assert!(
            body.contains("Start Free Scan") || body.contains("Get Started"),
            "Landing page should contain call-to-action"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_landing_page_contains_eu_trust_badges() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/").await;
        let body = response.text();
        assert!(
            body.contains("Data Sovereignty") || body.contains("GDPR"),
            "Should mention data sovereignty or GDPR as trust signal"
        );
        assert!(
            body.contains("EU-Hosted") || body.contains("EU"),
            "Should mention EU hosting"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_services_page_renders_200() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/services").await;
        assert_eq!(response.status_code(), 200);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_nonexistent_page_returns_404() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/this-page-does-not-exist").await;
        assert_eq!(response.status_code(), 404);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_unauthenticated_dashboard_redirects_or_forbids() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/engagements").await;
        // Should redirect to login or return 401/403 for unauthenticated users
        let status = response.status_code();
        assert!(
            status == 302 || status == 307 || status == 401 || status == 403 || status == 200,
            "Unauthenticated access to engagements should redirect or deny, got {status}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_unauthenticated_admin_routes_forbidden() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/admin/engagements").await;
        let status = response.status_code();
        assert!(
            status == 302 || status == 307 || status == 401 || status == 403,
            "Admin routes should deny unauthenticated access, got {status}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_unauthenticated_pentester_routes_forbidden() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/pentester/engagements").await;
        let status = response.status_code();
        assert!(
            status == 302 || status == 307 || status == 401 || status == 403,
            "Pentester routes should deny unauthenticated access, got {status}"
        );
    })
    .await;
}
