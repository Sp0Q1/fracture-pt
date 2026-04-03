use std::fmt::Write as _;

use async_trait::async_trait;
use loco_rs::{app::AppContext, bgworker::BackgroundWorker, Result};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};

use crate::models::_entities::{findings, scan_jobs};
use crate::services::asm::{self, ScanResult};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AsmScanArgs {
    pub target_id: i32,
    pub org_id: i32,
    pub hostname: String,
    pub job_id: i32,
}

pub struct AsmScanWorker {
    pub ctx: AppContext,
}

/// Create aggregated findings for expired and wildcard certificates.
/// Instead of one finding per cert, we create at most one finding per category
/// with a summary description listing the affected certificates.
async fn create_cert_findings(
    db: &DatabaseConnection,
    result: &ScanResult,
    org_id: i32,
    job_id: i32,
    hostname: &str,
) -> Result<i32> {
    let mut count = 0i32;

    // Collect wildcard certs
    let wildcards: Vec<&asm::CertInfo> = result.certs.iter().filter(|c| c.is_wildcard).collect();
    if !wildcards.is_empty() {
        let detail_lines: Vec<String> = wildcards
            .iter()
            .take(10)
            .map(|c| {
                format!(
                    "  \u{2022} {} (issuer: {}, valid until: {})",
                    c.common_name, c.issuer, c.not_after
                )
            })
            .collect();
        let mut description = format!(
            "{} wildcard certificate{} found for {}. \
             Wildcard certificates increase blast radius if compromised \
             and may mask shadow subdomains.\n\n{}",
            wildcards.len(),
            if wildcards.len() == 1 { "" } else { "s" },
            hostname,
            detail_lines.join("\n"),
        );
        if wildcards.len() > 10 {
            let _ = write!(
                description,
                "\n  \u{2026} and {} more",
                wildcards.len() - 10
            );
        }

        let finding = findings::ActiveModel {
            org_id: Set(org_id),
            job_id: Set(Some(job_id)),
            title: Set(format!(
                "{} Wildcard Certificate{}",
                wildcards.len(),
                if wildcards.len() == 1 { "" } else { "s" }
            )),
            description: Set(description),
            severity: Set("Low".to_string()),
            category: Set("certificate".to_string()),
            affected_asset: Set(Some(hostname.to_string())),
            status: Set("open".to_string()),
            ..Default::default()
        };
        finding.insert(db).await?;
        count += 1;
    }

    Ok(count)
}

#[async_trait]
impl BackgroundWorker<AsmScanArgs> for AsmScanWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }

    async fn perform(&self, args: AsmScanArgs) -> Result<()> {
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY_SECS: u64 = 10;

        let db = &self.ctx.db;
        tracing::info!(
            target_id = args.target_id,
            hostname = %args.hostname,
            "Starting ASM scan"
        );

        // Mark job as running
        let job = scan_jobs::ActiveModel {
            id: Set(args.job_id),
            status: Set("running".to_string()),
            started_at: Set(Some(chrono::Utc::now().into())),
            updated_at: Set(chrono::Utc::now().into()),
            ..Default::default()
        };
        job.update(db).await?;

        // Run the crt.sh scan with retries
        let mut last_error = String::new();
        for attempt in 1..=MAX_RETRIES {
            match asm::query_crtsh(&args.hostname).await {
                Ok(entries) => {
                    let result = asm::process_results(&args.hostname, &entries);
                    let finding_count = create_cert_findings(
                        db,
                        &result,
                        args.org_id,
                        args.job_id,
                        &args.hostname,
                    )
                    .await?;

                    let summary = serde_json::json!({
                        "domain": result.domain,
                        "subdomain_count": result.subdomain_count,
                        "cert_count": result.cert_count,
                        "expired_count": result.expired_count,
                        "wildcard_count": result.wildcard_count,
                        "subdomains": result.subdomains,
                        "certs": result.certs,
                    });

                    let job = scan_jobs::ActiveModel {
                        id: Set(args.job_id),
                        status: Set("completed".to_string()),
                        finding_count: Set(finding_count),
                        result_summary: Set(Some(summary.to_string())),
                        completed_at: Set(Some(chrono::Utc::now().into())),
                        updated_at: Set(chrono::Utc::now().into()),
                        ..Default::default()
                    };
                    job.update(db).await?;

                    tracing::info!(
                        target_id = args.target_id,
                        subdomains = result.subdomain_count,
                        certs = result.cert_count,
                        findings = finding_count,
                        attempt,
                        "ASM scan completed"
                    );
                    return Ok(());
                }
                Err(e) => {
                    last_error = e;
                    if attempt < MAX_RETRIES {
                        tracing::warn!(
                            target_id = args.target_id,
                            error = %last_error,
                            attempt,
                            max_retries = MAX_RETRIES,
                            "ASM scan failed, retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(
                            RETRY_DELAY_SECS * u64::from(attempt),
                        ))
                        .await;
                    }
                }
            }
        }

        // All retries exhausted
        tracing::error!(
            target_id = args.target_id,
            error = %last_error,
            "ASM scan failed after {MAX_RETRIES} attempts"
        );
        let job = scan_jobs::ActiveModel {
            id: Set(args.job_id),
            status: Set("failed".to_string()),
            error_message: Set(Some(last_error)),
            completed_at: Set(Some(chrono::Utc::now().into())),
            updated_at: Set(chrono::Utc::now().into()),
            ..Default::default()
        };
        job.update(db).await?;

        Ok(())
    }
}
