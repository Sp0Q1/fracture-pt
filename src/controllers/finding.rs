use axum::response::Redirect;
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use serde::Deserialize;

use super::auth::{AdminRole, OrgAuth, ViewerRole};
use crate::models::_entities::{engagements, finding_comments, pentester_assignments};
use crate::models::findings;
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::views;

/// A new comment posted on a finding.
#[derive(Debug, Deserialize)]
pub struct CommentParams {
    pub content: String,
}

/// `GET /findings` -- list all findings for org.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    auth: OrgAuth<ViewerRole>,
) -> Result<Response> {
    let items = findings::Model::find_by_org(&ctx.db, auth.org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, auth.user.id).await?;

    let is_admin = auth.org_ctx.role.at_least(OrgRole::Admin);
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
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, auth.user.id).await?;
    // Edit access mirrors the engagement's finding-edit gate: staff or the
    // assigned pentester. The link targets the existing engagement edit route.
    // A finding may be unattached to an engagement (engagement_id is optional).
    let (engagement, is_assigned) = if let Some(eid) = item.engagement_id {
        let eng = engagements::Entity::find_by_id(eid).one(&ctx.db).await?;
        let assigned =
            pentester_assignments::Model::is_assigned(&ctx.db, auth.user.id, eid).await;
        (eng, assigned)
    } else {
        (None, false)
    };
    let can_edit = auth.is_staff() || is_assigned;
    let comments = finding_comments::Model::find_by_finding_with_users(&ctx.db, item.id).await;
    views::finding::show(
        &v,
        &auth.user,
        &auth.org_ctx,
        &user_orgs,
        &item,
        auth.is_staff(),
        engagement.as_ref(),
        can_edit,
        &comments,
    )
}

/// `POST /findings/:pid/comments` -- add a comment to a finding.
///
/// Any org member who can view the finding may comment (clients and staff
/// alike), so findings are a shared discussion surface.
#[debug_handler]
pub async fn add_comment(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    auth: OrgAuth<ViewerRole>,
    Form(params): Form<CommentParams>,
) -> Result<Response> {
    let item = findings::Model::find_by_pid_and_org(&ctx.db, &pid, auth.org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    if params.content.trim().is_empty() {
        return Err(Error::BadRequest("Comment cannot be empty".into()));
    }
    let comment = finding_comments::ActiveModel {
        finding_id: Set(item.id),
        user_id: Set(auth.user.id),
        content: Set(params.content),
        ..Default::default()
    };
    comment.insert(&ctx.db).await?;
    Ok(Redirect::to(&format!("/findings/{pid}")).into_response())
}

/// `POST /findings/:pid/delete` -- delete a single finding (org admin+).
#[debug_handler]
pub async fn delete(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    auth: OrgAuth<AdminRole>,
) -> Result<Response> {
    let item = findings::Model::find_by_pid_and_org(&ctx.db, &pid, auth.org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    item.delete(&ctx.db).await?;
    Ok(Redirect::to("/findings").into_response())
}

/// `POST /findings/bulk-delete` -- delete multiple findings (org admin+).
///
/// HTML checkboxes with `name="pids[]"` produce repeated form keys which
/// serde cannot deserialize into a struct. We parse the URL-encoded body
/// directly with `form_urlencoded`.
#[debug_handler]
pub async fn bulk_delete(
    State(ctx): State<AppContext>,
    auth: OrgAuth<AdminRole>,
    body: String,
) -> Result<Response> {
    let pids: Vec<String> = form_urlencoded::parse(body.as_bytes())
        .filter(|(key, _)| key == "pids[]")
        .map(|(_, val)| val.into_owned())
        .collect();

    for pid in &pids {
        if let Some(item) =
            findings::Model::find_by_pid_and_org(&ctx.db, pid, auth.org_ctx.org.id).await
        {
            item.delete(&ctx.db).await?;
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
        .add("/{pid}/comments", post(add_comment))
        .add("/{pid}/delete", post(delete))
}
