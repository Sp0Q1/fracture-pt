use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{findings, organizations, users};
use crate::services::markdown;

/// Render the admin finding list with org names.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    items: &[serde_json::Value],
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx, user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "admin/finding/list.html", data!(ctx))
}

/// Render the admin finding detail with org name and markdown.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    item: &findings::Model,
    org_name: &str,
    engagement_title: Option<&str>,
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx, user_orgs);
    ctx["item"] = serde_json::json!(item);
    ctx["org_name"] = serde_json::json!(org_name);
    ctx["engagement_title"] = serde_json::json!(engagement_title);
    ctx["description_html"] = serde_json::json!(markdown::render(&item.description));
    ctx["technical_description_html"] =
        serde_json::json!(item.technical_description.as_deref().map(markdown::render));
    ctx["impact_html"] = serde_json::json!(item.impact.as_deref().map(markdown::render));
    ctx["recommendation_html"] =
        serde_json::json!(item.recommendation.as_deref().map(markdown::render));
    ctx["evidence_html"] = serde_json::json!(item.evidence.as_deref().map(markdown::render));
    format::render().view(v, "admin/finding/show.html", data!(ctx))
}
