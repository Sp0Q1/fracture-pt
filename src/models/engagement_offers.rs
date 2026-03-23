use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

pub use super::_entities::engagement_offers::{ActiveModel, Column, Entity, Model};
pub type EngagementOffers = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert {
            this.pid = sea_orm::ActiveValue::Set(Uuid::new_v4().to_string());
        } else if this.updated_at.is_unchanged() {
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        }
        Ok(this)
    }
}

impl Model {
    /// Finds all offers for an engagement, newest first.
    pub async fn find_by_engagement(db: &DatabaseConnection, engagement_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::EngagementId.eq(engagement_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds the most recent offer for an engagement.
    pub async fn find_latest_by_engagement(
        db: &DatabaseConnection,
        engagement_id: i32,
    ) -> Option<Self> {
        Entity::find()
            .filter(Column::EngagementId.eq(engagement_id))
            .order_by(Column::Id, Order::Desc)
            .one(db)
            .await
            .ok()
            .flatten()
    }
}

impl ActiveModel {}

impl Entity {}
