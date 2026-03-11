use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Subscriptions {
    Table,
    Id,
    Pid,
    OrgId,
    TierId,
    Status,
    StartsAt,
    ExpiresAt,
    CancelledAt,
    StripeSubscriptionId,
    StripeCustomerId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum PricingTiers {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Subscriptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Subscriptions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Subscriptions::Pid).uuid().not_null())
                    .col(ColumnDef::new(Subscriptions::OrgId).integer().not_null())
                    .col(ColumnDef::new(Subscriptions::TierId).integer().not_null())
                    .col(
                        ColumnDef::new(Subscriptions::Status)
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::StartsAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::ExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::CancelledAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::StripeSubscriptionId)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::StripeCustomerId)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Subscriptions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-subscriptions-org_id")
                            .from(Subscriptions::Table, Subscriptions::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-subscriptions-tier_id")
                            .from(Subscriptions::Table, Subscriptions::TierId)
                            .to(PricingTiers::Table, PricingTiers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-subscriptions-pid")
                    .table(Subscriptions::Table)
                    .col(Subscriptions::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-subscriptions-org_id-status")
                    .table(Subscriptions::Table)
                    .col(Subscriptions::OrgId)
                    .col(Subscriptions::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Subscriptions::Table).to_owned())
            .await?;
        Ok(())
    }
}
