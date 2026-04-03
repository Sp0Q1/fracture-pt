use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "engagements")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,
    pub service_id: i32,
    pub title: String,
    pub status: String,
    // Client-submitted scope
    #[sea_orm(column_type = "Text")]
    pub target_systems: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub ip_ranges: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub domains: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub exclusions: Option<String>,
    pub test_window_start: Option<DateTimeWithTimeZone>,
    pub test_window_end: Option<DateTimeWithTimeZone>,
    pub contact_name: String,
    pub contact_email: String,
    pub contact_phone: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub rules_of_engagement: Option<String>,
    // Admin-managed fields
    pub requested_at: DateTimeWithTimeZone,
    pub starts_at: Option<DateTimeWithTimeZone>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub price_cents: Option<i32>,
    pub currency: Option<String>,
    #[serde(skip_serializing)]
    #[sea_orm(column_type = "Text", nullable)]
    pub admin_notes: Option<String>,
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
        belongs_to = "super::services::Entity",
        from = "Column::ServiceId",
        to = "super::services::Column::Id"
    )]
    Services,
    #[sea_orm(has_many = "super::engagement_offers::Entity")]
    EngagementOffers,
    #[sea_orm(has_many = "super::pentester_assignments::Entity")]
    PentesterAssignments,
    #[sea_orm(has_many = "super::findings::Entity")]
    Findings,
    #[sea_orm(has_many = "super::non_findings::Entity")]
    NonFindings,
    #[sea_orm(has_many = "super::reports::Entity")]
    Reports,
    #[sea_orm(has_many = "super::invoices::Entity")]
    Invoices,
    #[sea_orm(has_many = "super::engagement_targets::Entity")]
    EngagementTargets,
}

impl Related<super::organizations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organizations.def()
    }
}

impl Related<super::services::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Services.def()
    }
}

impl Related<super::engagement_offers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EngagementOffers.def()
    }
}

impl Related<super::pentester_assignments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PentesterAssignments.def()
    }
}

impl Related<super::findings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Findings.def()
    }
}

impl Related<super::non_findings::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::NonFindings.def()
    }
}

impl Related<super::reports::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Reports.def()
    }
}

impl Related<super::invoices::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invoices.def()
    }
}

impl Related<super::engagement_targets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EngagementTargets.def()
    }
}
