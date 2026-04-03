use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;

use crate::controllers::middleware;
use crate::models::findings;
use crate::models::organizations as org_model;
use crate::{require_user, views};

/// `GET /admin/findings` -- list all findings cross-org.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let items = findings::Model::find_all(&ctx.db).await;
    views::admin::finding::list(&v, &user, &org_ctx, &user_orgs, &items)
}

/// `GET /admin/findings/:pid` -- show finding detail (admin view).
#[debug_handler]
pub async fn show(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let item = findings::Model::find_by_pid(&ctx.db, &pid)
        .await
        .ok_or_else(|| Error::NotFound)?;
    views::admin::finding::show(&v, &user, &org_ctx, &user_orgs, &item)
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![
        ("/findings".to_string(), get(list)),
        ("/findings/{pid}".to_string(), get(show)),
    ]
}
