use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

pub use super::_entities::pricing_tiers::{ActiveModel, Column, Entity, Model};
pub type PricingTiers = Entity;

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
    /// Finds all tiers for a service, ordered by sort_order.
    pub async fn find_by_service(db: &DatabaseConnection, service_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::ServiceId.eq(service_id))
            .order_by(Column::SortOrder, Order::Asc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds active tiers for a service.
    pub async fn find_active_by_service(db: &DatabaseConnection, service_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::ServiceId.eq(service_id))
            .filter(Column::IsActive.eq(true))
            .order_by(Column::SortOrder, Order::Asc)
            .all(db)
            .await
            .unwrap_or_default()
    }
}

impl ActiveModel {}

impl Entity {}
