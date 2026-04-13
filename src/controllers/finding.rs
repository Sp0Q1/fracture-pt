use axum::response::Redirect;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use serde::Deserialize;

use super::auth::{OrgAuth, PlatformAdmin, ViewerRole};
use crate::models::_entities::{engagements, pentester_assignments};
use crate::models::findings;
use crate::models::organizations as org_model;
use crate::views;

/// `GET /findings` -- list all findings for org.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    auth: OrgAuth<ViewerRole>,
) -> Result<Response> {
    let items = findings::Model::find_by_org(&ctx.db, auth.org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, auth.user.id).await;

    // For admins/pentesters: provide in-progress engagements they can add findings to
    let is_admin = auth.is_platform_admin();
    let editable_engagements = if is_admin {
        engagements::Entity::find()
            .filter(engagements::Column::OrgId.eq(auth.org_ctx.org.id))
            .all(&ctx.db)
            .await
            .unwrap_or_default()
    } else {
        let assigned = pentester_assignments::Entity::find()
            .filter(pentester_assignments::Column::UserId.eq(auth.user.id))
            .all(&ctx.db)
            .await
            .unwrap_or_default();
        let eng_ids: Vec<i32> = assigned.iter().map(|a| a.engagement_id).collect();
        if eng_ids.is_empty() {
            Vec::new()
        } else {
            engagements::Entity::find()
                .filter(engagements::Column::Id.is_in(eng_ids))
                .filter(engagements::Column::OrgId.eq(auth.org_ctx.org.id))
                .filter(engagements::Column::Status.eq("in_progress"))
                .all(&ctx.db)
                .await
                .unwrap_or_default()
        }
    };

    views::finding::list(
        &v,
        &auth.user,
        &auth.org_ctx,
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
    auth: OrgAuth<ViewerRole>,
) -> Result<Response> {
    let item = findings::Model::find_by_pid_and_org(&ctx.db, &pid, auth.org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, auth.user.id).await;
    views::finding::show(
        &v,
        &auth.user,
        &auth.org_ctx,
        &user_orgs,
        &item,
        auth.is_platform_admin(),
    )
}

/// `POST /findings/:pid/delete` -- delete a single finding (admin only).
#[debug_handler]
pub async fn delete(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    admin: PlatformAdmin,
) -> Result<Response> {
    let item = findings::Model::find_by_pid_and_org(&ctx.db, &pid, admin.org_ctx.org.id)
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
    admin: PlatformAdmin,
    Form(params): Form<BulkDeleteParams>,
) -> Result<Response> {
    for pid in &params.pids {
        if let Some(item) =
            findings::Model::find_by_pid_and_org(&ctx.db, pid, admin.org_ctx.org.id).await
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
