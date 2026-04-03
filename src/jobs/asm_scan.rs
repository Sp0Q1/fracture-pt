use std::collections::BTreeSet;
use std::fmt::Write as _;

use fracture_core::jobs::{JobDiff, JobExecutor, JobResult};
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};

use crate::models::_entities::findings;
use crate::services::asm::{self, ScanResult};

pub struct AsmScanExecutor;

#[async_trait::async_trait]
impl JobExecutor for AsmScanExecutor {
    fn job_type(&self) -> &str {
        "asm_scan"
    }

    async fn execute(
        &self,
        db: &DatabaseConnection,
        definition: &job_definitions::Model,
        previous_run: Option<&job_runs::Model>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        // Parse config
        let config: serde_json::Value = serde_json::from_str(&definition.config)?;
        let hostname = config["hostname"]
            .as_str()
            .ok_or("missing hostname in config")?
            .to_string();
        let org_id = config["org_id"]
            .as_i64()
            .ok_or("missing org_id in config")? as i32;

        // Query crt.sh with retry logic
        const MAX_RETRIES: u32 = 3;
        let mut last_error = String::new();

        let entries = {
            let mut result = None;
            for attempt in 1..=MAX_RETRIES {
                match asm::query_crtsh(&hostname).await {
                    Ok(entries) => {
                        result = Some(entries);
                        break;
                    }
                    Err(e) => {
                        last_error = e;
                        if attempt < MAX_RETRIES {
                            tracing::warn!(
                                hostname = %hostname,
                                error = %last_error,
                                attempt,
                                "ASM scan failed, retrying"
                            );
                            tokio::time::sleep(std::time::Duration::from_secs(
                                10 * u64::from(attempt),
                            ))
                            .await;
                        }
                    }
                }
            }
            result.ok_or_else(|| {
                format!("crt.sh query failed after {MAX_RETRIES} attempts: {last_error}")
            })?
        };

        let scan_result = asm::process_results(&hostname, &entries);

        // Create cert findings
        create_cert_findings(db, &scan_result, org_id, definition.id, &hostname).await?;

        // Compute diffs against previous run
        let diffs = compute_diffs(&scan_result, previous_run);

        // Build summary JSON (same format as scan_jobs.result_summary)
        let summary = serde_json::json!({
            "domain": scan_result.domain,
            "subdomain_count": scan_result.subdomain_count,
            "cert_count": scan_result.cert_count,
            "expired_count": scan_result.expired_count,
            "wildcard_count": scan_result.wildcard_count,
            "subdomains": scan_result.subdomains,
            "certs": scan_result.certs,
        });

        Ok(JobResult { summary, diffs })
    }
}

fn compute_diffs(result: &ScanResult, previous_run: Option<&job_runs::Model>) -> Vec<JobDiff> {
    let current_subdomains: BTreeSet<&str> = result.subdomains.iter().map(String::as_str).collect();

    let previous_subdomains: BTreeSet<String> = previous_run
        .and_then(|run| run.result_summary.as_deref())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| {
            v["subdomains"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
        })
        .unwrap_or_default();

    let mut diffs = Vec::new();

    // Added subdomains
    for sub in &current_subdomains {
        if !previous_subdomains.contains(*sub) {
            diffs.push(JobDiff {
                diff_type: "added".to_string(),
                entity_key: sub.to_string(),
                old_value: None,
                new_value: None,
            });
        }
    }

    // Removed subdomains
    for sub in &previous_subdomains {
        if !current_subdomains.contains(sub.as_str()) {
            diffs.push(JobDiff {
                diff_type: "removed".to_string(),
                entity_key: sub.clone(),
                old_value: None,
                new_value: None,
            });
        }
    }

    diffs
}

/// Create aggregated findings for wildcard certificates.
async fn create_cert_findings(
    db: &DatabaseConnection,
    result: &ScanResult,
    org_id: i32,
    job_id: i32,
    hostname: &str,
) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let mut count = 0i32;

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
