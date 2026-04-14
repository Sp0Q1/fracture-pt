use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, users};

/// Render the admin subscription list (cross-org).
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    items: &[serde_json::Value],
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx.as_ref(), user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "admin/subscription/list.html", data!(ctx))
}
