use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{engagements, finding_comments, findings, organizations, users};
use crate::services::markdown;

/// Render the finding list with optional admin controls.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    items: &[findings::Model],
    editable_engagements: &[engagements::Model],
    is_admin: bool,
) -> Result<Response> {
    let mut ctx = super::base_context(user, Some(org_ctx), user_orgs);
    ctx["items"] = serde_json::json!(items);
    ctx["editable_engagements"] = serde_json::json!(editable_engagements
        .iter()
        .map(|e| serde_json::json!({"pid": e.pid.to_string(), "title": e.title}))
        .collect::<Vec<_>>());
    ctx["is_admin"] = serde_json::json!(is_admin);
    format::render().view(v, "finding/list.html", data!(ctx))
}

/// Render a single finding with markdown fields pre-rendered to HTML.
#[allow(clippy::too_many_arguments)] // View threads the finding + its comments/edit context.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    item: &findings::Model,
    is_admin: bool,
    engagement: Option<&engagements::Model>,
    can_edit: bool,
    comments: &[(finding_comments::Model, Option<users::Model>)],
) -> Result<Response> {
    let mut ctx = super::base_context(user, Some(org_ctx), user_orgs);
    ctx["item"] = serde_json::json!(item);
    ctx["is_admin"] = serde_json::json!(is_admin);
    ctx["description_html"] = serde_json::json!(markdown::render(&item.description));
    ctx["technical_description_html"] =
        serde_json::json!(item.technical_description.as_deref().map(markdown::render));
    ctx["impact_html"] = serde_json::json!(item.impact.as_deref().map(markdown::render));
    ctx["recommendation_html"] =
        serde_json::json!(item.recommendation.as_deref().map(markdown::render));
    ctx["evidence_html"] = serde_json::json!(item.evidence.as_deref().map(markdown::render));
    // Edit action (detail-page): links to the engagement finding-edit route.
    ctx["can_edit"] = serde_json::json!(can_edit);
    ctx["engagement_pid"] = serde_json::json!(engagement.map(|e| e.pid.to_string()));
    // Comments: markdown rendered server-side, paired with the author's name.
    ctx["comments"] = serde_json::json!(comments
        .iter()
        .map(|(c, author)| serde_json::json!({
            "content_html": markdown::render(&c.content),
            "author": author.as_ref().map_or("Unknown", |u| u.name.as_str()),
            "created_at": c.created_at.to_rfc3339(),
        }))
        .collect::<Vec<_>>());
    format::render().view(v, "finding/show.html", data!(ctx))
}
