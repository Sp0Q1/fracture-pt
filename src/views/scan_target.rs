use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, scan_targets, users};

/// Render the scan target list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    items: &[scan_targets::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "scan_target/list.html", data!(ctx))
}

/// Render the new target form.
pub fn create(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
) -> Result<Response> {
    let ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
    format::render().view(v, "scan_target/create.html", data!(ctx))
}

/// Render a target detail page.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    item: &scan_targets::Model,
) -> Result<Response> {
    let mut ctx = super::base_context(user, &Some(org_ctx.clone()), user_orgs);
    ctx["item"] = serde_json::json!(item);
    format::render().view(v, "scan_target/show.html", data!(ctx))
}
