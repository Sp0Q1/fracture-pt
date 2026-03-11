use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Reports {
    Table,
    Id,
    Pid,
    OrgId,
    EngagementId,
    JobId,
    Title,
    ReportType,
    Format,
    StoragePath,
    GeneratedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Engagements {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ScanJobs {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Reports::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Reports::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Reports::Pid).uuid().not_null())
                    .col(ColumnDef::new(Reports::OrgId).integer().not_null())
                    .col(ColumnDef::new(Reports::EngagementId).integer().null())
                    .col(ColumnDef::new(Reports::JobId).integer().null())
                    .col(ColumnDef::new(Reports::Title).string().not_null())
                    .col(
                        ColumnDef::new(Reports::ReportType)
                            .string()
                            .not_null()
                            .default("scan_summary"),
                    )
                    .col(
                        ColumnDef::new(Reports::Format)
                            .string()
                            .not_null()
                            .default("pdf"),
                    )
                    .col(ColumnDef::new(Reports::StoragePath).string().null())
                    .col(
                        ColumnDef::new(Reports::GeneratedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Reports::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Reports::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-reports-org_id")
                            .from(Reports::Table, Reports::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-reports-engagement_id")
                            .from(Reports::Table, Reports::EngagementId)
                            .to(Engagements::Table, Engagements::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-reports-job_id")
                            .from(Reports::Table, Reports::JobId)
                            .to(ScanJobs::Table, ScanJobs::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-reports-pid")
                    .table(Reports::Table)
                    .col(Reports::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Reports::Table).to_owned())
            .await?;
        Ok(())
    }
}
