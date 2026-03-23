use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{
    engagement_offers, engagements, findings, organizations, pentester_assignments, users,
};

/// Render the admin engagement list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    items: &[engagements::Model],
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx, user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "admin/engagement/list.html", data!(ctx))
}

/// Render the admin engagement detail.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    item: &engagements::Model,
    offers: &[engagement_offers::Model],
    assignments: &[pentester_assignments::Model],
    engagement_findings: &[findings::Model],
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx, user_orgs);
    ctx["item"] = serde_json::json!(item);
    ctx["offers"] = serde_json::json!(offers);
    ctx["assignments"] = serde_json::json!(assignments);
    ctx["findings"] = serde_json::json!(engagement_findings);
    format::render().view(v, "admin/engagement/show.html", data!(ctx))
}
