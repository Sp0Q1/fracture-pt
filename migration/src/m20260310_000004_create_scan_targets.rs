use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ScanTargets {
    Table,
    Id,
    Pid,
    OrgId,
    Hostname,
    IpAddress,
    TargetType,
    VerifiedAt,
    VerificationMethod,
    VerificationToken,
    Label,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScanTargets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ScanTargets::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ScanTargets::Pid).uuid().not_null())
                    .col(ColumnDef::new(ScanTargets::OrgId).integer().not_null())
                    .col(ColumnDef::new(ScanTargets::Hostname).string().null())
                    .col(ColumnDef::new(ScanTargets::IpAddress).string().null())
                    .col(
                        ColumnDef::new(ScanTargets::TargetType)
                            .string()
                            .not_null()
                            .default("domain"),
                    )
                    .col(
                        ColumnDef::new(ScanTargets::VerifiedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScanTargets::VerificationMethod)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScanTargets::VerificationToken)
                            .string()
                            .null(),
                    )
                    .col(ColumnDef::new(ScanTargets::Label).string().null())
                    .col(
                        ColumnDef::new(ScanTargets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ScanTargets::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-scan_targets-org_id")
                            .from(ScanTargets::Table, ScanTargets::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-scan_targets-pid")
                    .table(ScanTargets::Table)
                    .col(ScanTargets::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-scan_targets-org_id-hostname")
                    .table(ScanTargets::Table)
                    .col(ScanTargets::OrgId)
                    .col(ScanTargets::Hostname)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScanTargets::Table).to_owned())
            .await?;
        Ok(())
    }
}
