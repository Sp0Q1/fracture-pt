use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "services")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub name: String,
    #[sea_orm(unique)]
    pub slug: String,
    pub category: String,
    #[sea_orm(column_type = "Text")]
    pub description: String,
    pub is_automated: bool,
    pub is_active: bool,
    pub sort_order: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::pricing_tiers::Entity")]
    PricingTiers,
    #[sea_orm(has_many = "super::scan_jobs::Entity")]
    ScanJobs,
    #[sea_orm(has_many = "super::engagements::Entity")]
    Engagements,
}

impl Related<super::pricing_tiers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PricingTiers.def()
    }
}

impl Related<super::scan_jobs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ScanJobs.def()
    }
}

impl Related<super::engagements::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Engagements.def()
    }
}
