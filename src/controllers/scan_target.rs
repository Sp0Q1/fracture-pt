use axum::response::Redirect;
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::middleware;
use crate::models::_entities::{job_definitions, job_runs, scan_targets::ActiveModel};
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::models::scan_targets;
use crate::services::tier::PlanTier;
use crate::workers::job_dispatcher::{JobDispatchArgs, JobDispatchWorker};
use crate::{require_role, require_user, views};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Params {
    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub target_type: String,
    pub label: Option<String>,
}

/// `GET /targets` -- list scan targets.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Viewer);
    let items = scan_targets::Model::find_by_org(&ctx.db, org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await;
    views::scan_target::list(&v, &user, &org_ctx, &user_orgs, &items)
}

/// `GET /targets/new` -- new target form.
#[debug_handler]
pub async fn new(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await;
    views::scan_target::create(&v, &user, &org_ctx, &user_orgs)
}

/// `POST /targets` -- add target.
#[debug_handler]
pub async fn add(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<Params>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);

    // Check tier target limits
    let tier = PlanTier::from_org(&org_ctx.org);
    if let Some(max) = tier.max_targets() {
        #[allow(clippy::cast_possible_truncation)]
        let existing_count = scan_targets::Entity::find()
            .filter(scan_targets::Column::OrgId.eq(org_ctx.org.id))
            .count(&ctx.db)
            .await
            .unwrap_or(0) as usize;
        if existing_count >= max {
            return Err(Error::BadRequest(format!(
                "Your {} plan allows up to {} target{}. Upgrade to add more.",
                tier.label(),
                max,
                if max == 1 { "" } else { "s" }
            )));
        }
    }

    // Validate hostname if provided
    if let Some(ref hostname) = params.hostname {
        validate_hostname(hostname)?;
    }

    // Validate IP address if provided
    if let Some(ref ip) = params.ip_address {
        validate_ip_address(ip)?;
    }

    // At least one of hostname or ip_address must be provided
    if params.hostname.is_none() && params.ip_address.is_none() {
        return Err(Error::BadRequest(
            "Either hostname or IP address must be provided".into(),
        ));
    }

    #[allow(clippy::default_trait_access)]
    let mut item: ActiveModel = Default::default();
    item.org_id = Set(org_ctx.org.id);
    item.hostname = Set(params.hostname.clone());
    item.ip_address = Set(params.ip_address);
    item.target_type = Set(params.target_type);
    item.label = Set(params.label);
    let target = item.insert(&ctx.db).await?;

    // If the target has a hostname, enqueue an ASM scan via the jobs system
    if let Some(ref hostname) = params.hostname {
        let hostname = hostname.trim().to_lowercase();
        if !hostname.is_empty() {
            let def_name = format!("ASM: {hostname}");

            // Find existing definition or create a new one
            let definition = if let Some(existing) = job_definitions::Entity::find()
                .filter(job_definitions::Column::OrgId.eq(org_ctx.org.id))
                .filter(job_definitions::Column::Name.eq(&def_name))
                .one(&ctx.db)
                .await?
            {
                existing
            } else {
                let config = serde_json::json!({
                    "target_id": target.id,
                    "hostname": hostname,
                    "org_id": org_ctx.org.id,
                });
                job_definitions::ActiveModel {
                    org_id: Set(org_ctx.org.id),
                    name: Set(def_name),
                    job_type: Set("asm_scan".to_string()),
                    config: Set(config.to_string()),
                    enabled: Set(true),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await?
            };

            let run =
                job_runs::Model::create_queued(&ctx.db, definition.id, org_ctx.org.id).await?;

            JobDispatchWorker::perform_later(
                &ctx,
                JobDispatchArgs {
                    job_run_id: run.id,
                    job_definition_id: definition.id,
                },
            )
            .await?;
        }
    }

    Ok(Redirect::to(&format!("/targets/{}", target.pid)).into_response())
}

/// Status of a job-based scan lookup.
struct JobScanStatus {
    result: Option<serde_json::Value>,
    status: Option<String>,
    error: Option<String>,
}

/// Looks up the latest run status for a given job definition.
async fn lookup_job_run_status(
    db: &sea_orm::DatabaseConnection,
    def: &job_definitions::Model,
) -> JobScanStatus {
    // Check for pending (queued/running) run first
    let pending = job_runs::Entity::find()
        .filter(job_runs::Column::JobDefinitionId.eq(def.id))
        .filter(
            job_runs::Column::Status
                .eq("queued")
                .or(job_runs::Column::Status.eq("running")),
        )
        .one(db)
        .await
        .ok()
        .flatten();

    if pending.is_some() {
        return JobScanStatus {
            result: None,
            status: Some("running".to_string()),
            error: None,
        };
    }

    // Check for latest completed run
    let completed = job_runs::Model::find_latest_completed_by_definition(db, def.id).await;
    if let Some(run) = completed {
        let result = run
            .result_summary
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
        return JobScanStatus {
            result,
            status: Some("completed".to_string()),
            error: None,
        };
    }

    // Check for failed run
    let failed = job_runs::Entity::find()
        .filter(job_runs::Column::JobDefinitionId.eq(def.id))
        .filter(job_runs::Column::Status.eq("failed"))
        .order_by_desc(job_runs::Column::CompletedAt)
        .one(db)
        .await
        .ok()
        .flatten();
    if let Some(run) = failed {
        return JobScanStatus {
            result: None,
            status: Some("failed".to_string()),
            error: run.error_message,
        };
    }

    JobScanStatus {
        result: None,
        status: None,
        error: None,
    }
}

/// Looks up ASM scan status for a target by hostname.
async fn lookup_asm_status(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    hostname: &str,
) -> JobScanStatus {
    let asm_defs = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_id))
        .filter(job_definitions::Column::JobType.eq("asm_scan"))
        .all(db)
        .await
        .unwrap_or_default();

    let matching_def = asm_defs.into_iter().find(|d| {
        serde_json::from_str::<serde_json::Value>(&d.config)
            .ok()
            .is_some_and(|v| v["hostname"].as_str().is_some_and(|h| h == hostname))
    });

    match matching_def {
        Some(def) => lookup_job_run_status(db, &def).await,
        None => JobScanStatus {
            result: None,
            status: None,
            error: None,
        },
    }
}

/// Looks up port scan status for a target by target ID.
async fn lookup_port_scan_status(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    target_id: i32,
) -> JobScanStatus {
    let port_defs = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_id))
        .filter(job_definitions::Column::JobType.eq("port_scan"))
        .all(db)
        .await
        .unwrap_or_default();

    let matching_def = port_defs.into_iter().find(|d| {
        serde_json::from_str::<serde_json::Value>(&d.config)
            .ok()
            .is_some_and(|v| {
                v["target_id"]
                    .as_i64()
                    .is_some_and(|id| id == i64::from(target_id))
            })
    });

    match matching_def {
        Some(def) => lookup_job_run_status(db, &def).await,
        None => JobScanStatus {
            result: None,
            status: None,
            error: None,
        },
    }
}

/// `GET /targets/:pid` -- target detail + verification status.
#[debug_handler]
pub async fn show(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Viewer);
    let item = scan_targets::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await;

    let asm = match item.hostname.as_deref() {
        Some(hostname) => lookup_asm_status(&ctx.db, org_ctx.org.id, hostname).await,
        None => JobScanStatus {
            result: None,
            status: None,
            error: None,
        },
    };

    let has_target = item.hostname.is_some() || item.ip_address.is_some();
    let port = if has_target {
        lookup_port_scan_status(&ctx.db, org_ctx.org.id, item.id).await
    } else {
        JobScanStatus {
            result: None,
            status: None,
            error: None,
        }
    };

    // Determine current scan schedule (from any matching job definition)
    let hostname = item
        .hostname
        .as_deref()
        .or(item.ip_address.as_deref())
        .unwrap_or("");
    let current_schedule = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_ctx.org.id))
        .all(&ctx.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .find_map(|def| {
            let cfg = serde_json::from_str::<serde_json::Value>(&def.config).ok()?;
            let matches = cfg["hostname"].as_str() == Some(hostname)
                || cfg["target_id"]
                    .as_i64()
                    .is_some_and(|id| id == i64::from(item.id));
            if matches {
                def.schedule
            } else {
                None
            }
        })
        .unwrap_or_default();

    let schedule_label = match current_schedule.as_str() {
        s if s.contains("* * *") && s.starts_with("0 3") => "daily",
        s if s.contains("* * 1") => "weekly",
        s if s.contains("1 * *") => "monthly",
        "" => "off",
        _ => "custom",
    };

    let tier = PlanTier::from_org(&org_ctx.org);

    let scan = views::scan_target::ScanState {
        asm_result: asm.result.as_ref(),
        status: asm.status.as_deref(),
        error: asm.error.as_deref(),
        port_scan_result: port.result.as_ref(),
        port_scan_status: port.status.as_deref(),
        port_scan_error: port.error.as_deref(),
        schedule: schedule_label,
        scheduling_enabled: tier.scheduling_enabled(),
        is_free_tier: matches!(tier, PlanTier::Free),
    };
    views::scan_target::show(&v, &user, &org_ctx, &user_orgs, &item, &scan)
}

/// `DELETE /targets/:pid` -- remove target.
#[debug_handler]
pub async fn remove(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);
    let item = scan_targets::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    item.delete(&ctx.db).await?;
    format::empty()
}

/// Validates a hostname per RFC 1123 and rejects reserved/dangerous domains.
fn validate_hostname(hostname: &str) -> Result<()> {
    let hostname = hostname.trim().to_lowercase();
    if hostname.is_empty() || hostname.len() > 253 {
        return Err(Error::BadRequest("Invalid hostname length".into()));
    }
    // Only allow safe characters
    if !hostname
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err(Error::BadRequest(
            "Hostname contains invalid characters".into(),
        ));
    }
    // Check label lengths
    for label in hostname.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(Error::BadRequest("Invalid hostname label length".into()));
        }
    }
    // Reject reserved domains
    let reserved = ["localhost", "localhost.localdomain", "broadcasthost"];
    if reserved.contains(&hostname.as_str()) {
        return Err(Error::BadRequest("Reserved hostname".into()));
    }
    let reserved_suffixes = [
        ".local",
        ".internal",
        ".test",
        ".example",
        ".invalid",
        ".onion",
    ];
    for suffix in &reserved_suffixes {
        if hostname.ends_with(suffix) {
            return Err(Error::BadRequest("Reserved hostname suffix".into()));
        }
    }
    Ok(())
}

/// Validates an IP address and rejects private/reserved ranges.
fn validate_ip_address(ip_str: &str) -> Result<()> {
    let ip_str = ip_str.trim();
    let addr: IpAddr = ip_str
        .parse()
        .map_err(|_| Error::BadRequest("Invalid IP address format".into()))?;
    match addr {
        IpAddr::V4(ipv4) => {
            if ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_unspecified()
                || ipv4.octets()[0] == 0 // 0.0.0.0/8
                || ipv4.octets() == [169, 254, 169, 254]
            // cloud metadata
            {
                return Err(Error::BadRequest(
                    "IP address is in a reserved or private range".into(),
                ));
            }
            // Carrier-grade NAT 100.64.0.0/10
            if ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xC0) == 64 {
                return Err(Error::BadRequest(
                    "IP address is in a reserved range (CGNAT)".into(),
                ));
            }
        }
        IpAddr::V6(ipv6) => {
            if ipv6.is_loopback() || ipv6.is_unspecified() {
                return Err(Error::BadRequest(
                    "IP address is in a reserved range".into(),
                ));
            }
            // Unique local fc00::/7
            let segments = ipv6.segments();
            if (segments[0] & 0xFE00) == 0xFC00 {
                return Err(Error::BadRequest(
                    "IP address is in a private range (ULA)".into(),
                ));
            }
            // Link-local fe80::/10
            if (segments[0] & 0xFFC0) == 0xFE80 {
                return Err(Error::BadRequest(
                    "IP address is in a link-local range".into(),
                ));
            }
        }
    }
    Ok(())
}

/// `POST /targets/:pid/port-scan` -- trigger a port scan.
#[debug_handler]
pub async fn trigger_port_scan(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);
    let item = scan_targets::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    // Free tier: only one completed scan allowed (the initial auto-scan)
    let tier = PlanTier::from_org(&org_ctx.org);
    if matches!(tier, PlanTier::Free) {
        let completed_count = job_runs::Entity::find()
            .filter(job_runs::Column::OrgId.eq(org_ctx.org.id))
            .filter(job_runs::Column::Status.eq("completed"))
            .count(&ctx.db)
            .await
            .unwrap_or(0);
        if completed_count > 0 {
            return Err(Error::BadRequest(
                "Free plan includes one scan. Upgrade to Pro for recurring scans.".into(),
            ));
        }
    }

    let scan_target_name = item
        .hostname
        .as_deref()
        .or(item.ip_address.as_deref())
        .ok_or_else(|| Error::BadRequest("Target has no hostname or IP".into()))?
        .to_string();

    let def_name = format!("Port Scan: {scan_target_name}");

    // Find existing definition or create a new one
    let definition = if let Some(existing) = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_ctx.org.id))
        .filter(job_definitions::Column::Name.eq(&def_name))
        .one(&ctx.db)
        .await?
    {
        existing
    } else {
        let config = serde_json::json!({
            "target_id": item.id,
            "hostname": item.hostname,
            "ip_address": item.ip_address,
            "org_id": org_ctx.org.id,
        });
        job_definitions::ActiveModel {
            org_id: Set(org_ctx.org.id),
            name: Set(def_name),
            job_type: Set("port_scan".to_string()),
            config: Set(config.to_string()),
            enabled: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?
    };

    let run = job_runs::Model::create_queued(&ctx.db, definition.id, org_ctx.org.id).await?;

    JobDispatchWorker::perform_later(
        &ctx,
        JobDispatchArgs {
            job_run_id: run.id,
            job_definition_id: definition.id,
        },
    )
    .await?;

    Ok(Redirect::to(&format!("/targets/{pid}")).into_response())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduleParams {
    pub schedule: String, // "off", "daily", "weekly", "monthly"
}

/// `POST /targets/:pid/schedule` — set scan schedule for a target.
#[debug_handler]
pub async fn set_schedule(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<ScheduleParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);

    let tier = PlanTier::from_org(&org_ctx.org);
    if !tier.scheduling_enabled() {
        return Err(Error::BadRequest(
            "Scheduled scans require a Pro or Enterprise plan.".into(),
        ));
    }

    let item = scan_targets::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    let hostname = item
        .hostname
        .as_deref()
        .or(item.ip_address.as_deref())
        .unwrap_or("unknown");

    // Find ASM and port scan definitions for this target
    let defs = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_ctx.org.id))
        .all(&ctx.db)
        .await
        .unwrap_or_default();

    let schedule_value = match params.schedule.as_str() {
        "daily" => Some("0 3 * * *".to_string()),   // 3 AM daily
        "weekly" => Some("0 3 * * 1".to_string()),  // 3 AM Monday
        "monthly" => Some("0 3 1 * *".to_string()), // 3 AM 1st of month
        _ => None,                                  // "off"
    };

    // Update all job definitions that match this target
    for def in &defs {
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&def.config) {
            let matches = cfg["hostname"].as_str() == Some(hostname)
                || cfg["target_id"]
                    .as_i64()
                    .is_some_and(|id| id == i64::from(item.id));
            if matches {
                let mut active: job_definitions::ActiveModel = def.clone().into();
                active.schedule = sea_orm::ActiveValue::Set(schedule_value.clone());
                active.update(&ctx.db).await?;
            }
        }
    }

    Ok(Redirect::to(&format!("/targets/{pid}")).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/targets")
        .add("/", get(list))
        .add("/", post(add))
        .add("/new", get(new))
        .add("/{pid}", get(show))
        .add("/{pid}", delete(remove))
        .add("/{pid}/port-scan", post(trigger_port_scan))
        .add("/{pid}/schedule", post(set_schedule))
}
