use axum::response::Redirect;
use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder};

use crate::controllers::middleware;
use crate::models::_entities::{engagement_comments, org_members, pentester_assignments, users};
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
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await?;

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
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await?;

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
            "org_name": org.as_ref().map_or("Unknown", |o| o.name.as_str()),
            "org_slug": org.as_ref().map_or("", |o| o.slug.as_str()),
            "role": m.role,
            "joined_at": m.created_at,
        }));
    }

    views::admin::user::show(
        &v,
        &user,
        &org_ctx,
        &user_orgs,
        &target_user,
        &membership_details,
    )
}

/// `POST /admin/users/:pid/delete` -- delete a user's local record.
///
/// Cascades: org memberships, pentester assignments, comments.
/// Does NOT delete the IdP account — the user could re-register on next login.
#[debug_handler]
pub async fn delete(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);

    let target_user = crate::models::users::Model::find_by_pid(&ctx.db, &pid)
        .await
        .map_err(|_| Error::NotFound)?;

    // Cannot delete yourself
    if target_user.id == user.id {
        return Err(Error::BadRequest("Cannot delete your own account".into()));
    }

    // Preserve comments but mark as from deleted user:
    // Prepend original author name to content, then reassign to the admin
    // performing the deletion so the FK stays valid.
    let user_comments = engagement_comments::Entity::find()
        .filter(engagement_comments::Column::UserId.eq(target_user.id))
        .all(&ctx.db)
        .await
        .unwrap_or_default();
    for comment in user_comments {
        let prefixed = format!(
            "*Originally posted by {}:*\n\n{}",
            target_user.name, comment.content
        );
        let mut active: engagement_comments::ActiveModel = comment.into();
        active.content = sea_orm::ActiveValue::Set(prefixed);
        active.user_id = sea_orm::ActiveValue::Set(user.id);
        active.update(&ctx.db).await?;
    }

    // Delete pentester assignments
    pentester_assignments::Entity::delete_many()
        .filter(pentester_assignments::Column::UserId.eq(target_user.id))
        .exec(&ctx.db)
        .await?;

    // Delete org memberships
    org_members::Entity::delete_many()
        .filter(org_members::Column::UserId.eq(target_user.id))
        .exec(&ctx.db)
        .await?;

    // Delete the user record
    target_user.delete(&ctx.db).await?;

    Ok(Redirect::to("/admin/users").into_response())
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![
        ("/users".to_string(), get(list)),
        ("/users/{pid}".to_string(), get(show)),
        ("/users/{pid}/delete".to_string(), post(delete)),
    ]
}
