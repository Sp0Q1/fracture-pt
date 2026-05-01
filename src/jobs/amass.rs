//! `JobExecutor` for amass passive subdomain enumeration.
//!
//! Wraps [`crate::services::amass::run_passive`] in the standard executor
//! shape so the orchestrator and admin UI treat it like any other job.

use fracture_core::jobs::{JobExecutor, JobResult};
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::DatabaseConnection;

use crate::services::amass;

pub struct AmassExecutor;

#[async_trait::async_trait]
impl JobExecutor for AmassExecutor {
    #[allow(clippy::unnecessary_literal_bound)]
    fn job_type(&self) -> &str {
        "amass_passive"
    }

    async fn execute(
        &self,
        _db: &DatabaseConnection,
        definition: &job_definitions::Model,
        _previous_run: Option<&job_runs::Model>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        let config: serde_json::Value = serde_json::from_str(&definition.config)?;
        let domain = config["domain"]
            .as_str()
            .or_else(|| config["hostname"].as_str())
            .ok_or("amass_passive config missing required `domain`")?;

        let result = amass::run_passive(domain).await?;
        let summary = serde_json::to_value(&result)?;

        // Diff support is plumbed in a follow-up — the parser already
        // produces a stable, sorted hosts vector so subdomain_added /
        // subdomain_removed can be computed from successive runs.
        Ok(JobResult {
            summary,
            diffs: Vec::new(),
        })
    }
}
