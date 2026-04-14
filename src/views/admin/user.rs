use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, users};

/// Render the admin user list (cross-org).
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    items: &[(users::Model, u64)],
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx.as_ref(), user_orgs);
    let items_json: Vec<serde_json::Value> = items
        .iter()
        .map(|(u, org_count)| {
            serde_json::json!({
                "pid": u.pid,
                "name": u.name,
                "email": u.email,
                "org_count": org_count,
                "created_at": u.created_at,
            })
        })
        .collect();
    ctx["items"] = serde_json::json!(items_json);
    format::render().view(v, "admin/user/list.html", data!(ctx))
}

/// Render the admin user detail.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    target_user: &users::Model,
    memberships: &[serde_json::Value],
) -> Result<Response> {
    let mut ctx = crate::views::base_context(user, org_ctx.as_ref(), user_orgs);
    ctx["target_user"] = serde_json::json!(target_user);
    ctx["memberships"] = serde_json::json!(memberships);
    format::render().view(v, "admin/user/show.html", data!(ctx))
}
