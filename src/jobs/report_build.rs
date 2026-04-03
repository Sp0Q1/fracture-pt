use fracture_core::jobs::{JobDiff, JobExecutor, JobResult};
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::{DatabaseConnection, EntityTrait};

use crate::models::findings;
use crate::models::non_findings;
use crate::services::report_builder;

pub struct ReportBuildExecutor;

#[async_trait::async_trait]
impl JobExecutor for ReportBuildExecutor {
    #[allow(clippy::unnecessary_literal_bound)]
    fn job_type(&self) -> &str {
        "report_build"
    }

    async fn execute(
        &self,
        db: &DatabaseConnection,
        definition: &job_definitions::Model,
        _previous_run: Option<&job_runs::Model>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        let config: serde_json::Value = serde_json::from_str(&definition.config)?;
        let engagement_id = i32::try_from(
            config["engagement_id"]
                .as_i64()
                .ok_or("missing engagement_id in config")?,
        )?;

        let engagement = crate::models::_entities::engagements::Entity::find_by_id(engagement_id)
            .one(db)
            .await?
            .ok_or("engagement not found")?;

        let engagement_findings = findings::Model::find_by_engagement(db, engagement.id).await;
        let engagement_non_findings =
            non_findings::Model::find_by_engagement(db, engagement.id).await;

        let path = report_builder::generate_report(
            db,
            &engagement,
            &engagement_findings,
            &engagement_non_findings,
        )
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

        let summary = serde_json::json!({
            "engagement_id": engagement_id,
            "engagement_title": engagement.title,
            "finding_count": engagement_findings.len(),
            "non_finding_count": engagement_non_findings.len(),
            "pdf_path": path,
        });

        Ok(JobResult {
            summary,
            diffs: vec![JobDiff {
                diff_type: "added".to_string(),
                entity_key: "report".to_string(),
                old_value: None,
                new_value: Some(serde_json::json!(path)),
            }],
        })
    }
}
