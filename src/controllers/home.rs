use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

use super::middleware;
use crate::models::_entities::{
    engagements, findings, job_definitions, job_runs, reports, scan_targets,
};
use crate::models::organizations as org_model;
use crate::views;
use crate::views::home::DashboardData;

/// Severity breakdown for the dashboard bar chart.
#[derive(Default)]
struct SeverityBreakdown {
    extreme: u64,
    high: u64,
    elevated: u64,
    moderate: u64,
    low: u64,
}

impl SeverityBreakdown {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn to_json(&self) -> serde_json::Value {
        let total = self.extreme + self.high + self.elevated + self.moderate + self.low;
        if total == 0 {
            return serde_json::json!({
                "extreme": 0, "high": 0, "elevated": 0, "moderate": 0, "low": 0,
                "extreme_pct": 0, "high_pct": 0, "elevated_pct": 0, "moderate_pct": 0, "low_pct": 0,
                "total": 0,
            });
        }
        let pct = |n: u64| (n as f64 / total as f64 * 100.0).round() as u64;
        serde_json::json!({
            "extreme": self.extreme,
            "high": self.high,
            "elevated": self.elevated,
            "moderate": self.moderate,
            "low": self.low,
            "extreme_pct": pct(self.extreme),
            "high_pct": pct(self.high),
            "elevated_pct": pct(self.elevated),
            "moderate_pct": pct(self.moderate),
            "low_pct": pct(self.low),
            "total": total,
        })
    }
}

/// Fetch stat counts (targets, engagements, findings, reports) for an org.
async fn fetch_stat_counts(db: &sea_orm::DatabaseConnection, org_id: i32) -> (u64, u64, u64, u64) {
    let targets = scan_targets::Entity::find()
        .filter(scan_targets::Column::OrgId.eq(org_id))
        .count(db)
        .await
        .unwrap_or(0);

    let engs = engagements::Entity::find()
        .filter(engagements::Column::OrgId.eq(org_id))
        .count(db)
        .await
        .unwrap_or(0);

    let total_findings = findings::Entity::find()
        .filter(findings::Column::OrgId.eq(org_id))
        .count(db)
        .await
        .unwrap_or(0);

    let reps = reports::Entity::find()
        .filter(reports::Column::OrgId.eq(org_id))
        .count(db)
        .await
        .unwrap_or(0);

    (targets, engs, total_findings, reps)
}

/// Compute severity breakdown from all findings in an org.
async fn compute_severity(db: &sea_orm::DatabaseConnection, org_id: i32) -> SeverityBreakdown {
    let all_findings = findings::Entity::find()
        .filter(findings::Column::OrgId.eq(org_id))
        .all(db)
        .await
        .unwrap_or_default();

    let mut sev = SeverityBreakdown::default();
    for f in &all_findings {
        match f.severity.as_str() {
            "extreme" => sev.extreme += 1,
            "high" => sev.high += 1,
            "elevated" => sev.elevated += 1,
            "moderate" => sev.moderate += 1,
            "low" => sev.low += 1,
            _ => {}
        }
    }
    sev
}

/// Find the latest completed ASM scan summary for an org.
async fn fetch_asm_summary(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
) -> Option<serde_json::Value> {
    let asm_defs = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_id))
        .filter(job_definitions::Column::JobType.eq("asm_scan"))
        .all(db)
        .await
        .unwrap_or_default();

    for def in &asm_defs {
        let run = job_runs::Model::find_latest_completed_by_definition(db, def.id).await;
        if let Some(run) = run {
            if let Some(summary_str) = run.result_summary.as_deref() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(summary_str) {
                    let domain = v
                        .get("domain")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let subdomain_count = v
                        .get("subdomain_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    let cert_count = v
                        .get("cert_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    let resolved_count = v
                        .get("subdomains")
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, |arr| {
                            arr.iter()
                                .filter(|s| {
                                    s.get("resolved")
                                        .and_then(serde_json::Value::as_bool)
                                        .unwrap_or(false)
                                })
                                .count() as u64
                        });

                    let target_pid = scan_targets::Entity::find()
                        .filter(scan_targets::Column::OrgId.eq(org_id))
                        .filter(scan_targets::Column::Hostname.eq(&domain))
                        .one(db)
                        .await
                        .ok()
                        .flatten()
                        .map(|t| t.pid.to_string());

                    return Some(serde_json::json!({
                        "domain": domain,
                        "subdomain_count": subdomain_count,
                        "resolved_count": resolved_count,
                        "cert_count": cert_count,
                        "target_pid": target_pid,
                    }));
                }
            }
        }
    }
    None
}

/// Fetch active engagements (in_progress or review) with findings counts.
async fn fetch_active_engagements(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
) -> Vec<serde_json::Value> {
    let active = engagements::Entity::find()
        .filter(engagements::Column::OrgId.eq(org_id))
        .filter(
            engagements::Column::Status
                .eq("in_progress")
                .or(engagements::Column::Status.eq("review")),
        )
        .order_by_desc(engagements::Column::UpdatedAt)
        .all(db)
        .await
        .unwrap_or_default();

    let mut active_json = Vec::new();
    for eng in &active {
        let fc = findings::Entity::find()
            .filter(findings::Column::EngagementId.eq(eng.id))
            .count(db)
            .await
            .unwrap_or(0);
        active_json.push(serde_json::json!({
            "pid": eng.pid.to_string(),
            "title": eng.title,
            "status": eng.status,
            "findings_count": fc,
        }));
    }
    active_json
}

/// Render the home page (authenticated or guest).
#[debug_handler]
pub async fn index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    match user {
        Some(user) => {
            let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
            let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;

            let dashboard_data = if let Some(ref oc) = org_ctx {
                let org_id = oc.org.id;
                let db = &ctx.db;

                let (target_count, engagement_count, findings_count, report_count) =
                    fetch_stat_counts(db, org_id).await;
                let severity = compute_severity(db, org_id).await;
                let asm_summary = fetch_asm_summary(db, org_id).await;
                let active_engagements = fetch_active_engagements(db, org_id).await;

                DashboardData {
                    target_count,
                    engagement_count,
                    findings_count,
                    report_count,
                    severity: severity.to_json(),
                    asm_summary,
                    active_engagements,
                }
            } else {
                DashboardData {
                    target_count: 0,
                    engagement_count: 0,
                    findings_count: 0,
                    report_count: 0,
                    severity: SeverityBreakdown::default().to_json(),
                    asm_summary: None,
                    active_engagements: Vec::new(),
                }
            };

            views::home::index(&v, &user, &org_ctx, &user_orgs, &dashboard_data)
        }
        None => views::home::index_guest(&v),
    }
}

pub fn routes() -> Routes {
    Routes::new().prefix("/").add("", get(index))
}
