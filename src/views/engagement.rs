use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{
    engagement_offers, engagements, findings, non_findings, organizations, services, users,
};

/// Render the engagement list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    items: &[engagements::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "engagement/list.html", data!(ctx))
}

/// Render the scope submission (new engagement request) form.
pub fn request_form(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    all_services: &[services::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
    ctx["services"] = serde_json::json!(all_services);
    format::render().view(v, "engagement/request.html", data!(ctx))
}

/// Render the engagement detail page (client view).
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    item: &engagements::Model,
    offers: &[engagement_offers::Model],
    engagement_findings: &[findings::Model],
    engagement_non_findings: &[non_findings::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
    ctx["item"] = serde_json::json!(item);
    ctx["offers"] = serde_json::json!(offers);
    ctx["findings"] = serde_json::json!(engagement_findings);
    ctx["non_findings"] = serde_json::json!(engagement_non_findings);
    format::render().view(v, "engagement/show.html", data!(ctx))
}
