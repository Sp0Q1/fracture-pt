use axum::response::Redirect;
use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use serde::Deserialize;

use super::middleware;
use crate::models::_entities::{engagements, pentester_assignments};
use crate::models::findings;
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::{require_role, require_user, views};

/// `GET /findings` -- list all findings for org.
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
    let items = findings::Model::find_by_org(&ctx.db, org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await;

    // For admins/pentesters: provide in-progress engagements they can add findings to
    let is_admin =
        fracture_core::models::organizations::Model::is_user_platform_admin(&ctx.db, user.id).await;
    let editable_engagements = if is_admin {
        // Admins can add findings to any engagement in the org
        engagements::Entity::find()
            .filter(engagements::Column::OrgId.eq(org_ctx.org.id))
            .all(&ctx.db)
            .await
            .unwrap_or_default()
    } else {
        // Check if user is a pentester with assignments in this org
        let assigned = pentester_assignments::Entity::find()
            .filter(pentester_assignments::Column::UserId.eq(user.id))
            .all(&ctx.db)
            .await
            .unwrap_or_default();
        let eng_ids: Vec<i32> = assigned.iter().map(|a| a.engagement_id).collect();
        if eng_ids.is_empty() {
            Vec::new()
        } else {
            engagements::Entity::find()
                .filter(engagements::Column::Id.is_in(eng_ids))
                .filter(engagements::Column::OrgId.eq(org_ctx.org.id))
                .filter(engagements::Column::Status.eq("in_progress"))
                .all(&ctx.db)
                .await
                .unwrap_or_default()
        }
    };

    views::finding::list(
        &v,
        &user,
        &org_ctx,
        &user_orgs,
        &items,
        &editable_engagements,
        is_admin,
    )
}

/// `GET /findings/:pid` -- show a single finding.
#[debug_handler]
pub async fn show(
    Path(pid): Path<String>,
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
    let item = findings::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await;
    let is_admin =
        fracture_core::models::organizations::Model::is_user_platform_admin(&ctx.db, user.id).await;
    views::finding::show(&v, &user, &org_ctx, &user_orgs, &item, is_admin)
}

/// `POST /findings/:pid/delete` -- delete a single finding (admin only).
#[debug_handler]
pub async fn delete(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let is_admin =
        fracture_core::models::organizations::Model::is_user_platform_admin(&ctx.db, user.id).await;
    if !is_admin {
        return Err(Error::NotFound);
    }
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let item = findings::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    item.delete(&ctx.db).await?;
    Ok(Redirect::to("/findings").into_response())
}

#[derive(Debug, Deserialize)]
pub struct BulkDeleteParams {
    pub pids: Vec<String>,
}

/// `POST /findings/bulk-delete` -- delete multiple findings (admin only).
#[debug_handler]
pub async fn bulk_delete(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<BulkDeleteParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let is_admin =
        fracture_core::models::organizations::Model::is_user_platform_admin(&ctx.db, user.id).await;
    if !is_admin {
        return Err(Error::NotFound);
    }
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;

    for pid in &params.pids {
        if let Some(item) = findings::Model::find_by_pid_and_org(&ctx.db, pid, org_ctx.org.id).await
        {
            let _ = item.delete(&ctx.db).await;
        }
    }
    Ok(Redirect::to("/findings").into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/findings")
        .add("/", get(list))
        .add("/bulk-delete", post(bulk_delete))
        .add("/{pid}", get(show))
        .add("/{pid}/delete", post(delete))
}
