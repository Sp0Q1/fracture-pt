use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Invoices {
    Table,
    Id,
    Pid,
    OrgId,
    SubscriptionId,
    EngagementId,
    AmountCents,
    Currency,
    Status,
    IssuedAt,
    DueAt,
    PaidAt,
    StripeInvoiceId,
    PdfPath,
    LineItems,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Subscriptions {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Engagements {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Invoices::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Invoices::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Invoices::Pid).uuid().not_null())
                    .col(ColumnDef::new(Invoices::OrgId).integer().not_null())
                    .col(ColumnDef::new(Invoices::SubscriptionId).integer().null())
                    .col(ColumnDef::new(Invoices::EngagementId).integer().null())
                    .col(
                        ColumnDef::new(Invoices::AmountCents)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Invoices::Currency)
                            .string()
                            .not_null()
                            .default("EUR"),
                    )
                    .col(
                        ColumnDef::new(Invoices::Status)
                            .string()
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(Invoices::IssuedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Invoices::DueAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Invoices::PaidAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(Invoices::StripeInvoiceId).string().null())
                    .col(ColumnDef::new(Invoices::PdfPath).string().null())
                    .col(ColumnDef::new(Invoices::LineItems).text().null())
                    .col(
                        ColumnDef::new(Invoices::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Invoices::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-invoices-org_id")
                            .from(Invoices::Table, Invoices::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-invoices-subscription_id")
                            .from(Invoices::Table, Invoices::SubscriptionId)
                            .to(Subscriptions::Table, Subscriptions::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-invoices-engagement_id")
                            .from(Invoices::Table, Invoices::EngagementId)
                            .to(Engagements::Table, Engagements::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-invoices-pid")
                    .table(Invoices::Table)
                    .col(Invoices::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Invoices::Table).to_owned())
            .await?;
        Ok(())
    }
}
