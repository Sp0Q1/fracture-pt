use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "engagement_targets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub engagement_id: i32,
    pub scan_target_id: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::engagements::Entity",
        from = "Column::EngagementId",
        to = "super::engagements::Column::Id"
    )]
    Engagements,
    #[sea_orm(
        belongs_to = "super::scan_targets::Entity",
        from = "Column::ScanTargetId",
        to = "super::scan_targets::Column::Id"
    )]
    ScanTargets,
}

impl Related<super::engagements::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Engagements.def()
    }
}

impl Related<super::scan_targets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ScanTargets.def()
    }
}
