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

pub struct ShowViewData<'a> {
    pub item: &'a engagements::Model,
    pub offers: &'a [engagement_offers::Model],
    pub assignments: &'a [pentester_assignments::Model],
    pub findings: &'a [findings::Model],
}

/// Render the admin engagement detail.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    data: &ShowViewData<'_>,
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx, user_orgs);
    ctx["item"] = serde_json::json!(data.item);
    ctx["offers"] = serde_json::json!(data.offers);
    ctx["assignments"] = serde_json::json!(data.assignments);
    ctx["findings"] = serde_json::json!(data.findings);
    format::render().view(v, "admin/engagement/show.html", data!(ctx))
}
