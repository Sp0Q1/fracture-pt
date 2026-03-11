use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{organizations, users};

/// Render the home page for an authenticated user.
pub fn index(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
) -> Result<Response> {
    let ctx = super::base_context(user, org_ctx, user_orgs);
    format::render().view(v, "home/index.html", data!(ctx))
}

/// Render the home page for a guest (unauthenticated) user.
pub fn index_guest(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "home/index.html", data!({}))
}
