use super::*;
use sea_orm_migration::sea_orm::{
    entity::prelude::*, ActiveModelTrait, ActiveValue::Set, EntityTrait, QueryFilter,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Minimal entity for seeding — avoids depending on the full app models.
mod organizations {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, DeriveEntityModel)]
    #[sea_orm(table_name = "organizations")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub pid: Uuid,
        pub name: String,
        #[sea_orm(unique)]
        pub slug: String,
        pub is_personal: bool,
        pub is_platform_admin: bool,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let exists = organizations::Entity::find()
            .filter(organizations::Column::Slug.eq("gethacked-admin"))
            .one(db)
            .await?;

        if exists.is_none() {
            let pid = Uuid::parse_str("00000000-0000-0000-0000-000000000001")
                .map_err(|e| DbErr::Custom(e.to_string()))?;
            let now: DateTimeWithTimeZone = chrono::Utc::now().into();
            organizations::ActiveModel {
                pid: Set(pid),
                name: Set("GetHacked Platform".to_string()),
                slug: Set("gethacked-admin".to_string()),
                is_personal: Set(false),
                is_platform_admin: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        organizations::Entity::delete_many()
            .filter(organizations::Column::Slug.eq("gethacked-admin"))
            .exec(db)
            .await?;
        Ok(())
    }
}
