use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum EngagementOffers {
    Table,
    Id,
    Pid,
    EngagementId,
    CreatedByUserId,
    AmountCents,
    Currency,
    TimelineDays,
    Deliverables,
    Terms,
    ValidUntil,
    Status,
    ClientResponse,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Engagements {
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
                    .table(EngagementOffers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EngagementOffers::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(EngagementOffers::Pid).uuid().not_null())
                    .col(
                        ColumnDef::new(EngagementOffers::EngagementId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::CreatedByUserId)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::AmountCents)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::Currency)
                            .string()
                            .not_null()
                            .default("EUR"),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::TimelineDays)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::Deliverables)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(EngagementOffers::Terms).text().null())
                    .col(
                        ColumnDef::new(EngagementOffers::ValidUntil)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::ClientResponse)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(EngagementOffers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagement_offers-engagement_id")
                            .from(EngagementOffers::Table, EngagementOffers::EngagementId)
                            .to(Engagements::Table, Engagements::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagement_offers-created_by_user_id")
                            .from(EngagementOffers::Table, EngagementOffers::CreatedByUserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-engagement_offers-pid")
                    .table(EngagementOffers::Table)
                    .col(EngagementOffers::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-engagement_offers-engagement_id-status")
                    .table(EngagementOffers::Table)
                    .col(EngagementOffers::EngagementId)
                    .col(EngagementOffers::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EngagementOffers::Table).to_owned())
            .await?;
        Ok(())
    }
}
