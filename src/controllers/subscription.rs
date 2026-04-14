use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;

use super::middleware;
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::models::subscriptions;
use crate::{require_role, require_user, views};

/// `GET /subscriptions` -- list org subscriptions.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Viewer);
    let items = subscriptions::Model::find_by_org(&ctx.db, org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await?;
    views::subscription::list(&v, &user, &org_ctx, &user_orgs, &items)
}

pub fn routes() -> Routes {
    Routes::new().prefix("/subscriptions").add("/", get(list))
}
