use axum::response::Redirect;
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::middleware;
use crate::models::_entities::{scan_jobs, scan_targets::ActiveModel};
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::models::scan_targets;
use crate::models::services;
use crate::workers::asm_scan::{AsmScanArgs, AsmScanWorker};
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
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
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
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
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

    // If the target has a hostname, enqueue an ASM scan
    if let Some(ref hostname) = params.hostname {
        let hostname = hostname.trim().to_lowercase();
        if !hostname.is_empty() {
            if let Some(asm_service) =
                services::Model::find_by_slug(&ctx.db, "attack-surface-mapping").await
            {
                // Create a scan job row
                let job = scan_jobs::ActiveModel {
                    org_id: Set(org_ctx.org.id),
                    target_id: Set(target.id),
                    service_id: Set(asm_service.id),
                    status: Set("queued".to_string()),
                    ..Default::default()
                };
                let job = job.insert(&ctx.db).await?;

                // Enqueue the background worker
                AsmScanWorker::perform_later(
                    &ctx,
                    AsmScanArgs {
                        target_id: target.id,
                        org_id: org_ctx.org.id,
                        hostname,
                        job_id: job.id,
                    },
                )
                .await?;
            }
        }
    }

    Ok(Redirect::to("/targets").into_response())
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
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;

    // Get the latest completed ASM scan job for this target
    let latest_job = scan_jobs::Entity::find()
        .filter(scan_jobs::Column::TargetId.eq(item.id))
        .filter(scan_jobs::Column::OrgId.eq(org_ctx.org.id))
        .filter(scan_jobs::Column::Status.eq("completed"))
        .filter(scan_jobs::Column::ResultSummary.is_not_null())
        .order_by_desc(scan_jobs::Column::CompletedAt)
        .one(&ctx.db)
        .await
        .ok()
        .flatten();

    let asm_result = latest_job.and_then(|job| {
        job.result_summary
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
    });

    // Also check if there's a running/queued scan
    let pending_job = scan_jobs::Entity::find()
        .filter(scan_jobs::Column::TargetId.eq(item.id))
        .filter(scan_jobs::Column::OrgId.eq(org_ctx.org.id))
        .filter(
            scan_jobs::Column::Status
                .eq("queued")
                .or(scan_jobs::Column::Status.eq("running")),
        )
        .one(&ctx.db)
        .await
        .ok()
        .flatten();

    // Check for latest failed job if no completed result
    let failed_job = if asm_result.is_none() {
        scan_jobs::Entity::find()
            .filter(scan_jobs::Column::TargetId.eq(item.id))
            .filter(scan_jobs::Column::OrgId.eq(org_ctx.org.id))
            .filter(scan_jobs::Column::Status.eq("failed"))
            .order_by_desc(scan_jobs::Column::CompletedAt)
            .one(&ctx.db)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let failed_message = failed_job.as_ref().and_then(|j| j.error_message.clone());

    let scan_status = if pending_job.is_some() {
        Some("running")
    } else if asm_result.is_some() {
        Some("completed")
    } else if failed_job.is_some() {
        Some("failed")
    } else {
        None
    };

    let scan = views::scan_target::ScanState {
        asm_result: asm_result.as_ref(),
        status: scan_status,
        error: failed_message.as_deref(),
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/targets")
        .add("/", get(list))
        .add("/", post(add))
        .add("/new", get(new))
        .add("/{pid}", get(show))
        .add("/{pid}", delete(remove))
}
