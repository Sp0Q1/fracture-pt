use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ScanJobs {
    Table,
    Id,
    Pid,
    OrgId,
    TargetId,
    ServiceId,
    Status,
    ScheduledAt,
    StartedAt,
    CompletedAt,
    ResultSummary,
    FindingCount,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ScanTargets {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Services {
    Table,
    Id,
}

#[async_trait::async_trait]
#[allow(clippy::too_many_lines)]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScanJobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ScanJobs::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ScanJobs::Pid).uuid().not_null())
                    .col(ColumnDef::new(ScanJobs::OrgId).integer().not_null())
                    .col(ColumnDef::new(ScanJobs::TargetId).integer().not_null())
                    .col(ColumnDef::new(ScanJobs::ServiceId).integer().not_null())
                    .col(
                        ColumnDef::new(ScanJobs::Status)
                            .string()
                            .not_null()
                            .default("queued"),
                    )
                    .col(
                        ColumnDef::new(ScanJobs::ScheduledAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScanJobs::StartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScanJobs::CompletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(ScanJobs::ResultSummary).text().null())
                    .col(
                        ColumnDef::new(ScanJobs::FindingCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(ScanJobs::ErrorMessage).string().null())
                    .col(
                        ColumnDef::new(ScanJobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ScanJobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-scan_jobs-org_id")
                            .from(ScanJobs::Table, ScanJobs::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-scan_jobs-target_id")
                            .from(ScanJobs::Table, ScanJobs::TargetId)
                            .to(ScanTargets::Table, ScanTargets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-scan_jobs-service_id")
                            .from(ScanJobs::Table, ScanJobs::ServiceId)
                            .to(Services::Table, Services::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-scan_jobs-pid")
                    .table(ScanJobs::Table)
                    .col(ScanJobs::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-scan_jobs-org_id-status")
                    .table(ScanJobs::Table)
                    .col(ScanJobs::OrgId)
                    .col(ScanJobs::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-scan_jobs-target_id")
                    .table(ScanJobs::Table)
                    .col(ScanJobs::TargetId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScanJobs::Table).to_owned())
            .await?;
        Ok(())
    }
}
