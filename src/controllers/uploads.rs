//! Upload controller override that adds pentester assignment checks
//! to the base fracture-core upload access control.
//!
//! Pentesters assigned to an engagement in an org get read/delete access
//! to that org's uploads (for evidence screenshots in findings).

use axum::body::Body;
use axum::extract::Multipart;
use axum::http::header;
use axum_extra::extract::cookie::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::controllers::middleware;
use crate::models::_entities::{engagements, pentester_assignments};
use crate::require_user;

use fracture_core::models::_entities::uploads as uploads_entity;
use fracture_core::models::org_members::OrgRole;
use fracture_core::models::uploads as upload_model;
use fracture_core::upload::config::UploadConfig;
use fracture_core::upload::service::UploadService;

/// Check if a user has a pentester assignment to any engagement in the given org.
async fn has_org_pentester_assignment(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    org_id: i32,
) -> bool {
    // Find engagements in this org
    let org_engagements = engagements::Entity::find()
        .filter(engagements::Column::OrgId.eq(org_id))
        .all(db)
        .await
        .unwrap_or_default();

    if org_engagements.is_empty() {
        return false;
    }

    let eng_ids: Vec<i32> = org_engagements.iter().map(|e| e.id).collect();

    pentester_assignments::Entity::find()
        .filter(pentester_assignments::Column::UserId.eq(user_id))
        .filter(pentester_assignments::Column::EngagementId.is_in(eng_ids))
        .one(db)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// Constructs an `UploadService` from the application settings.
async fn get_upload_service(ctx: &AppContext) -> Result<UploadService> {
    let config = UploadConfig::from_settings(ctx.config.settings.as_ref());
    UploadService::new(config)
        .await
        .map_err(|e| Error::Message(format!("Failed to initialize upload service: {e}")))
}

/// `POST /api/uploads` — delegates to core's create handler.
///
/// No override needed — the core handler's org membership check is sufficient
/// because only org members and pentesters with workspace access upload.
#[debug_handler]
pub async fn create(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    multipart: Multipart,
) -> Result<Response> {
    fracture_core::controllers::uploads::create(State(ctx), jar, multipart).await
}

/// `GET /api/uploads/{pid}` — serve file with pentester assignment check.
#[debug_handler]
pub async fn show(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let upload = upload_model::Model::find_by_pid(&ctx.db, &pid)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let vis = upload_model::Visibility::parse(&upload.visibility);
    match vis {
        Some(upload_model::Visibility::Public) => {
            // Public files served to everyone
        }
        Some(upload_model::Visibility::Org) | None => {
            let Some(user) = middleware::get_current_user(&jar, &ctx).await else {
                return Err(Error::NotFound);
            };

            let is_platform_admin =
                fracture_core::models::organizations::Model::is_user_platform_admin(
                    &ctx.db, user.id,
                )
                .await;

            if !is_platform_admin {
                let is_org_member = fracture_core::models::_entities::org_members::Entity::find()
                    .filter(
                        fracture_core::models::_entities::org_members::Column::OrgId
                            .eq(upload.org_id),
                    )
                    .filter(
                        fracture_core::models::_entities::org_members::Column::UserId.eq(user.id),
                    )
                    .one(&ctx.db)
                    .await
                    .ok()
                    .flatten()
                    .is_some();

                let is_pentester =
                    has_org_pentester_assignment(&ctx.db, user.id, upload.org_id).await;

                if !is_org_member && !is_pentester {
                    return Err(Error::NotFound);
                }
            }
        }
    }

    let service = get_upload_service(&ctx).await?;
    let data = service
        .read_file(&upload)
        .await
        .map_err(|_| Error::NotFound)?;

    let cache_control = match vis {
        Some(upload_model::Visibility::Public) => "public, max-age=86400, immutable",
        _ => "private, no-cache",
    };

    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(header::CONTENT_TYPE, &upload.content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", upload.original_name),
        )
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(data))
        .unwrap()
        .into_response())
}

/// `DELETE /api/uploads/{pid}` — delete with pentester assignment check.
#[debug_handler]
pub async fn destroy(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);

    let upload = upload_model::Model::find_by_pid(&ctx.db, &pid)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let is_uploader = upload.uploaded_by == user.id;
    if !is_uploader {
        let is_platform_admin =
            fracture_core::models::organizations::Model::is_user_platform_admin(&ctx.db, user.id)
                .await;

        if !is_platform_admin {
            let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
            let is_org_admin = org_ctx
                .as_ref()
                .is_some_and(|c| c.org.id == upload.org_id && c.role.at_least(OrgRole::Admin));
            let is_pentester = has_org_pentester_assignment(&ctx.db, user.id, upload.org_id).await;

            if !is_org_admin && !is_pentester {
                return Err(Error::NotFound);
            }
        }
    }

    let service = get_upload_service(&ctx).await?;
    let _ = service.delete_file(&upload).await;

    let active: uploads_entity::ActiveModel = upload.into();
    active.delete(&ctx.db).await?;

    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap()
        .into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/uploads")
        .add("/", post(create))
        .add("/{pid}", get(show))
        .add("/{pid}", delete(destroy))
}
