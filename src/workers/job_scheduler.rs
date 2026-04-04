//! Periodic job scheduler.
//!
//! This worker is triggered on a schedule (e.g. every hour) and creates queued
//! runs for any enabled job definition that has a `schedule` set and whose org
//! tier permits auto-scheduling.

use async_trait::async_trait;
use loco_rs::{app::AppContext, bgworker::BackgroundWorker, Result};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use fracture_core::models::_entities::{job_definitions, job_runs, organizations};

use crate::services::tier::PlanTier;
use crate::workers::job_dispatcher::{JobDispatchArgs, JobDispatchWorker};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobSchedulerArgs {}

pub struct JobSchedulerWorker {
    pub ctx: AppContext,
}

#[async_trait]
impl BackgroundWorker<JobSchedulerArgs> for JobSchedulerWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, _args: JobSchedulerArgs) -> Result<()> {
        let db = &self.ctx.db;

        // Find all enabled definitions that have a schedule
        let definitions = job_definitions::Entity::find()
            .filter(job_definitions::Column::Enabled.eq(true))
            .filter(job_definitions::Column::Schedule.is_not_null())
            .all(db)
            .await?;

        for def in definitions {
            // Check if there is already a queued or running run
            let pending = job_runs::Entity::find()
                .filter(job_runs::Column::JobDefinitionId.eq(def.id))
                .filter(
                    job_runs::Column::Status
                        .eq("queued")
                        .or(job_runs::Column::Status.eq("running")),
                )
                .one(db)
                .await?;

            if pending.is_some() {
                continue;
            }

            // Load the org to check tier
            let org = organizations::Entity::find_by_id(def.org_id)
                .one(db)
                .await?;

            let Some(org) = org else {
                tracing::warn!(
                    job_definition_id = def.id,
                    "Skipping scheduled job: org not found"
                );
                continue;
            };

            let tier = PlanTier::from_org(&org);
            if !tier.scheduling_enabled() {
                tracing::debug!(
                    job_definition_id = def.id,
                    org_id = def.org_id,
                    tier = tier.label(),
                    "Skipping scheduled job: tier does not allow scheduling"
                );
                continue;
            }

            // Create a queued run and dispatch
            let run = job_runs::Model::create_queued(db, def.id, def.org_id).await?;

            JobDispatchWorker::perform_later(
                &self.ctx,
                JobDispatchArgs {
                    job_run_id: run.id,
                    job_definition_id: def.id,
                },
            )
            .await?;

            tracing::info!(
                job_definition_id = def.id,
                job_run_id = run.id,
                job_type = %def.job_type,
                "Scheduled job dispatched"
            );
        }

        Ok(())
    }
}
