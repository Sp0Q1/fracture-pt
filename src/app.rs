use async_trait::async_trait;
use axum::Router as AxumRouter;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    bgworker::Queue,
    boot::{create_app, BootResult, StartMode},
    config::Config,
    controller::AppRoutes,
    db::truncate_table,
    environment::Environment,
    task::Tasks,
    Result,
};
use migration::Migrator;
use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait};
use std::path::Path;

use loco_rs::bgworker::BackgroundWorker;

use crate::{
    controllers, initializers,
    jobs::{
        asm_scan::AsmScanExecutor, port_scan::PortScanExecutor, report_build::ReportBuildExecutor,
    },
    models::_entities::{
        blog_posts, engagement_offers, engagement_targets, engagements, findings, invoices,
        job_definitions, job_run_diffs, job_runs, non_findings, org_invites, org_members,
        organizations, pentester_assignments, pricing_tiers, reports, scan_jobs, scan_targets,
        services, subscriptions, users,
    },
    workers,
};

use fracture_core::entity_registry::{AdminEntity, EntityRegistry};
use fracture_core::jobs::{init_job_registry, JobRegistry};

// ---------------------------------------------------------------------------
// Gethacked-specific admin entity implementations
// ---------------------------------------------------------------------------

struct EngagementsEntity;

#[async_trait]
impl AdminEntity for EngagementsEntity {
    fn entity_name(&self) -> &'static str {
        "Engagements"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/engagements"
    }
    fn description(&self) -> &'static str {
        "Manage engagement requests"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        engagements::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct ScanTargetsEntity;

#[async_trait]
impl AdminEntity for ScanTargetsEntity {
    fn entity_name(&self) -> &'static str {
        "Scan Targets"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/scan-targets"
    }
    fn description(&self) -> &'static str {
        "Monitored targets across organizations"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        scan_targets::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct ScanJobsEntity;

#[async_trait]
impl AdminEntity for ScanJobsEntity {
    fn entity_name(&self) -> &'static str {
        "Scan Jobs"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/scan-jobs"
    }
    fn description(&self) -> &'static str {
        "Scan execution history"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        scan_jobs::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct FindingsEntity;

#[async_trait]
impl AdminEntity for FindingsEntity {
    fn entity_name(&self) -> &'static str {
        "Findings"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/findings"
    }
    fn description(&self) -> &'static str {
        "Security findings across engagements"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        findings::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct NonFindingsEntity;

#[async_trait]
impl AdminEntity for NonFindingsEntity {
    fn entity_name(&self) -> &'static str {
        "Non-Findings"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/non-findings"
    }
    fn description(&self) -> &'static str {
        "Secure areas documented during engagements"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        non_findings::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct ReportsEntity;

#[async_trait]
impl AdminEntity for ReportsEntity {
    fn entity_name(&self) -> &'static str {
        "Reports"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/reports"
    }
    fn description(&self) -> &'static str {
        "Generated reports"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        reports::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct InvoicesEntity;

#[async_trait]
impl AdminEntity for InvoicesEntity {
    fn entity_name(&self) -> &'static str {
        "Invoices"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/invoices"
    }
    fn description(&self) -> &'static str {
        "Billing invoices"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        invoices::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct SubscriptionsEntity;

#[async_trait]
impl AdminEntity for SubscriptionsEntity {
    fn entity_name(&self) -> &'static str {
        "Subscriptions"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/subscriptions"
    }
    fn description(&self) -> &'static str {
        "Active subscriptions"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        subscriptions::Entity::find().count(db).await.unwrap_or(0)
    }
}

struct UsersEntity;

#[async_trait]
impl AdminEntity for UsersEntity {
    fn entity_name(&self) -> &'static str {
        "Users"
    }
    fn url_prefix(&self) -> &'static str {
        "/admin/users"
    }
    fn description(&self) -> &'static str {
        "Registered platform users"
    }
    async fn count_all(&self, db: &DatabaseConnection) -> u64 {
        users::Entity::find().count(db).await.unwrap_or(0)
    }
}

fn build_entity_registry() -> EntityRegistry {
    let mut registry = EntityRegistry::new();
    // Re-register core entities (with custom UsersEntity url_prefix)
    registry.register(Box::new(fracture_core::entity_registry::OrgsEntity));
    registry.register(Box::new(UsersEntity));
    registry.register(Box::new(fracture_core::entity_registry::BlogPostsEntity));
    registry.register(Box::new(
        fracture_core::entity_registry::JobDefinitionsEntity,
    ));
    registry.register(Box::new(fracture_core::entity_registry::JobRunsEntity));
    // Gethacked-specific entities
    registry.register(Box::new(EngagementsEntity));
    registry.register(Box::new(ScanTargetsEntity));
    registry.register(Box::new(ScanJobsEntity));
    registry.register(Box::new(FindingsEntity));
    registry.register(Box::new(NonFindingsEntity));
    registry.register(Box::new(ReportsEntity));
    registry.register(Box::new(InvoicesEntity));
    registry.register(Box::new(SubscriptionsEntity));
    registry
}

pub struct App;
#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        create_app::<Self, Migrator>(mode, environment, config).await
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![
            Box::new(initializers::view_engine::TemplateInitializer),
            Box::new(initializers::oidc::OidcInitializer),
            Box::new(initializers::security_headers::SecurityHeadersInitializer),
        ])
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        // Initialise the entity registry with gethacked-specific entities
        fracture_core::entity_registry::init_entity_registry(build_entity_registry());

        // Initialise the job registry with gethacked-specific executors
        let mut job_reg = JobRegistry::new();
        job_reg.register(Box::new(AsmScanExecutor));
        job_reg.register(Box::new(PortScanExecutor));
        job_reg.register(Box::new(ReportBuildExecutor));
        init_job_registry(job_reg);

        AppRoutes::with_default_routes()
            // Org routes with tier-aware settings override (replaces core org::routes)
            .add_route(controllers::org_settings::routes())
            .add_route(controllers::org::invite_routes())
            .add_route(controllers::oidc::routes())
            .add_route(controllers::blog::public_routes())
            .add_route(controllers::blog::admin_routes())
            .add_route(controllers::jobs::org_routes())
            .add_route(controllers::jobs::admin_routes())
            .add_route(fracture_core::controllers::admin::routes())
            // Public pages
            .add_route(controllers::home::routes())
            .add_route(controllers::pages::routes())
            .add_route(controllers::contact::routes())
            .add_route(controllers::free_scan::routes())
            .add_route(controllers::service::routes())
            .add_route(controllers::subscription::routes())
            .add_route(controllers::scan_target::routes())
            .add_route(controllers::scan_job::routes())
            .add_route(controllers::finding::routes())
            .add_route(controllers::engagement::routes())
            .add_route(controllers::report::routes())
            .add_route(controllers::invoice::routes())
            // Admin routes (gethacked-specific)
            .add_route(controllers::admin::routes())
            // Pentester routes
            .add_route(controllers::pentester::routes())
    }

    async fn after_routes(router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter> {
        // Auto-seed development data if the services table is empty
        Self::seed(ctx, Path::new(".")).await.ok();
        Ok(router.fallback(controllers::fallback::not_found))
    }

    async fn connect_workers(ctx: &AppContext, queue: &Queue) -> Result<()> {
        queue
            .register(workers::job_dispatcher::JobDispatchWorker::build(ctx))
            .await?;
        queue
            .register(workers::job_scheduler::JobSchedulerWorker::build(ctx))
            .await?;
        Ok(())
    }

    fn register_tasks(_tasks: &mut Tasks) {}

    async fn truncate(ctx: &AppContext) -> Result<()> {
        // Children first (reverse FK dependency order)
        truncate_table(&ctx.db, non_findings::Entity).await?;
        truncate_table(&ctx.db, findings::Entity).await?;
        truncate_table(&ctx.db, reports::Entity).await?;
        truncate_table(&ctx.db, pentester_assignments::Entity).await?;
        truncate_table(&ctx.db, engagement_offers::Entity).await?;
        truncate_table(&ctx.db, engagement_targets::Entity).await?;
        truncate_table(&ctx.db, scan_jobs::Entity).await?;
        truncate_table(&ctx.db, invoices::Entity).await?;
        truncate_table(&ctx.db, engagements::Entity).await?;
        truncate_table(&ctx.db, subscriptions::Entity).await?;
        truncate_table(&ctx.db, scan_targets::Entity).await?;
        truncate_table(&ctx.db, pricing_tiers::Entity).await?;
        truncate_table(&ctx.db, services::Entity).await?;
        // Blog & job tables
        truncate_table(&ctx.db, blog_posts::Entity).await?;
        truncate_table(&ctx.db, job_run_diffs::Entity).await?;
        truncate_table(&ctx.db, job_runs::Entity).await?;
        truncate_table(&ctx.db, job_definitions::Entity).await?;
        // Core tables
        truncate_table(&ctx.db, org_invites::Entity).await?;
        truncate_table(&ctx.db, org_members::Entity).await?;
        truncate_table(&ctx.db, organizations::Entity).await?;
        truncate_table(&ctx.db, users::Entity).await?;
        Ok(())
    }

    async fn seed(ctx: &AppContext, _base: &Path) -> Result<()> {
        use sea_orm::ActiveValue::Set;
        use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait};

        let db = &ctx.db;

        // Only seed if the services table is empty
        let count = services::Entity::find().count(db).await?;
        if count > 0 {
            return Ok(());
        }

        let svc_data = [
            ("Penetration Testing", "pentesting", "pentest", "Professional penetration testing for web applications, APIs, mobile apps, and infrastructure. Our pentesters simulate real-world attacks to find vulnerabilities before malicious actors do.", false, 1),
            ("Vulnerability Scanning", "scanning", "scanning", "Automated vulnerability scanning for your digital assets. Continuous monitoring detects new vulnerabilities as they emerge, with prioritized reporting and remediation guidance.", true, 2),
            ("Red Team Operations", "red-team", "offensive", "Full-scope adversary simulation testing your people, processes, and technology. Our red team uses real-world TTPs to evaluate your detection and response capabilities.", false, 3),
            ("Attack Surface Mapping", "attack-surface-mapping", "scanning", "Continuous discovery and monitoring of your external attack surface. Enumerate subdomains, exposed services, open ports, certificates, and shadow IT. Know what attackers see before they act.", true, 4),
        ];

        for (name, slug, category, description, is_automated, sort_order) in &svc_data {
            let svc = services::ActiveModel {
                name: Set(name.to_string()),
                slug: Set(slug.to_string()),
                category: Set(category.to_string()),
                description: Set(description.to_string()),
                is_automated: Set(*is_automated),
                is_active: Set(true),
                sort_order: Set(*sort_order),
                ..Default::default()
            };
            let svc = svc.insert(db).await?;

            if slug == &"pentesting" {
                let tiers = [
                    ("Recon", "recon", 349_900, "one_time", 3, 0, "Up to 3 applications/APIs,Full methodology pentest,Detailed technical report,60-day remediation support,Retest included", 1),
                    ("Strike", "strike", 749_900, "one_time", 10, 0, "Full infrastructure scope,Advanced persistent threat simulation,Board-ready report,90-day remediation support,Two retests included,Dedicated team lead", 2),
                ];
                for (tname, tslug, price, period, targets, scans, features, order) in &tiers {
                    pricing_tiers::ActiveModel {
                        service_id: Set(svc.id),
                        name: Set(tname.to_string()),
                        slug: Set(tslug.to_string()),
                        price_cents: Set(*price),
                        billing_period: Set(period.to_string()),
                        max_targets: Set(*targets),
                        max_scans_per_month: Set(*scans),
                        features: Set(features.to_string()),
                        is_active: Set(true),
                        sort_order: Set(*order),
                        ..Default::default()
                    }
                    .insert(db)
                    .await?;
                }
            } else if slug == &"attack-surface-mapping" {
                let tiers = [
                    ("Free Scan", "free-scan", 0, "one_time", 1, 1, "Single domain scan,Subdomain enumeration,Open port discovery,Certificate transparency check,Basic summary report", 1),
                    ("Continuous", "continuous", 4_990, "monthly", 10, 0, "Up to 10 domains,Continuous monitoring,Change alerts,Shadow IT detection,Weekly digest,API access", 2),
                    ("Enterprise ASM", "enterprise-asm", 0, "monthly", 0, 0, "Unlimited domains,Real-time alerting,Dark web monitoring,Custom integrations,Dedicated support", 3),
                ];
                for (tname, tslug, price, period, targets, scans, features, order) in &tiers {
                    pricing_tiers::ActiveModel {
                        service_id: Set(svc.id),
                        name: Set(tname.to_string()),
                        slug: Set(tslug.to_string()),
                        price_cents: Set(*price),
                        billing_period: Set(period.to_string()),
                        max_targets: Set(*targets),
                        max_scans_per_month: Set(*scans),
                        features: Set(features.to_string()),
                        is_active: Set(true),
                        sort_order: Set(*order),
                        ..Default::default()
                    }
                    .insert(db)
                    .await?;
                }
            } else if slug == &"scanning" {
                let tiers = [
                    ("Basic Scan", "basic-scan", 2_900, "monthly", 5, 1, "Up to 5 targets,Monthly scans,Basic vulnerability detection,Email alerts", 1),
                    ("Pro Scan", "pro-scan", 9_900, "monthly", 25, 4, "Up to 25 targets,Weekly scans,Advanced detection engine,Slack/webhook integration,Priority support", 2),
                    ("Enterprise Scan", "enterprise-scan", 29_900, "monthly", 0, 0, "Unlimited targets,Daily scans,Custom scan profiles,API access,Dedicated support,SLA guarantee", 3),
                ];
                for (tname, tslug, price, period, targets, scans, features, order) in &tiers {
                    pricing_tiers::ActiveModel {
                        service_id: Set(svc.id),
                        name: Set(tname.to_string()),
                        slug: Set(tslug.to_string()),
                        price_cents: Set(*price),
                        billing_period: Set(period.to_string()),
                        max_targets: Set(*targets),
                        max_scans_per_month: Set(*scans),
                        features: Set(features.to_string()),
                        is_active: Set(true),
                        sort_order: Set(*order),
                        ..Default::default()
                    }
                    .insert(db)
                    .await?;
                }
            }
        }

        tracing::info!("seed data inserted: 4 services with pricing tiers");
        Ok(())
    }
}
