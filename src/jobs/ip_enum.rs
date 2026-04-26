//! Job executor for passive per-IP enumeration.
//!
//! Triggered by the [`crate::services::pipeline`] orchestrator after an
//! `asm_scan` resolves subdomains to IPs. Runs only passive sources today
//! (PTR), with ASN / RDAP / geo planned. Persists structured intel in
//! `result_summary` for the UI to render.

use fracture_core::jobs::{JobExecutor, JobResult};
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::DatabaseConnection;

use crate::services::ip_enum;

pub struct IpEnumExecutor;

#[async_trait::async_trait]
impl JobExecutor for IpEnumExecutor {
    #[allow(clippy::unnecessary_literal_bound)]
    fn job_type(&self) -> &str {
        "ip_enum"
    }

    async fn execute(
        &self,
        _db: &DatabaseConnection,
        definition: &job_definitions::Model,
        _previous_run: Option<&job_runs::Model>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        let config: serde_json::Value = serde_json::from_str(&definition.config)?;
        let ip = config["ip"]
            .as_str()
            .ok_or("ip_enum config missing required `ip`")?;

        let result = ip_enum::enumerate(ip).await;

        let summary = serde_json::to_value(&result)?;

        // No diffs yet — when we record state per-IP across runs, we'll surface
        // PTR / ASN changes here.
        Ok(JobResult {
            summary,
            diffs: Vec::new(),
        })
    }
}
