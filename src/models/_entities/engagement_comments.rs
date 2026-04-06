//! `SeaORM` Entity — engagement_comments

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "engagement_comments")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub engagement_id: i32,
    pub user_id: i32,
    #[sea_orm(column_type = "Text")]
    pub content: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::engagements::Entity",
        from = "Column::EngagementId",
        to = "super::engagements::Column::Id"
    )]
    Engagements,
}

impl Related<super::engagements::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Engagements.def()
    }
}

// ActiveModelBehavior is implemented in models/engagement_comments.rs
