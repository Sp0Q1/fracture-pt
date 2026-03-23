use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "engagement_offers")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Text", unique)]
    pub pid: String,
    pub engagement_id: i32,
    pub created_by_user_id: Option<i32>,
    pub amount_cents: i32,
    pub currency: String,
    pub timeline_days: i32,
    #[sea_orm(column_type = "Text")]
    pub deliverables: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub terms: Option<String>,
    pub valid_until: DateTimeWithTimeZone,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub client_response: Option<String>,
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
        belongs_to = "super::users::Entity",
        from = "Column::CreatedByUserId",
        to = "super::users::Column::Id"
    )]
    Users,
}

impl Related<super::engagements::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Engagements.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}
