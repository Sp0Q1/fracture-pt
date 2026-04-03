use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};

use crate::controllers::middleware;
use crate::models::_entities::{org_members, users};
use crate::models::organizations as org_model;
use crate::{require_user, views};

/// `GET /admin/users` -- list all users cross-org.
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

    let items = users::Entity::find()
        .order_by_desc(users::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .unwrap_or_default();

    // For each user, count their org memberships
    let mut user_org_counts: Vec<(users::Model, u64)> = Vec::new();
    for u in items {
        let count = org_members::Entity::find()
            .filter(org_members::Column::UserId.eq(u.id))
            .count(&ctx.db)
            .await
            .unwrap_or(0);
        user_org_counts.push((u, count));
    }

    views::admin::user::list(&v, &user, &org_ctx, &user_orgs, &user_org_counts)
}

/// `GET /admin/users/:pid` -- show user detail with org memberships.
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

    let target_user = crate::models::users::Model::find_by_pid(&ctx.db, &pid)
        .await
        .map_err(|_| Error::NotFound)?;

    // Get org memberships with org details
    let memberships = org_members::Entity::find()
        .filter(org_members::Column::UserId.eq(target_user.id))
        .all(&ctx.db)
        .await
        .unwrap_or_default();

    // Load org details for each membership
    let mut membership_details: Vec<serde_json::Value> = Vec::new();
    for m in &memberships {
        let org = crate::models::_entities::organizations::Entity::find_by_id(m.org_id)
            .one(&ctx.db)
            .await
            .ok()
            .flatten();
        membership_details.push(serde_json::json!({
            "org_id": m.org_id,
            "org_name": org.as_ref().map(|o| o.name.as_str()).unwrap_or("Unknown"),
            "org_slug": org.as_ref().map(|o| o.slug.as_str()).unwrap_or(""),
            "role": m.role,
            "joined_at": m.created_at,
        }));
    }

    views::admin::user::show(&v, &user, &org_ctx, &user_orgs, &target_user, &membership_details)
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![
        ("/users".to_string(), get(list)),
        ("/users/{pid}".to_string(), get(show)),
    ]
}
