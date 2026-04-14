use async_trait::async_trait;
use loco_rs::{app::AppContext, bgworker::BackgroundWorker, Result};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QuerySelect,
};
use serde::{Deserialize, Serialize};

use fracture_core::jobs::{job_registry, JobDiff};
use fracture_core::models::_entities::{job_definitions, job_run_diffs, job_runs, organizations};

use crate::mailers::scan_alert::ScanAlertMailer;
use crate::services::tier::PlanTier;

/// Basic email validation without external dependencies.
fn is_valid_email(s: &str) -> bool {
    let parts: Vec<&str> = s.splitn(2, '@').collect();
    if parts.len() != 2 {
        return false;
    }
    let (local, domain) = (parts[0], parts[1]);
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !s.contains(char::is_whitespace)
        && !s.contains('\n')
        && !s.contains('\r')
}

/// Strip newlines and control characters from a string (for safe use in emails).
fn sanitize_for_email(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobDispatchArgs {
    pub job_run_id: i32,
    pub job_definition_id: i32,
}

pub struct JobDispatchWorker {
    pub ctx: AppContext,
}

impl JobDispatchWorker {
    /// Dispatch any queued runs that were created during job execution
    /// (e.g. subdomain port scans created by the ASM executor).
    async fn dispatch_queued_runs(&self) {
        let db = &self.ctx.db;

        let queued_runs = match job_runs::Entity::find()
            .filter(job_runs::Column::Status.eq("queued"))
            .filter(job_runs::Column::StartedAt.is_null())
            .limit(50)
            .all(db)
            .await
        {
            Ok(runs) => runs,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to query queued runs");
                return;
            }
        };

        for run in queued_runs {
            if let Err(e) = Self::perform_later(
                &self.ctx,
                JobDispatchArgs {
                    job_run_id: run.id,
                    job_definition_id: run.job_definition_id,
                },
            )
            .await
            {
                tracing::warn!(
                    job_run_id = run.id,
                    error = %e,
                    "Failed to dispatch queued run"
                );
            }
        }
    }

    /// Fire-and-forget email alerts for job diffs.
    async fn send_diff_alerts(&self, definition: &job_definitions::Model, diffs: &[JobDiff]) {
        let db = &self.ctx.db;

        // Load the org
        let Ok(Some(org)) = organizations::Entity::find_by_id(definition.org_id)
            .one(db)
            .await
        else {
            tracing::warn!(
                job_definition_id = definition.id,
                org_id = definition.org_id,
                "Cannot send diff alerts: org not found"
            );
            return;
        };

        let tier = PlanTier::from_org(&org);
        if !tier.email_alerts_enabled() {
            tracing::warn!(
                job_definition_id = definition.id,
                org_id = definition.org_id,
                "Skipping diff alerts: email alerts not enabled for tier"
            );
            return;
        }

        // Read alert_emails from org settings
        let emails_str = org
            .get_setting("alert_emails")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        if emails_str.trim().is_empty() {
            tracing::warn!(
                job_definition_id = definition.id,
                org_id = definition.org_id,
                "Skipping diff alerts: no alert_emails configured"
            );
            return;
        }

        // Extract domain from config and sanitize for safe email use
        let domain = serde_json::from_str::<serde_json::Value>(&definition.config)
            .ok()
            .and_then(|c| c["hostname"].as_str().map(String::from))
            .unwrap_or_else(|| definition.name.clone());
        let domain = sanitize_for_email(&domain);

        let recipients: Vec<&str> = emails_str
            .lines()
            .map(str::trim)
            .filter(|s| is_valid_email(s))
            .collect();

        for email in recipients {
            if let Err(e) =
                ScanAlertMailer::send_alert(&self.ctx, email, &definition.name, &domain, diffs)
                    .await
            {
                tracing::error!(
                    email = %email,
                    job_definition_id = definition.id,
                    error = %e,
                    "Failed to send scan alert email"
                );
            }
        }
    }
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
            job_runs::Model::find_latest_completed_by_definition(db, args.job_definition_id).await?;

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

                // Send email alerts if diffs are non-empty
                if !result.diffs.is_empty() {
                    self.send_diff_alerts(&definition, &result.diffs).await;
                }

                // Dispatch any newly queued runs (e.g. subdomain port scans)
                self.dispatch_queued_runs().await;
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
