use async_trait::async_trait;
use loco_rs::{app::AppContext, bgworker::BackgroundWorker, Result};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};

use fracture_core::jobs::job_registry;
use fracture_core::models::_entities::{job_definitions, job_run_diffs, job_runs};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobDispatchArgs {
    pub job_run_id: i32,
    pub job_definition_id: i32,
}

pub struct JobDispatchWorker {
    pub ctx: AppContext,
}

#[async_trait]
impl BackgroundWorker<JobDispatchArgs> for JobDispatchWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: JobDispatchArgs) -> Result<()> {
        let db = &self.ctx.db;

        // Load the job definition
        let definition = job_definitions::Entity::find_by_id(args.job_definition_id)
            .one(db)
            .await?
            .ok_or_else(|| {
                loco_rs::Error::string(&format!(
                    "Job definition {} not found",
                    args.job_definition_id
                ))
            })?;

        // Look up the executor from the registry
        let registry = job_registry();
        let executor = registry.get(&definition.job_type).ok_or_else(|| {
            loco_rs::Error::string(&format!(
                "No executor registered for job type: {}",
                definition.job_type
            ))
        })?;

        // Mark run as "running"
        let run_update = job_runs::ActiveModel {
            id: Set(args.job_run_id),
            status: Set("running".to_string()),
            started_at: Set(Some(chrono::Utc::now().into())),
            ..Default::default()
        };
        run_update.update(db).await?;

        // Find previous completed run for diff comparison
        let previous_run =
            job_runs::Model::find_latest_completed_by_definition(db, args.job_definition_id).await;

        // Execute the job
        match executor
            .execute(db, &definition, previous_run.as_ref())
            .await
        {
            Ok(result) => {
                // Save diffs to job_run_diffs
                for diff in &result.diffs {
                    let diff_model = job_run_diffs::ActiveModel {
                        job_run_id: Set(args.job_run_id),
                        diff_type: Set(diff.diff_type.clone()),
                        entity_key: Set(diff.entity_key.clone()),
                        old_value: Set(diff.old_value.as_ref().map(ToString::to_string)),
                        new_value: Set(diff.new_value.as_ref().map(ToString::to_string)),
                        ..Default::default()
                    };
                    diff_model.insert(db).await?;
                }

                // Mark run as completed
                let run_update = job_runs::ActiveModel {
                    id: Set(args.job_run_id),
                    status: Set("completed".to_string()),
                    result_summary: Set(Some(result.summary.to_string())),
                    completed_at: Set(Some(chrono::Utc::now().into())),
                    ..Default::default()
                };
                run_update.update(db).await?;

                tracing::info!(
                    job_run_id = args.job_run_id,
                    job_type = %definition.job_type,
                    diffs = result.diffs.len(),
                    "Job completed successfully"
                );
            }
            Err(e) => {
                // Mark run as failed
                let run_update = job_runs::ActiveModel {
                    id: Set(args.job_run_id),
                    status: Set("failed".to_string()),
                    error_message: Set(Some(e.to_string())),
                    completed_at: Set(Some(chrono::Utc::now().into())),
                    ..Default::default()
                };
                run_update.update(db).await?;

                tracing::error!(
                    job_run_id = args.job_run_id,
                    job_type = %definition.job_type,
                    error = %e,
                    "Job execution failed"
                );
            }
        }

        Ok(())
    }
}
