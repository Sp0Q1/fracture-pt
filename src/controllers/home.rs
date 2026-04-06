use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{
    ColumnTrait, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use std::collections::HashMap;

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

/// Result struct for the severity GROUP BY query.
#[derive(Debug, FromQueryResult)]
struct SeverityCount {
    severity: String,
    count: i64,
}

/// Compute severity breakdown using a GROUP BY aggregate query.
/// This avoids loading all findings into memory.
async fn compute_severity(db: &sea_orm::DatabaseConnection, org_id: i32) -> SeverityBreakdown {
    let rows: Vec<SeverityCount> = findings::Entity::find()
        .filter(findings::Column::OrgId.eq(org_id))
        .select_only()
        .column(findings::Column::Severity)
        .column_as(findings::Column::Id.count(), "count")
        .group_by(findings::Column::Severity)
        .into_model::<SeverityCount>()
        .all(db)
        .await
        .unwrap_or_default();

    let mut sev = SeverityBreakdown::default();
    for row in &rows {
        #[allow(clippy::cast_sign_loss)]
        let n = row.count as u64;
        match row.severity.as_str() {
            "extreme" => sev.extreme = n,
            "high" => sev.high = n,
            "elevated" => sev.elevated = n,
            "moderate" => sev.moderate = n,
            "low" => sev.low = n,
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
/// Uses a batch query for findings counts to avoid N+1.
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

    if active.is_empty() {
        return Vec::new();
    }

    // Batch-fetch findings counts for all active engagements in one query
    let eng_ids: Vec<i32> = active.iter().map(|e| e.id).collect();
    let counts: Vec<EngagementFindingsCount> = findings::Entity::find()
        .filter(findings::Column::EngagementId.is_in(eng_ids))
        .select_only()
        .column(findings::Column::EngagementId)
        .column_as(findings::Column::Id.count(), "count")
        .group_by(findings::Column::EngagementId)
        .into_model::<EngagementFindingsCount>()
        .all(db)
        .await
        .unwrap_or_default();

    let count_map: HashMap<i32, i64> = counts
        .into_iter()
        .filter_map(|c| c.engagement_id.map(|eid| (eid, c.count)))
        .collect();

    active
        .iter()
        .map(|eng| {
            let fc = count_map.get(&eng.id).copied().unwrap_or(0);
            serde_json::json!({
                "pid": eng.pid.to_string(),
                "title": eng.title,
                "status": eng.status,
                "findings_count": fc,
            })
        })
        .collect()
}

/// Result struct for engagement findings count GROUP BY query.
#[derive(Debug, FromQueryResult)]
struct EngagementFindingsCount {
    engagement_id: Option<i32>,
    count: i64,
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

                // Run all dashboard queries concurrently
                let (stat_counts, severity, asm_summary, active_engagements) = tokio::join!(
                    fetch_stat_counts(db, org_id),
                    compute_severity(db, org_id),
                    fetch_asm_summary(db, org_id),
                    fetch_active_engagements(db, org_id),
                );

                let (target_count, engagement_count, findings_count, report_count) = stat_counts;

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
