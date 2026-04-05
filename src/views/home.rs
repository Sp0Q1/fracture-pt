use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, users};

/// Render the home page for an authenticated user.
pub fn index(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    target_count: u64,
    engagement_count: u64,
    findings_count: u64,
    report_count: u64,
    severity: &serde_json::Value,
    asm_summary: Option<&serde_json::Value>,
    active_engagements: &[serde_json::Value],
) -> Result<Response> {
    let mut ctx = super::base_context(user, org_ctx, user_orgs);
    ctx["target_count"] = serde_json::json!(target_count);
    ctx["engagement_count"] = serde_json::json!(engagement_count);
    ctx["findings_count"] = serde_json::json!(findings_count);
    ctx["report_count"] = serde_json::json!(report_count);
    ctx["severity"] = severity.clone();
    if let Some(asm) = asm_summary {
        ctx["asm"] = asm.clone();
    }
    ctx["active_engagements"] = serde_json::json!(active_engagements);
    format::render().view(v, "home/index.html", data!(ctx))
}

/// Render the home page for a guest (unauthenticated) user.
pub fn index_guest(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "home/index.html", data!({}))
}
