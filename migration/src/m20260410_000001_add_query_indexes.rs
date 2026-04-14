use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Reports {
    Table,
    OrgId,
    EngagementId,
}

#[derive(DeriveIden)]
enum Invoices {
    Table,
    OrgId,
}

#[derive(DeriveIden)]
enum PricingTiers {
    Table,
    ServiceId,
}

#[derive(DeriveIden)]
enum EngagementTargets {
    Table,
    ScanTargetId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // reports.org_id — used by find_by_org queries
        manager
            .create_index(
                Index::create()
                    .name("idx-reports-org_id")
                    .table(Reports::Table)
                    .col(Reports::OrgId)
                    .to_owned(),
            )
            .await?;

        // reports.engagement_id — used by find_by_engagement queries
        manager
            .create_index(
                Index::create()
                    .name("idx-reports-engagement_id")
                    .table(Reports::Table)
                    .col(Reports::EngagementId)
                    .to_owned(),
            )
            .await?;

        // invoices.org_id — used by find_by_org queries
        manager
            .create_index(
                Index::create()
                    .name("idx-invoices-org_id")
                    .table(Invoices::Table)
                    .col(Invoices::OrgId)
                    .to_owned(),
            )
            .await?;

        // pricing_tiers.service_id — used by find_by_service queries
        manager
            .create_index(
                Index::create()
                    .name("idx-pricing_tiers-service_id")
                    .table(PricingTiers::Table)
                    .col(PricingTiers::ServiceId)
                    .to_owned(),
            )
            .await?;

        // engagement_targets.scan_target_id — for reverse lookups
        manager
            .create_index(
                Index::create()
                    .name("idx-engagement_targets-scan_target_id")
                    .table(EngagementTargets::Table)
                    .col(EngagementTargets::ScanTargetId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-engagement_targets-scan_target_id")
                    .table(EngagementTargets::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-pricing_tiers-service_id")
                    .table(PricingTiers::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-invoices-org_id")
                    .table(Invoices::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-reports-engagement_id")
                    .table(Reports::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-reports-org_id")
                    .table(Reports::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
