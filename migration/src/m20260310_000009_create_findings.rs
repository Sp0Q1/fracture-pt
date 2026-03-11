use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Findings {
    Table,
    Id,
    Pid,
    OrgId,
    EngagementId,
    JobId,
    CreatedByUserId,
    Title,
    Description,
    TechnicalDescription,
    Impact,
    Recommendation,
    Severity,
    CveId,
    Category,
    Evidence,
    AffectedAsset,
    Status,
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

#[derive(DeriveIden)]
enum Users {
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
                    .table(Findings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Findings::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Findings::Pid).uuid().not_null())
                    .col(ColumnDef::new(Findings::OrgId).integer().not_null())
                    .col(ColumnDef::new(Findings::EngagementId).integer().null())
                    .col(ColumnDef::new(Findings::JobId).integer().null())
                    .col(ColumnDef::new(Findings::CreatedByUserId).integer().null())
                    .col(ColumnDef::new(Findings::Title).string().not_null())
                    .col(ColumnDef::new(Findings::Description).text().not_null())
                    .col(ColumnDef::new(Findings::TechnicalDescription).text().null())
                    .col(ColumnDef::new(Findings::Impact).text().null())
                    .col(ColumnDef::new(Findings::Recommendation).text().null())
                    .col(
                        ColumnDef::new(Findings::Severity)
                            .string()
                            .not_null()
                            .default("low"),
                    )
                    .col(ColumnDef::new(Findings::CveId).string().null())
                    .col(
                        ColumnDef::new(Findings::Category)
                            .string()
                            .not_null()
                            .default("other"),
                    )
                    .col(ColumnDef::new(Findings::Evidence).text().null())
                    .col(ColumnDef::new(Findings::AffectedAsset).string().null())
                    .col(
                        ColumnDef::new(Findings::Status)
                            .string()
                            .not_null()
                            .default("open"),
                    )
                    .col(
                        ColumnDef::new(Findings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Findings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-findings-org_id")
                            .from(Findings::Table, Findings::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-findings-engagement_id")
                            .from(Findings::Table, Findings::EngagementId)
                            .to(Engagements::Table, Engagements::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-findings-job_id")
                            .from(Findings::Table, Findings::JobId)
                            .to(ScanJobs::Table, ScanJobs::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-findings-created_by_user_id")
                            .from(Findings::Table, Findings::CreatedByUserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-findings-pid")
                    .table(Findings::Table)
                    .col(Findings::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-findings-org_id-severity")
                    .table(Findings::Table)
                    .col(Findings::OrgId)
                    .col(Findings::Severity)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-findings-engagement_id")
                    .table(Findings::Table)
                    .col(Findings::EngagementId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-findings-job_id")
                    .table(Findings::Table)
                    .col(Findings::JobId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Findings::Table).to_owned())
            .await?;
        Ok(())
    }
}
