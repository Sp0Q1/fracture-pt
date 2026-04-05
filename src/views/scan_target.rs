use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, scan_targets, users};

/// Bundled scan state for the target detail view.
pub struct ScanState<'a> {
    pub asm_result: Option<&'a serde_json::Value>,
    pub status: Option<&'a str>,
    pub error: Option<&'a str>,
    pub port_scan_result: Option<&'a serde_json::Value>,
    pub port_scan_status: Option<&'a str>,
    pub port_scan_error: Option<&'a str>,
}

/// Render the scan target list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    items: &[scan_targets::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
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
    let ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
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
    let mut ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
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
    if let Some(port_scan) = scan.port_scan_result {
        ctx["port_scan"] = port_scan.clone();
    }
    ctx["port_scan_status"] = serde_json::json!(scan.port_scan_status.unwrap_or(""));
    ctx["port_scan_error"] = serde_json::json!(scan.port_scan_error.unwrap_or(""));
    format::render().view(v, "scan_target/show.html", data!(ctx))
}
