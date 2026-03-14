use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, users};

/// Render the home page for an authenticated user.
pub fn index(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    scan_count: u64,
    engagement_count: u64,
    asm_summary: Option<&serde_json::Value>,
) -> Result<Response> {
    let mut ctx = super::base_context(user, org_ctx, user_orgs);
    ctx["scan_count"] = serde_json::json!(scan_count);
    ctx["engagement_count"] = serde_json::json!(engagement_count);
    if let Some(asm) = asm_summary {
        ctx["asm"] = asm.clone();
    }
    format::render().view(v, "home/index.html", data!(ctx))
}

/// Render the home page for a guest (unauthenticated) user.
pub fn index_guest(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "home/index.html", data!({}))
}
