use super::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // Seed the platform admin org. The UNIQUE constraint on slug
        // prevents any user from creating an org with this slug later.
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            r"INSERT INTO organizations (pid, name, slug, is_personal, created_at, updated_at)
              SELECT '00000000-0000-0000-0000-000000000001', 'GetHacked Platform', 'gethacked-admin', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
              WHERE NOT EXISTS (SELECT 1 FROM organizations WHERE slug = 'gethacked-admin')"
                .to_string(),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            "DELETE FROM organizations WHERE slug = 'gethacked-admin'".to_string(),
        ))
        .await?;
        Ok(())
    }
}
