use std::collections::BTreeSet;
use std::fmt::Write as _;

use fracture_core::jobs::{JobDiff, JobExecutor, JobResult};
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

use crate::models::_entities::findings;
use crate::services::asm::{self, ScanResult, SubdomainInfo};

const MAX_RETRIES: u32 = 3;

pub struct AsmScanExecutor;

#[async_trait::async_trait]
impl JobExecutor for AsmScanExecutor {
    #[allow(clippy::unnecessary_literal_bound)]
    fn job_type(&self) -> &str {
        "asm_scan"
    }

    async fn execute(
        &self,
        db: &DatabaseConnection,
        definition: &job_definitions::Model,
        previous_run: Option<&job_runs::Model>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        let config: serde_json::Value = serde_json::from_str(&definition.config)?;
        let hostname = config["hostname"]
            .as_str()
            .ok_or("missing hostname in config")?
            .to_string();
        let org_id = i32::try_from(
            config["org_id"]
                .as_i64()
                .ok_or("missing org_id in config")?,
        )?;

        // Query crt.sh with retry logic
        let mut last_error = String::new();
        let mut result_entries = None;
        for attempt in 1..=MAX_RETRIES {
            match asm::query_crtsh(&hostname).await {
                Ok(entries) => {
                    result_entries = Some(entries);
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
                        tokio::time::sleep(std::time::Duration::from_secs(10 * u64::from(attempt)))
                            .await;
                    }
                }
            }
        }
        let entries = result_entries.ok_or_else(|| {
            format!("crt.sh query failed after {MAX_RETRIES} attempts: {last_error}")
        })?;

        let scan_result = asm::process_results(&hostname, &entries);

        // Resolve subdomains via DNS
        let subdomain_infos = asm::resolve_subdomains(&scan_result.subdomains).await;

        create_cert_findings(db, &scan_result, org_id, &hostname).await?;

        let diffs = compute_diffs(&subdomain_infos, previous_run);

        let summary = serde_json::json!({
            "domain": scan_result.domain,
            "subdomain_count": scan_result.subdomain_count,
            "cert_count": scan_result.cert_count,
            "expired_count": scan_result.expired_count,
            "wildcard_count": scan_result.wildcard_count,
            "subdomains": subdomain_infos,
            "certs": scan_result.certs,
        });

        // Enqueue port scans for unique resolved IPs (deduplicated).
        let mut scanned_ips = std::collections::HashSet::new();
        for info in &subdomain_infos {
            if info.resolved {
                for ip in &info.ips {
                    if scanned_ips.insert(ip.clone()) {
                        if let Err(e) = enqueue_ip_port_scan(db, org_id, &info.name, ip).await {
                            tracing::warn!(
                                subdomain = %info.name, ip = %ip, error = %e,
                                "Failed to enqueue IP port scan"
                            );
                        }
                    }
                }
            }
        }

        Ok(JobResult { summary, diffs })
    }
}

/// Extract subdomain names from the previous run summary.
/// Supports both the new `SubdomainInfo` format (objects with `name` key)
/// and the legacy plain-string format.
fn extract_previous_subdomains(previous_run: Option<&job_runs::Model>) -> BTreeSet<String> {
    previous_run
        .and_then(|run| run.result_summary.as_deref())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| {
            v["subdomains"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        // New format: { "name": "sub.example.com", "resolved": true, ... }
                        v["name"]
                            .as_str()
                            .map(String::from)
                            // Legacy format: plain string
                            .or_else(|| v.as_str().map(String::from))
                    })
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Build a map of subdomain -> resolved status from the previous run.
fn extract_previous_resolution(
    previous_run: Option<&job_runs::Model>,
) -> std::collections::BTreeMap<String, bool> {
    previous_run
        .and_then(|run| run.result_summary.as_deref())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| {
            v["subdomains"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let name = v["name"].as_str()?;
                        let resolved = v["resolved"].as_bool().unwrap_or(false);
                        Some((name.to_string(), resolved))
                    })
                    .collect()
            })
        })
        .unwrap_or_default()
}

fn compute_diffs(
    current_infos: &[SubdomainInfo],
    previous_run: Option<&job_runs::Model>,
) -> Vec<JobDiff> {
    let current_subdomains: BTreeSet<&str> =
        current_infos.iter().map(|i| i.name.as_str()).collect();

    let previous_subdomains = extract_previous_subdomains(previous_run);
    let previous_resolution = extract_previous_resolution(previous_run);

    let mut diffs = Vec::new();

    for info in current_infos {
        if previous_subdomains.contains(&info.name) {
            // Check for resolution status changes
            let was_resolved = previous_resolution
                .get(&info.name)
                .copied()
                .unwrap_or(false);
            if info.resolved && !was_resolved {
                diffs.push(JobDiff {
                    diff_type: "subdomain_resolved".to_string(),
                    entity_key: info.name.clone(),
                    old_value: Some(serde_json::json!("unresolved")),
                    new_value: Some(serde_json::json!({
                        "resolved": true,
                        "ips": info.ips,
                    })),
                });
            } else if !info.resolved && was_resolved {
                diffs.push(JobDiff {
                    diff_type: "subdomain_unresolved".to_string(),
                    entity_key: info.name.clone(),
                    old_value: Some(serde_json::json!("resolved")),
                    new_value: Some(serde_json::json!("unresolved")),
                });
            }
        } else {
            diffs.push(JobDiff {
                diff_type: "subdomain_added".to_string(),
                entity_key: info.name.clone(),
                old_value: None,
                new_value: Some(serde_json::json!({
                    "resolved": info.resolved,
                    "ips": info.ips,
                })),
            });
        }
    }

    for sub in &previous_subdomains {
        if !current_subdomains.contains(sub.as_str()) {
            diffs.push(JobDiff {
                diff_type: "subdomain_removed".to_string(),
                entity_key: sub.clone(),
                old_value: None,
                new_value: None,
            });
        }
    }

    diffs
}

/// Enqueue a port scan for a specific IP address (associated with a subdomain).
async fn enqueue_ip_port_scan(
    db: &sea_orm::DatabaseConnection,
    org_id: i32,
    subdomain: &str,
    ip: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let def_name = format!("Port Scan: {ip} ({subdomain})");

    let definition = if let Some(existing) = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_id))
        .filter(job_definitions::Column::Name.eq(&def_name))
        .one(db)
        .await?
    {
        existing
    } else {
        let config = serde_json::json!({
            "hostname": ip,
            "org_id": org_id,
            "source": "asm_resolved_ip",
            "subdomain": subdomain,
        });
        job_definitions::ActiveModel {
            org_id: Set(org_id),
            name: Set(def_name),
            job_type: Set("port_scan".to_string()),
            config: Set(config.to_string()),
            enabled: Set(true),
            ..Default::default()
        }
        .insert(db)
        .await?
    };

    // Check for existing queued/running jobs
    let pending = job_runs::Entity::find()
        .filter(job_runs::Column::JobDefinitionId.eq(definition.id))
        .filter(
            job_runs::Column::Status
                .eq("queued")
                .or(job_runs::Column::Status.eq("running")),
        )
        .one(db)
        .await?;

    if let Some(run) = pending {
        // If running >10 minutes, mark as failed (stuck) so we can re-enqueue
        let is_stuck = run.started_at.as_ref().is_some_and(|started| {
            let age = chrono::Utc::now() - started.with_timezone(&chrono::Utc);
            age.num_minutes() > 10
        });
        if is_stuck {
            let mut active: job_runs::ActiveModel = run.into();
            active.status = Set("failed".to_string());
            active.error_message = Set(Some("Timed out after 10 minutes".to_string()));
            active.completed_at = Set(Some(chrono::Utc::now().into()));
            active.update(db).await?;
            tracing::warn!(ip = %ip, "Marked stuck port scan as failed, re-enqueuing");
        } else {
            return Ok(());
        }
    }

    let _run = job_runs::Model::create_queued(db, definition.id, org_id).await?;

    tracing::info!(
        ip = %ip, subdomain = %subdomain,
        "Queued port scan for resolved IP"
    );

    Ok(())
}

async fn create_cert_findings(
    db: &DatabaseConnection,
    result: &ScanResult,
    org_id: i32,
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

        findings::ActiveModel {
            org_id: Set(org_id),
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
        }
        .insert(db)
        .await?;
        count += 1;
    }

    Ok(count)
}
