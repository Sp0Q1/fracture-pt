use sea_orm::entity::prelude::*;
use sea_orm::QueryOrder;

pub use super::_entities::engagement_comments::{ActiveModel, Column, Entity, Model};

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert {
            this.pid = sea_orm::ActiveValue::Set(Uuid::new_v4());
        }
        this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        Ok(this)
    }
}

impl Model {
    /// Find all comments for an engagement, oldest first.
    pub async fn find_by_engagement(db: &DatabaseConnection, engagement_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::EngagementId.eq(engagement_id))
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await
            .unwrap_or_default()
    }
}
