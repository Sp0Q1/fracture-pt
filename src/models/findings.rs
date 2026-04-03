use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

pub use super::_entities::findings::{ActiveModel, Column, Entity, Model};
pub type Findings = Entity;

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
    /// Finds all findings for an org.
    pub async fn find_by_org(db: &DatabaseConnection, org_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::OrgId.eq(org_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds findings for a specific scan job, scoped to an org.
    pub async fn find_by_job(db: &DatabaseConnection, job_id: i32, org_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::JobId.eq(job_id))
            .filter(Column::OrgId.eq(org_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds findings for a specific engagement.
    pub async fn find_by_engagement(db: &DatabaseConnection, engagement_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::EngagementId.eq(engagement_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds findings by severity for an org.
    pub async fn find_by_severity(
        db: &DatabaseConnection,
        org_id: i32,
        severity: &str,
    ) -> Vec<Self> {
        Entity::find()
            .filter(Column::OrgId.eq(org_id))
            .filter(Column::Severity.eq(severity))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds a finding by pid, scoped to an org (client access).
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

    /// Finds a finding by pid, scoped to an engagement (pentester access).
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

    /// Finds all findings cross-org (admin use).
    pub async fn find_all(db: &DatabaseConnection) -> Vec<Self> {
        Entity::find()
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds a finding by pid (admin use -- cross-org).
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
