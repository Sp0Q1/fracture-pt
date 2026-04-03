use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};

use super::_entities::scan_targets;

pub use super::_entities::engagement_targets::{ActiveModel, Column, Entity, Model};
pub type EngagementTargets = Entity;

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Finds all scan targets linked to an engagement.
    pub async fn find_targets_for_engagement(
        db: &DatabaseConnection,
        engagement_id: i32,
    ) -> Vec<scan_targets::Model> {
        let links = Entity::find()
            .filter(Column::EngagementId.eq(engagement_id))
            .all(db)
            .await
            .unwrap_or_default();

        let target_ids: Vec<i32> = links.iter().map(|l| l.scan_target_id).collect();
        if target_ids.is_empty() {
            return Vec::new();
        }

        scan_targets::Entity::find()
            .filter(scan_targets::Column::Id.is_in(target_ids))
            .all(db)
            .await
            .unwrap_or_default()
    }

    /// Links a scan target to an engagement.
    pub async fn link(
        db: &DatabaseConnection,
        engagement_id: i32,
        scan_target_id: i32,
    ) -> Result<Self, DbErr> {
        let item = ActiveModel {
            engagement_id: sea_orm::ActiveValue::Set(engagement_id),
            scan_target_id: sea_orm::ActiveValue::Set(scan_target_id),
            ..Default::default()
        };
        item.insert(db).await
    }

    /// Unlinks a scan target from an engagement.
    pub async fn unlink(
        db: &DatabaseConnection,
        engagement_id: i32,
        scan_target_id: i32,
    ) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::EngagementId.eq(engagement_id))
            .filter(Column::ScanTargetId.eq(scan_target_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl ActiveModel {}

impl Entity {}
