use chrono::Datelike;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

pub use super::_entities::scan_jobs::{ActiveModel, Column, Entity, Model};
pub type ScanJobs = Entity;

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
    /// Finds all scan jobs for an org.
    pub async fn find_by_org(db: &DatabaseConnection, org_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::OrgId.eq(org_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds scan jobs for a specific target, scoped to an org.
    pub async fn find_by_target(db: &DatabaseConnection, target_id: i32, org_id: i32) -> Vec<Self> {
        Entity::find()
            .filter(Column::TargetId.eq(target_id))
            .filter(Column::OrgId.eq(org_id))
            .order_by(Column::Id, Order::Desc)
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Finds a scan job by pid, scoped to an org.
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

    /// Counts scan jobs created this month for quota enforcement.
    pub async fn count_this_month(db: &DatabaseConnection, org_id: i32) -> u64 {
        let now = chrono::Utc::now();
        let start_of_month = now
            .date_naive()
            .with_day(1)
            .unwrap_or_else(|| now.date_naive());
        let start: chrono::DateTime<chrono::Utc> = start_of_month
            .and_hms_opt(0, 0, 0)
            .unwrap_or_else(|| start_of_month.and_hms_opt(0, 0, 1).expect("valid time"))
            .and_utc();
        Entity::find()
            .filter(Column::OrgId.eq(org_id))
            .filter(Column::CreatedAt.gte(start))
            .count(db)
            .await
            .unwrap_or(0)
    }
}

impl ActiveModel {}

impl Entity {}
