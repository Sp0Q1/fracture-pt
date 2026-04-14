use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, subscriptions, users};

/// Render the subscription list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    items: &[subscriptions::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, Some(org_ctx), user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "subscription/list.html", data!(ctx))
}
