use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "scan_targets")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,
    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub target_type: String,
    pub verified_at: Option<DateTimeWithTimeZone>,
    pub verification_method: Option<String>,
    pub verification_token: Option<String>,
    pub label: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organizations::Entity",
        from = "Column::OrgId",
        to = "super::organizations::Column::Id"
    )]
    Organizations,
    #[sea_orm(has_many = "super::scan_jobs::Entity")]
    ScanJobs,
    #[sea_orm(has_many = "super::engagement_targets::Entity")]
    EngagementTargets,
}

impl Related<super::organizations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organizations.def()
    }
}

impl Related<super::scan_jobs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ScanJobs.def()
    }
}

impl Related<super::engagement_targets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EngagementTargets.def()
    }
}
