#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::missing_errors_doc)]
pub use sea_orm_migration::prelude::*;

mod m20260310_000001_create_services;
mod m20260310_000002_create_pricing_tiers;
mod m20260310_000003_create_subscriptions;
mod m20260310_000004_create_scan_targets;
mod m20260310_000005_create_engagements;
mod m20260310_000006_create_engagement_offers;
mod m20260310_000007_create_pentester_assignments;
mod m20260310_000008_create_scan_jobs;
mod m20260310_000009_create_findings;
mod m20260310_000010_create_reports;
mod m20260310_000011_create_invoices;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        fracture_core_migration::Migrator::migrations()
            .into_iter()
            .chain(vec![
                Box::new(m20260310_000001_create_services::Migration) as Box<dyn MigrationTrait>,
                Box::new(m20260310_000002_create_pricing_tiers::Migration),
                Box::new(m20260310_000003_create_subscriptions::Migration),
                Box::new(m20260310_000004_create_scan_targets::Migration),
                Box::new(m20260310_000005_create_engagements::Migration),
                Box::new(m20260310_000006_create_engagement_offers::Migration),
                Box::new(m20260310_000007_create_pentester_assignments::Migration),
                Box::new(m20260310_000008_create_scan_jobs::Migration),
                Box::new(m20260310_000009_create_findings::Migration),
                Box::new(m20260310_000010_create_reports::Migration),
                Box::new(m20260310_000011_create_invoices::Migration),
            ])
            .collect()
    }
}
