use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum EngagementTargets {
    Table,
    Id,
    EngagementId,
    ScanTargetId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Engagements {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ScanTargets {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EngagementTargets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EngagementTargets::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EngagementTargets::EngagementId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementTargets::ScanTargetId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementTargets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagement_targets-engagement_id")
                            .from(EngagementTargets::Table, EngagementTargets::EngagementId)
                            .to(Engagements::Table, Engagements::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagement_targets-scan_target_id")
                            .from(EngagementTargets::Table, EngagementTargets::ScanTargetId)
                            .to(ScanTargets::Table, ScanTargets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-engagement_targets-engagement_id-scan_target_id")
                    .table(EngagementTargets::Table)
                    .col(EngagementTargets::EngagementId)
                    .col(EngagementTargets::ScanTargetId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EngagementTargets::Table).to_owned())
            .await?;
        Ok(())
    }
}
