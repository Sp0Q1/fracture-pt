use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "findings")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,
    // Source: exactly one of these must be set
    pub engagement_id: Option<i32>,
    pub job_id: Option<i32>,
    // Who created it
    pub created_by_user_id: Option<i32>,
    // Finding content
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub description: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub technical_description: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub impact: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub recommendation: Option<String>,
    // Classification
    pub severity: String,
    pub cve_id: Option<String>,
    pub category: String,
    // Evidence
    #[sea_orm(column_type = "Text", nullable)]
    pub evidence: Option<String>,
    pub affected_asset: Option<String>,
    // Status tracking
    pub status: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organizations::Entity",
        from = "Column::OrgId",
        to = "super::organizations::Column::Id"
    )]
    Organizations,
    #[sea_orm(
        belongs_to = "super::engagements::Entity",
        from = "Column::EngagementId",
        to = "super::engagements::Column::Id"
    )]
    Engagements,
    #[sea_orm(
        belongs_to = "super::scan_jobs::Entity",
        from = "Column::JobId",
        to = "super::scan_jobs::Column::Id"
    )]
    ScanJobs,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::CreatedByUserId",
        to = "super::users::Column::Id"
    )]
    Users,
}

impl Related<super::organizations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organizations.def()
    }
}

impl Related<super::engagements::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Engagements.def()
    }
}

impl Related<super::scan_jobs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ScanJobs.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}
