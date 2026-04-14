use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, scan_targets, users};

/// Bundled scan state for the target detail view.
pub struct ScanState<'a> {
    pub asm_result: Option<&'a serde_json::Value>,
    pub status: Option<&'a str>,
    pub error: Option<&'a str>,
    pub port_scans: &'a [serde_json::Value],
    pub port_scans_running: bool,
    pub schedule: &'a str,
    pub scheduling_enabled: bool,
    pub is_free_tier: bool,
}

/// Render the scan target list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    items: &[scan_targets::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, Some(org_ctx), user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "scan_target/list.html", data!(ctx))
}

/// Render the new target form.
pub fn create(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
) -> Result<Response> {
    let ctx = super::base_context(user, Some(org_ctx), user_orgs);
    format::render().view(v, "scan_target/create.html", data!(ctx))
}

/// Render a target detail page.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    item: &scan_targets::Model,
    scan: &ScanState<'_>,
) -> Result<Response> {
    let mut ctx = super::base_context(user, Some(org_ctx), user_orgs);
    ctx["item"] = serde_json::json!(item);
    if let Some(asm) = scan.asm_result {
        let mut asm_ctx = asm.clone();
        if let Some(subs) = asm.get("subdomains").and_then(|s| s.as_array()) {
            asm_ctx["subdomains"] = serde_json::json!(super::flatten_subdomains(subs));
        }
        ctx["asm"] = asm_ctx;
    }
    ctx["scan_status"] = serde_json::json!(scan.status.unwrap_or(""));
    ctx["scan_error"] = serde_json::json!(scan.error.unwrap_or(""));
    ctx["port_scans"] = serde_json::json!(scan.port_scans);
    ctx["port_scan_status"] = serde_json::json!(if scan.port_scans_running {
        "running"
    } else if scan.port_scans.is_empty() {
        ""
    } else {
        "completed"
    });
    ctx["schedule"] = serde_json::json!(scan.schedule);
    ctx["scheduling_enabled"] = serde_json::json!(scan.scheduling_enabled);
    ctx["is_free_tier"] = serde_json::json!(scan.is_free_tier);
    format::render().view(v, "scan_target/show.html", data!(ctx))
}
