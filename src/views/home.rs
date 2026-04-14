use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, users};

/// Bundled dashboard data to avoid too many function arguments.
pub struct DashboardData {
    pub target_count: u64,
    pub engagement_count: u64,
    pub findings_count: u64,
    pub report_count: u64,
    pub severity: serde_json::Value,
    pub asm_summary: Option<serde_json::Value>,
    pub active_engagements: Vec<serde_json::Value>,
    pub attack_surface: Vec<serde_json::Value>,
    pub recent_findings: Vec<serde_json::Value>,
}

/// Render the home page for an authenticated user.
pub fn index(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    data: &DashboardData,
) -> Result<Response> {
    let mut ctx = super::base_context(user, org_ctx, user_orgs);
    ctx["target_count"] = serde_json::json!(data.target_count);
    ctx["engagement_count"] = serde_json::json!(data.engagement_count);
    ctx["findings_count"] = serde_json::json!(data.findings_count);
    ctx["report_count"] = serde_json::json!(data.report_count);
    ctx["severity"] = data.severity.clone();
    if let Some(ref asm) = data.asm_summary {
        ctx["asm"] = asm.clone();
    }
    ctx["active_engagements"] = serde_json::json!(data.active_engagements);
    ctx["attack_surface"] = serde_json::json!(data.attack_surface);
    ctx["attack_surface_json"] = serde_json::json!(&data.attack_surface);
    ctx["recent_findings"] = serde_json::json!(data.recent_findings);
    format::render().view(v, "home/index.html", data!(ctx))
}

/// Render the home page for a guest (unauthenticated) user.
pub fn index_guest(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "home/index.html", data!({}))
}
