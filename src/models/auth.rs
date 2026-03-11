use sea_orm::entity::prelude::*;

use crate::models::_entities::{org_members, organizations, pentester_assignments};

/// Check if a user is a platform admin (member of the gethacked-admin org).
pub async fn is_platform_admin(db: &DatabaseConnection, user_id: i32) -> bool {
    if let Some(admin_org) = organizations::Entity::find()
        .filter(organizations::Column::Slug.eq("gethacked-admin"))
        .one(db)
        .await
        .ok()
        .flatten()
    {
        org_members::Entity::find()
            .filter(org_members::Column::OrgId.eq(admin_org.id))
            .filter(org_members::Column::UserId.eq(user_id))
            .one(db)
            .await
            .ok()
            .flatten()
            .is_some()
    } else {
        false
    }
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
                .body(axum::body::Body::from("Forbidden"))
                .expect("static response body")
                .into_response());
        }
    };
}
