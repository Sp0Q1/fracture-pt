use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum PricingTiers {
    Table,
    Id,
    Pid,
    ServiceId,
    Name,
    Slug,
    PriceCents,
    BillingPeriod,
    MaxTargets,
    MaxScansPerMonth,
    Features,
    IsActive,
    SortOrder,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Services {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PricingTiers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PricingTiers::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PricingTiers::Pid).uuid().not_null())
                    .col(ColumnDef::new(PricingTiers::ServiceId).integer().not_null())
                    .col(ColumnDef::new(PricingTiers::Name).string().not_null())
                    .col(ColumnDef::new(PricingTiers::Slug).string().not_null())
                    .col(
                        ColumnDef::new(PricingTiers::PriceCents)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PricingTiers::BillingPeriod)
                            .string()
                            .not_null()
                            .default("monthly"),
                    )
                    .col(
                        ColumnDef::new(PricingTiers::MaxTargets)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PricingTiers::MaxScansPerMonth)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(PricingTiers::Features).text().not_null())
                    .col(
                        ColumnDef::new(PricingTiers::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(PricingTiers::SortOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PricingTiers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PricingTiers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-pricing_tiers-service_id")
                            .from(PricingTiers::Table, PricingTiers::ServiceId)
                            .to(Services::Table, Services::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-pricing_tiers-pid")
                    .table(PricingTiers::Table)
                    .col(PricingTiers::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PricingTiers::Table).to_owned())
            .await?;
        Ok(())
    }
}
