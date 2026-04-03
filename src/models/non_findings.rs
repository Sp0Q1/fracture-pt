use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

pub use super::_entities::non_findings::{ActiveModel, Column, Entity, Model};
pub type NonFindings = Entity;

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
    /// Finds a non-finding by its public id.
    pub async fn find_by_pid(db: &DatabaseConnection, pid: &str) -> Option<Self> {
        let uuid = Uuid::parse_str(pid).ok()?;
        Entity::find()
            .filter(Column::Pid.eq(uuid))
            .one(db)
            .await
            .ok()
            .flatten()
    }

    /// Finds all non-findings for an engagement, ordered by sort_order.
    pub async fn find_by_engagement(db: &DatabaseConnection, engagement_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::EngagementId.eq(engagement_id))
            .order_by(Column::SortOrder, Order::Asc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds a non-finding by pid, scoped to an org.
    pub async fn find_by_pid_and_org(
        db: &DatabaseConnection,
        pid: &str,
        org_id: i32,
    ) -> Option<Self> {
        let uuid = Uuid::parse_str(pid).ok()?;
        Entity::find()
            .filter(Column::Pid.eq(uuid))
            .filter(Column::OrgId.eq(org_id))
            .one(db)
            .await
            .ok()
            .flatten()
    }

    /// Finds a non-finding by pid, scoped to an engagement (pentester access).
    pub async fn find_by_pid_and_engagement(
        db: &DatabaseConnection,
        pid: &str,
        engagement_id: i32,
    ) -> Option<Self> {
        let uuid = Uuid::parse_str(pid).ok()?;
        Entity::find()
            .filter(Column::Pid.eq(uuid))
            .filter(Column::EngagementId.eq(engagement_id))
            .one(db)
            .await
            .ok()
            .flatten()
    }
}

impl ActiveModel {}

impl Entity {}
