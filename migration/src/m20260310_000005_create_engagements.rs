use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Engagements {
    Table,
    Id,
    Pid,
    OrgId,
    ServiceId,
    Title,
    Status,
    // Scope fields
    TargetSystems,
    IpRanges,
    Domains,
    Exclusions,
    TestWindowStart,
    TestWindowEnd,
    ContactName,
    ContactEmail,
    ContactPhone,
    RulesOfEngagement,
    // Admin fields
    RequestedAt,
    StartsAt,
    CompletedAt,
    PriceCents,
    Currency,
    AdminNotes,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
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
                    .table(Engagements::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Engagements::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Engagements::Pid).uuid().not_null())
                    .col(ColumnDef::new(Engagements::OrgId).integer().not_null())
                    .col(ColumnDef::new(Engagements::ServiceId).integer().not_null())
                    .col(ColumnDef::new(Engagements::Title).string().not_null())
                    .col(
                        ColumnDef::new(Engagements::Status)
                            .string()
                            .not_null()
                            .default("requested"),
                    )
                    // Scope fields
                    .col(ColumnDef::new(Engagements::TargetSystems).text().not_null())
                    .col(ColumnDef::new(Engagements::IpRanges).text().null())
                    .col(ColumnDef::new(Engagements::Domains).text().null())
                    .col(ColumnDef::new(Engagements::Exclusions).text().null())
                    .col(
                        ColumnDef::new(Engagements::TestWindowStart)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Engagements::TestWindowEnd)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(Engagements::ContactName).string().not_null())
                    .col(
                        ColumnDef::new(Engagements::ContactEmail)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Engagements::ContactPhone).string().null())
                    .col(ColumnDef::new(Engagements::RulesOfEngagement).text().null())
                    // Admin fields
                    .col(
                        ColumnDef::new(Engagements::RequestedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Engagements::StartsAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Engagements::CompletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(Engagements::PriceCents).integer().null())
                    .col(ColumnDef::new(Engagements::Currency).string().null())
                    .col(ColumnDef::new(Engagements::AdminNotes).text().null())
                    .col(
                        ColumnDef::new(Engagements::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Engagements::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagements-org_id")
                            .from(Engagements::Table, Engagements::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagements-service_id")
                            .from(Engagements::Table, Engagements::ServiceId)
                            .to(Services::Table, Services::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-engagements-pid")
                    .table(Engagements::Table)
                    .col(Engagements::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-engagements-org_id-status")
                    .table(Engagements::Table)
                    .col(Engagements::OrgId)
                    .col(Engagements::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Engagements::Table).to_owned())
            .await?;
        Ok(())
    }
}
