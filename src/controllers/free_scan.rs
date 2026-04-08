use axum::response::Redirect;
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;

use crate::controllers::middleware;
use crate::models::_entities::{job_definitions, job_runs, organizations};
use crate::views;
use crate::workers::job_dispatcher::{JobDispatchArgs, JobDispatchWorker};

#[derive(Debug, Deserialize)]
pub struct ScanForm {
    pub domain: String,
}

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }
    let blocked = [
        "localhost",
        ".local",
        ".test",
        ".example",
        ".invalid",
        ".onion",
        ".internal",
    ];
    let lower = domain.to_lowercase();
    if blocked.iter().any(|b| lower == *b || lower.ends_with(b)) {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

/// `POST /free-scan` — validate domain, create a job, redirect to progress page.
#[debug_handler]
pub async fn submit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    Form(form): Form<ScanForm>,
) -> Result<Response> {
    let domain = form.domain.trim().to_lowercase();

    if !is_valid_domain(&domain) {
        return format::render().view(
            &v,
            "asm/free-scan.html",
            data!({ "error": "Please enter a valid domain name (e.g. example.com)." }),
        );
    }

    let admin_org = organizations::Entity::find()
        .filter(organizations::Column::IsPlatformAdmin.eq(true))
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::BadRequest("Platform admin org not configured".into()))?;

    let def_name = format!("Free scan: {domain}");
    let config = serde_json::json!({
        "hostname": domain,
        "org_id": admin_org.id,
    });

    let definition = if let Some(existing) = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(admin_org.id))
        .filter(job_definitions::Column::Name.eq(&def_name))
        .one(&ctx.db)
        .await?
    {
        existing
    } else {
        job_definitions::ActiveModel {
            org_id: Set(admin_org.id),
            name: Set(def_name),
            job_type: Set("asm_scan".to_string()),
            config: Set(config.to_string()),
            enabled: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?
    };

    let run = job_runs::Model::create_queued(&ctx.db, definition.id, admin_org.id).await?;

    JobDispatchWorker::perform_later(
        &ctx,
        JobDispatchArgs {
            job_run_id: run.id,
            job_definition_id: definition.id,
        },
    )
    .await?;

    let run_pid = run.pid.to_string();
    let mut cookie = Cookie::new("free_scan_token", run_pid.clone());
    cookie.set_path(format!("/free-scan/{run_pid}"));
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(ctx.config.server.host.starts_with("https"));

    let mut response = Redirect::to(&format!("/free-scan/{run_pid}")).into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        cookie.to_string().parse().expect("cookie is valid ASCII"),
    );
    Ok(response)
}

/// `GET /free-scan/:run_pid` — show scan progress / results.
#[debug_handler]
pub async fn results_page(
    Path(run_pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let has_cookie = jar
        .get("free_scan_token")
        .is_some_and(|c| c.value() == run_pid);
    let is_admin = if let Some(user) = middleware::get_current_user(&jar, &ctx).await {
        let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
        org_ctx.is_some_and(|o| o.is_platform_admin)
    } else {
        false
    };

    if !has_cookie && !is_admin {
        return Err(Error::Unauthorized("Access denied".into()));
    }

    let run = job_runs::Model::find_by_pid(&ctx.db, &run_pid)
        .await
        .ok_or_else(|| Error::NotFound)?;

    let definition = job_definitions::Entity::find_by_id(run.job_definition_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let config: serde_json::Value = serde_json::from_str(&definition.config)
        .map_err(|_| Error::BadRequest("Invalid job config".into()))?;
    let domain = config["hostname"].as_str().unwrap_or("unknown").to_string();
    let org_id = definition.org_id;

    match run.status.as_str() {
        "completed" => {
            let asm_result = run
                .result_summary
                .as_deref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

            // Collect port scan results for this domain's resolved IPs
            let port_data = collect_port_scans(&ctx.db, org_id, &domain).await;

            // Keep refreshing if port scans are still running
            let status = if port_data.any_running {
                "port_scanning"
            } else {
                "completed"
            };

            views::free_scan::progress(
                &v,
                &domain,
                status,
                asm_result.as_ref(),
                None,
                &port_data.results,
            )
        }
        "failed" => views::free_scan::progress(
            &v,
            &domain,
            "failed",
            None,
            run.error_message.as_deref(),
            &[],
        ),
        _ => views::free_scan::progress(&v, &domain, &run.status, None, None, &[]),
    }
}

/// Collected port scan results for a domain.
struct PortScanData {
    results: Vec<serde_json::Value>,
    any_running: bool,
}

/// Find all port scan results for resolved IPs of a given domain.
/// Deduplicates by IP — if multiple subdomains resolve to the same IP,
/// shows one entry with all associated subdomains listed.
async fn collect_port_scans(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    domain: &str,
) -> PortScanData {
    let port_defs = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_id))
        .filter(job_definitions::Column::JobType.eq("port_scan"))
        .all(db)
        .await
        .unwrap_or_default();

    let mut ip_results: std::collections::HashMap<
        String,
        (Vec<String>, Vec<serde_json::Value>, u64),
    > = std::collections::HashMap::new();
    let mut any_running = false;

    for def in &port_defs {
        let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&def.config) else {
            continue;
        };
        let subdomain = cfg["subdomain"].as_str().unwrap_or("").to_string();
        let hostname = cfg["hostname"].as_str().unwrap_or("");

        if !subdomain.ends_with(domain) && subdomain != domain && hostname != domain {
            continue;
        }

        if is_job_pending(db, def.id, &mut any_running).await {
            continue;
        }

        collect_def_result(db, def.id, hostname, &subdomain, &mut ip_results).await;
    }

    let mut results: Vec<serde_json::Value> = ip_results
        .into_iter()
        .map(|(ip, (subdomains, ports, total_open))| {
            serde_json::json!({
                "ip": ip,
                "subdomains": subdomains.join(", "),
                "total_open": total_open,
                "ports": ports,
            })
        })
        .collect();

    // Sort by IP for stable display order
    results.sort_by(|a, b| {
        a["ip"]
            .as_str()
            .unwrap_or("")
            .cmp(b["ip"].as_str().unwrap_or(""))
    });

    PortScanData {
        results,
        any_running,
    }
}

/// Check if a job definition has a pending (queued/running) run.
/// Returns true if pending and not stuck (caller should skip).
/// Updates `any_running` flag.
async fn is_job_pending(
    db: &sea_orm::DatabaseConnection,
    def_id: i32,
    any_running: &mut bool,
) -> bool {
    let pending = job_runs::Entity::find()
        .filter(job_runs::Column::JobDefinitionId.eq(def_id))
        .filter(
            job_runs::Column::Status
                .eq("queued")
                .or(job_runs::Column::Status.eq("running")),
        )
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(ref p) = pending {
        let is_stuck = p.started_at.as_ref().is_some_and(|started| {
            let age = chrono::Utc::now() - started.with_timezone(&chrono::Utc);
            age.num_minutes() > 10
        });
        if !is_stuck {
            *any_running = true;
            return true;
        }
    }
    false
}

/// Collect completed or failed results for a job definition into the IP map.
async fn collect_def_result(
    db: &sea_orm::DatabaseConnection,
    def_id: i32,
    hostname: &str,
    subdomain: &str,
    ip_results: &mut std::collections::HashMap<String, (Vec<String>, Vec<serde_json::Value>, u64)>,
) {
    if let Some(run) = job_runs::Model::find_latest_completed_by_definition(db, def_id).await {
        if let Some(summary) = run
            .result_summary
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        {
            let ip = summary["ip"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(hostname)
                .to_string();
            if ip.is_empty() && summary["total_open"].as_u64().unwrap_or(0) == 0 {
                return;
            }
            let total_open = summary["total_open"].as_u64().unwrap_or(0);
            let ports = summary["ports"].as_array().cloned().unwrap_or_default();

            let entry = ip_results
                .entry(ip)
                .or_insert_with(|| (Vec::new(), Vec::new(), 0));
            if !subdomain.is_empty() && !entry.0.contains(&subdomain.to_string()) {
                entry.0.push(subdomain.to_string());
            }
            if ports.len() > entry.1.len() {
                entry.1 = ports;
                entry.2 = total_open;
            }
        }
    } else {
        // Show failed scans so the user sees the target was attempted
        let failed = job_runs::Entity::find()
            .filter(job_runs::Column::JobDefinitionId.eq(def_id))
            .filter(job_runs::Column::Status.eq("failed"))
            .one(db)
            .await
            .ok()
            .flatten();
        if failed.is_some() {
            let entry = ip_results
                .entry(hostname.to_string())
                .or_insert_with(|| (Vec::new(), Vec::new(), 0));
            if !subdomain.is_empty() && !entry.0.contains(&subdomain.to_string()) {
                entry.0.push(subdomain.to_string());
            }
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/free-scan", post(submit))
        .add("/free-scan/{run_pid}", get(results_page))
}
