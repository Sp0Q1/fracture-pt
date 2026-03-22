use sea_orm::entity::prelude::*;

use crate::models::_entities::{org_members, pentester_assignments};
use crate::models::organizations as org_model;

/// The platform admin org has a well-known UUID, seeded by migration.
/// Membership in this org grants platform admin access.
const ADMIN_ORG_PID: &str = "00000000-0000-0000-0000-000000000001";

pub async fn is_platform_admin(db: &DatabaseConnection, user_id: i32) -> bool {
    let Some(admin_org) = org_model::Model::find_by_pid(db, ADMIN_ORG_PID).await else {
        tracing::warn!("admin org not found (pid={ADMIN_ORG_PID}), denying admin access");
        return false;
    };
    let is_admin = org_members::Entity::find()
        .filter(org_members::Column::OrgId.eq(admin_org.id))
        .filter(org_members::Column::UserId.eq(user_id))
        .one(db)
        .await
        .ok()
        .flatten()
        .is_some();
    tracing::debug!(user_id, is_admin, "platform admin check");
    is_admin
}

/// Check if a user is an assigned pentester for a specific engagement.
pub async fn is_assigned_pentester(
    db: &DatabaseConnection,
    user_id: i32,
    engagement_id: i32,
) -> bool {
    pentester_assignments::Entity::find()
        .filter(pentester_assignments::Column::UserId.eq(user_id))
        .filter(pentester_assignments::Column::EngagementId.eq(engagement_id))
        .one(db)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// Macro: require the user to be a platform admin.
#[macro_export]
macro_rules! require_platform_admin {
    ($db:expr, $user:expr) => {
        if !$crate::models::auth::is_platform_admin($db, $user.id).await {
            return Ok(axum::response::Response::builder()
                .status(axum::http::StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(axum::body::Body::from(
                    "<h1>403 Forbidden</h1><p>You do not have admin access.</p>",
                ))
                .expect("static response body")
                .into_response());
        }
    };
}
