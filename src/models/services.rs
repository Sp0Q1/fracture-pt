use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

pub use super::_entities::services::{ActiveModel, Column, Entity, Model};
pub type Services = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert {
            this.pid = sea_orm::ActiveValue::Set(Uuid::new_v4());
        } else if this.updated_at.is_unchanged() {
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        }
        Ok(this)
    }
}

impl Model {
    /// Finds all active services ordered by sort_order.
    pub async fn find_active(db: &DatabaseConnection) -> Vec<Self> {
        Entity::find()
            .filter(Column::IsActive.eq(true))
            .order_by(Column::SortOrder, Order::Asc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds a service by its URL slug.
    pub async fn find_by_slug(db: &DatabaseConnection, slug: &str) -> Option<Self> {
        Entity::find()
            .filter(Column::Slug.eq(slug))
            .one(db)
            .await
            .ok()
            .flatten()
    }

    /// Finds a service by internal id.
    pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> Option<Self> {
        Entity::find_by_id(id).one(db).await.ok().flatten()
    }

    /// Finds a service by pid.
    pub async fn find_by_pid(db: &DatabaseConnection, pid: &str) -> Option<Self> {
        let uuid = Uuid::parse_str(pid).ok()?;
        Entity::find()
            .filter(Column::Pid.eq(uuid))
            .one(db)
            .await
            .ok()
            .flatten()
    }
}

impl ActiveModel {}

impl Entity {}
