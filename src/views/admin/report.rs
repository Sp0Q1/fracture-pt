use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, reports, users};

/// Render the admin report list (cross-org).
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    items: &[reports::Model],
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx, user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "admin/report/list.html", data!(ctx))
}
