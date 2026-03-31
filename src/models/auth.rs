use sea_orm::entity::prelude::*;

use crate::models::_entities::pentester_assignments;

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
