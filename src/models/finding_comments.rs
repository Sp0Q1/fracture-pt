use sea_orm::entity::prelude::*;
use sea_orm::QueryOrder;

pub use super::_entities::finding_comments::{ActiveModel, Column, Entity, Model};

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert {
            this.pid = sea_orm::ActiveValue::Set(Uuid::new_v4());
        }
        this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        Ok(this)
    }
}

impl Model {
    /// Find all comments for a finding, oldest first, each paired with its
    /// author (so the view can show who said what).
    pub async fn find_by_finding_with_users(
        db: &DatabaseConnection,
        finding_id: i32,
    ) -> Vec<(Self, Option<super::users::Model>)> {
        let comments = Entity::find()
            .filter(Column::FindingId.eq(finding_id))
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await
            .unwrap_or_default();
        let user_ids: Vec<i32> = comments.iter().map(|c| c.user_id).collect();
        let mut users_by_id: std::collections::HashMap<i32, super::users::Model> =
            super::_entities::users::Entity::find()
                .filter(super::_entities::users::Column::Id.is_in(user_ids))
                .all(db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|u| (u.id, u))
                .collect();
        comments
            .into_iter()
            .map(|c| {
                let author = users_by_id.remove(&c.user_id);
                (c, author)
            })
            .collect()
    }
}
