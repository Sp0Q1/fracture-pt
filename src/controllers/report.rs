use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;

use super::middleware;
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::models::reports;
use crate::{require_role, require_user, views};

/// `GET /reports` -- list reports.
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
    let items = reports::Model::find_by_org(&ctx.db, org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    views::report::list(&v, &user, &org_ctx, &user_orgs, &items)
}

/// `GET /reports/:pid` -- show a report.
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
    let item = reports::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    views::report::show(&v, &user, &org_ctx, &user_orgs, &item)
}

/// `GET /reports/:pid/download` -- download a report PDF file.
#[debug_handler]
pub async fn download(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Viewer);
    let item = reports::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    let storage_path = item
        .storage_path
        .as_deref()
        .ok_or_else(|| Error::NotFound)?;

    let bytes = tokio::fs::read(storage_path)
        .await
        .map_err(|_| Error::NotFound)?;

    let (content_type, ext) = match item.format.as_str() {
        "pdf" => ("application/pdf", "pdf"),
        "zip" => ("application/zip", "zip"),
        _ => ("application/octet-stream", "bin"),
    };
    let filename = format!("report-{}.{ext}", item.pid);
    Ok(axum::response::Response::builder()
        .header("content-type", content_type)
        .header(
            "content-disposition",
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(bytes))
        .unwrap()
        .into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/reports")
        .add("/", get(list))
        .add("/{pid}", get(show))
        .add("/{pid}/download", get(download))
}
