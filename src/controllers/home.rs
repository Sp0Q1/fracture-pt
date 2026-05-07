use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{
    ColumnTrait, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use std::collections::HashMap;

use super::middleware;
use crate::models::_entities::{
    engagement_targets, engagements, findings, job_definitions, job_runs, reports, scan_targets,
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
        let run = job_runs::Model::find_latest_completed_by_definition(db, def.id)
            .await
            .ok()
            .flatten();
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

/// Result struct for per-target findings severity GROUP BY query.
#[derive(Debug, FromQueryResult)]
struct TargetSeverityCount {
    affected_asset: Option<String>,
    severity: String,
    count: i64,
}

/// Build the attack surface overview: one entry per target with subdomain count,
/// open port count, and per-severity finding counts.
#[allow(clippy::too_many_lines)]
async fn fetch_attack_surface(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
) -> Vec<serde_json::Value> {
    let targets = scan_targets::Entity::find()
        .filter(scan_targets::Column::OrgId.eq(org_id))
        .all(db)
        .await
        .unwrap_or_default();

    if targets.is_empty() {
        return Vec::new();
    }

    // Collect all target IDs for batch queries
    let target_ids: Vec<i32> = targets.iter().map(|t| t.id).collect();

    // Collect all hostnames (for matching findings.affected_asset)
    let hostnames: Vec<String> = targets.iter().filter_map(|t| t.hostname.clone()).collect();

    // --- Batch: get engagement IDs linked to these targets ---
    let eng_target_links = engagement_targets::Entity::find()
        .filter(engagement_targets::Column::ScanTargetId.is_in(target_ids.clone()))
        .all(db)
        .await
        .unwrap_or_default();

    // Build scan_target_id -> set of engagement_ids
    let mut target_eng_map: HashMap<i32, Vec<i32>> = HashMap::new();
    for link in &eng_target_links {
        target_eng_map
            .entry(link.scan_target_id)
            .or_default()
            .push(link.engagement_id);
    }

    // --- Batch: findings severity counts grouped by affected_asset ---
    let hostname_severity_counts: Vec<TargetSeverityCount> = if hostnames.is_empty() {
        Vec::new()
    } else {
        findings::Entity::find()
            .filter(findings::Column::OrgId.eq(org_id))
            .filter(findings::Column::AffectedAsset.is_in(hostnames.clone()))
            .select_only()
            .column(findings::Column::AffectedAsset)
            .column(findings::Column::Severity)
            .column_as(findings::Column::Id.count(), "count")
            .group_by(findings::Column::AffectedAsset)
            .group_by(findings::Column::Severity)
            .into_model::<TargetSeverityCount>()
            .all(db)
            .await
            .unwrap_or_default()
    };

    // Build hostname -> severity -> count
    let mut hostname_sev_map: HashMap<String, HashMap<String, i64>> = HashMap::new();
    for row in &hostname_severity_counts {
        if let Some(ref asset) = row.affected_asset {
            hostname_sev_map
                .entry(asset.clone())
                .or_default()
                .insert(row.severity.clone(), row.count);
        }
    }

    // --- Batch: findings via engagement_targets link ---
    let all_eng_ids: Vec<i32> = eng_target_links.iter().map(|l| l.engagement_id).collect();
    let eng_severity_counts: Vec<EngagementSeverityCount> = if all_eng_ids.is_empty() {
        Vec::new()
    } else {
        findings::Entity::find()
            .filter(findings::Column::EngagementId.is_in(all_eng_ids))
            .select_only()
            .column(findings::Column::EngagementId)
            .column(findings::Column::Severity)
            .column_as(findings::Column::Id.count(), "count")
            .group_by(findings::Column::EngagementId)
            .group_by(findings::Column::Severity)
            .into_model::<EngagementSeverityCount>()
            .all(db)
            .await
            .unwrap_or_default()
    };

    // Build engagement_id -> severity -> count
    let mut eng_sev_map: HashMap<i32, HashMap<String, i64>> = HashMap::new();
    for row in &eng_severity_counts {
        if let Some(eid) = row.engagement_id {
            eng_sev_map
                .entry(eid)
                .or_default()
                .insert(row.severity.clone(), row.count);
        }
    }

    // --- Fetch ASM + amass + port scan summaries per target ---
    let all_defs = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_id))
        .filter(
            job_definitions::Column::JobType
                .eq("asm_scan")
                .or(job_definitions::Column::JobType.eq("port_scan"))
                .or(job_definitions::Column::JobType.eq("amass_passive")),
        )
        .all(db)
        .await
        .unwrap_or_default();

    // Group definitions by target — match by target_id from config,
    // or by hostname/domain if target_id is missing (e.g. free scans).
    let hostname_to_target_id: HashMap<&str, i32> = targets
        .iter()
        .filter_map(|t| t.hostname.as_deref().map(|h| (h, t.id)))
        .collect();
    let mut asm_defs_by_target: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut amass_defs_by_target: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut port_defs_by_target: HashMap<i32, Vec<i32>> = HashMap::new();
    for def in &all_defs {
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&def.config) {
            // Try matching by target_id first, fall back to hostname / domain.
            let tid = cfg["target_id"]
                .as_i64()
                .map(|id| {
                    #[allow(clippy::cast_possible_truncation)]
                    let id = id as i32;
                    id
                })
                .or_else(|| {
                    cfg["hostname"]
                        .as_str()
                        .or_else(|| cfg["domain"].as_str())
                        .and_then(|h| hostname_to_target_id.get(h))
                        .copied()
                });
            if let Some(tid) = tid {
                match def.job_type.as_str() {
                    "asm_scan" => asm_defs_by_target.entry(tid).or_default().push(def.id),
                    "amass_passive" => amass_defs_by_target.entry(tid).or_default().push(def.id),
                    "port_scan" => port_defs_by_target.entry(tid).or_default().push(def.id),
                    _ => {}
                }
            }
        }
    }

    // Fetch latest completed runs for all relevant definition IDs
    let all_def_ids: Vec<i32> = all_defs.iter().map(|d| d.id).collect();
    let all_completed_runs = if all_def_ids.is_empty() {
        Vec::new()
    } else {
        job_runs::Entity::find()
            .filter(job_runs::Column::JobDefinitionId.is_in(all_def_ids))
            .filter(job_runs::Column::Status.eq("completed"))
            .order_by_desc(job_runs::Column::CompletedAt)
            .all(db)
            .await
            .unwrap_or_default()
    };

    // Build definition_id -> latest completed run (first match since ordered desc)
    let mut latest_run_by_def: HashMap<i32, &job_runs::Model> = HashMap::new();
    for run in &all_completed_runs {
        latest_run_by_def
            .entry(run.job_definition_id)
            .or_insert(run);
    }

    // --- Assemble per-target data ---
    let mut result = Vec::with_capacity(targets.len());

    for target in &targets {
        let hostname = target
            .hostname
            .as_deref()
            .or(target.ip_address.as_deref())
            .unwrap_or("unknown");

        // Subdomain set: union of latest ASM scan + latest amass passive,
        // deduplicated by name. Either source alone often misses entries
        // the other found (CT logs vs OSINT), so the union is what the
        // dashboard topology should display.
        let mut by_name: std::collections::BTreeMap<String, bool> =
            std::collections::BTreeMap::new();

        // From asm_scan: take {name, resolved}
        if let Some(def_ids) = asm_defs_by_target.get(&target.id) {
            for did in def_ids {
                let Some(run) = latest_run_by_def.get(did) else {
                    continue;
                };
                let Some(summary) = run.result_summary.as_deref() else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(summary) else {
                    continue;
                };
                if let Some(arr) = v.get("subdomains").and_then(serde_json::Value::as_array) {
                    for s in arr {
                        if let Some(name) = s.get("name").and_then(serde_json::Value::as_str) {
                            let resolved = s
                                .get("resolved")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false);
                            by_name
                                .entry(name.to_string())
                                .and_modify(|r| *r = *r || resolved)
                                .or_insert(resolved);
                        }
                    }
                }
            }
        }

        // From amass_passive: take hosts[].name; addresses non-empty == resolved
        if let Some(def_ids) = amass_defs_by_target.get(&target.id) {
            for did in def_ids {
                let Some(run) = latest_run_by_def.get(did) else {
                    continue;
                };
                let Some(summary) = run.result_summary.as_deref() else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(summary) else {
                    continue;
                };
                if let Some(arr) = v.get("hosts").and_then(serde_json::Value::as_array) {
                    for h in arr {
                        if let Some(name) = h.get("name").and_then(serde_json::Value::as_str) {
                            let resolved = h
                                .get("addresses")
                                .and_then(serde_json::Value::as_array)
                                .is_some_and(|a| !a.is_empty());
                            by_name
                                .entry(name.to_string())
                                .and_modify(|r| *r = *r || resolved)
                                .or_insert(resolved);
                        }
                    }
                }
            }
        }

        let subdomain_count = u64::try_from(by_name.len()).unwrap_or(u64::MAX);
        let subdomains_list: Vec<serde_json::Value> = by_name
            .into_iter()
            .take(30)
            .map(|(name, resolved)| serde_json::json!({"name": name, "resolved": resolved}))
            .collect();

        // Open port count from latest port scan
        let open_ports = port_defs_by_target
            .get(&target.id)
            .and_then(|def_ids| {
                def_ids.iter().find_map(|did| {
                    let run = latest_run_by_def.get(did)?;
                    let summary = run.result_summary.as_deref()?;
                    let v = serde_json::from_str::<serde_json::Value>(summary).ok()?;
                    v.get("total_open").and_then(serde_json::Value::as_u64)
                })
            })
            .unwrap_or(0);

        // Merge findings from hostname match and engagement links
        let mut merged_sev: HashMap<String, i64> = HashMap::new();

        if let Some(hn) = &target.hostname {
            if let Some(sev_map) = hostname_sev_map.get(hn) {
                for (sev, count) in sev_map {
                    *merged_sev.entry(sev.clone()).or_default() += count;
                }
            }
        }

        if let Some(eng_ids) = target_eng_map.get(&target.id) {
            for eid in eng_ids {
                if let Some(sev_map) = eng_sev_map.get(eid) {
                    for (sev, count) in sev_map {
                        *merged_sev.entry(sev.clone()).or_default() += count;
                    }
                }
            }
        }

        let findings_json = serde_json::json!({
            "extreme": merged_sev.get("extreme").copied().unwrap_or(0),
            "high": merged_sev.get("high").copied().unwrap_or(0),
            "elevated": merged_sev.get("elevated").copied().unwrap_or(0),
            "moderate": merged_sev.get("moderate").copied().unwrap_or(0),
            "low": merged_sev.get("low").copied().unwrap_or(0),
        });

        let findings_total = merged_sev.values().sum::<i64>();

        // Determine highest severity for border coloring
        let highest_severity = if merged_sev.get("extreme").copied().unwrap_or(0) > 0 {
            "extreme"
        } else if merged_sev.get("high").copied().unwrap_or(0) > 0 {
            "high"
        } else if merged_sev.get("elevated").copied().unwrap_or(0) > 0 {
            "elevated"
        } else if merged_sev.get("moderate").copied().unwrap_or(0) > 0 {
            "moderate"
        } else if merged_sev.get("low").copied().unwrap_or(0) > 0 {
            "low"
        } else {
            "none"
        };

        result.push(serde_json::json!({
            "hostname": hostname,
            "pid": target.pid.to_string(),
            "label": target.label.as_deref().unwrap_or(""),
            "target_type": target.target_type,
            "subdomains": subdomain_count,
            "open_ports": open_ports,
            "findings": findings_json,
            "findings_total": findings_total,
            "highest_severity": highest_severity,
            "subdomains_list": subdomains_list,
        }));
    }

    result
}

/// Result struct for engagement-scoped severity GROUP BY query.
#[derive(Debug, FromQueryResult)]
struct EngagementSeverityCount {
    engagement_id: Option<i32>,
    severity: String,
    count: i64,
}

/// Fetch the 10 most recent findings for an org.
async fn fetch_recent_findings(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
) -> Vec<serde_json::Value> {
    let recent = findings::Entity::find()
        .filter(findings::Column::OrgId.eq(org_id))
        .order_by_desc(findings::Column::CreatedAt)
        .limit(10)
        .all(db)
        .await
        .unwrap_or_default();

    recent
        .iter()
        .map(|f| {
            serde_json::json!({
                "pid": f.pid.to_string(),
                "title": f.title,
                "severity": f.severity,
                "affected_asset": f.affected_asset.as_deref().unwrap_or(""),
                "category": f.category,
                "created_at": f.created_at.to_rfc3339(),
            })
        })
        .collect()
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
            let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id)
                .await
                .unwrap_or_default();

            let dashboard_data = if let Some(ref oc) = org_ctx {
                let org_id = oc.org.id;
                let db = &ctx.db;

                // Run all dashboard queries concurrently
                let (
                    stat_counts,
                    severity,
                    asm_summary,
                    active_engagements,
                    attack_surface,
                    recent_findings,
                ) = tokio::join!(
                    fetch_stat_counts(db, org_id),
                    compute_severity(db, org_id),
                    fetch_asm_summary(db, org_id),
                    fetch_active_engagements(db, org_id),
                    fetch_attack_surface(db, org_id),
                    fetch_recent_findings(db, org_id),
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
                    attack_surface,
                    recent_findings,
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
                    attack_surface: Vec::new(),
                    recent_findings: Vec::new(),
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
