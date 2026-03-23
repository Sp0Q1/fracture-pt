use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

use super::_entities::pentester_assignments;

pub use super::_entities::engagements::{ActiveModel, Column, Entity, Model};
pub type Engagements = Entity;

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
    /// Finds all engagements for an org.
    pub async fn find_by_org(db: &DatabaseConnection, org_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::OrgId.eq(org_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds an engagement by pid, scoped to an org.
    pub async fn find_by_pid_and_org(
        db: &DatabaseConnection,
        pid: &str,
        org_id: i32,
    ) -> Option<Self> {
        let _uuid = Uuid::parse_str(pid).ok()?;
        Entity::find()
            .filter(Column::Pid.eq(pid))
            .filter(Column::OrgId.eq(org_id))
            .one(db)
            .await
            .ok()
            .flatten()
    }

    /// Finds an engagement by pid (admin use -- cross-org).
    pub async fn find_by_pid(db: &DatabaseConnection, pid: &str) -> Option<Self> {
        let _uuid = Uuid::parse_str(pid).ok()?;
        Entity::find()
            .filter(Column::Pid.eq(pid))
            .one(db)
            .await
            .ok()
            .flatten()
    }

    /// Finds all pending engagement requests (admin use -- cross-org).
    pub async fn find_all_pending(db: &DatabaseConnection) -> Vec<Self> {
        Entity::find()
            .filter(Column::Status.eq("requested"))
            .order_by(Column::RequestedAt, Order::Asc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds engagements by status (admin use -- cross-org).
    pub async fn find_by_status(db: &DatabaseConnection, status: &str) -> Vec<Self> {
        Entity::find()
            .filter(Column::Status.eq(status))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds all engagements assigned to a pentester (via pentester_assignments join).
    pub async fn find_by_pentester(db: &DatabaseConnection, user_id: i32) -> Vec<Self> {
        Entity::find()
            .inner_join(pentester_assignments::Entity)
            .filter(pentester_assignments::Column::UserId.eq(user_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }
}

impl ActiveModel {}

impl Entity {}
