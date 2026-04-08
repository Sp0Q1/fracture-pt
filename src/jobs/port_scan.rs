use fracture_core::jobs::{JobDiff, JobExecutor, JobResult};
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};

use crate::models::_entities::findings;
use crate::services::port_scan::{self, PortScanResult};

pub struct PortScanExecutor;

#[async_trait::async_trait]
impl JobExecutor for PortScanExecutor {
    #[allow(clippy::unnecessary_literal_bound)]
    fn job_type(&self) -> &str {
        "port_scan"
    }

    async fn execute(
        &self,
        db: &DatabaseConnection,
        definition: &job_definitions::Model,
        previous_run: Option<&job_runs::Model>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        let config: serde_json::Value = serde_json::from_str(&definition.config)?;

        // Prefer hostname, fall back to IP
        let target = config["hostname"]
            .as_str()
            .or_else(|| config["ip_address"].as_str())
            .ok_or("missing hostname or ip_address in config")?
            .to_string();

        let result = port_scan::run_nmap(&target).await.map_err(|e| {
            Box::<dyn std::error::Error + Send + Sync>::from(format!("nmap failed: {e}"))
        })?;

        // Create version disclosure findings for services with detected versions
        create_version_findings(db, definition.org_id, &result, &target).await?;

        let diffs = compute_diffs(&result, previous_run);

        let summary = serde_json::json!({
            "target": result.target,
            "ip": result.ip,
            "total_open": result.total_open,
            "scan_time": result.scan_time,
            "os_guess": result.os_guess,
            "ports": result.ports,
        });

        Ok(JobResult { summary, diffs })
    }
}

/// Create findings for services that disclose version information.
/// Each unique service+version combination gets one finding per scan target.
async fn create_version_findings(
    db: &DatabaseConnection,
    org_id: i32,
    result: &PortScanResult,
    target: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let versioned_ports: Vec<_> = result
        .ports
        .iter()
        .filter(|p| p.state == "open" && !p.version.is_empty())
        .collect();

    if versioned_ports.is_empty() {
        return Ok(());
    }

    // Build a single finding summarizing all version disclosures for this target
    let mut description = format!(
        "The following services on **{}** ({}) disclose version information, \
         which aids attackers in identifying known vulnerabilities:\n\n",
        target, result.ip
    );
    for p in &versioned_ports {
        use std::fmt::Write;
        let _ = writeln!(
            description,
            "- **{}/{}** — {} {}",
            p.port, p.protocol, p.service, p.version
        );
    }
    description.push_str(
        "\nVersion disclosure allows attackers to search for CVEs specific to \
         the detected software versions.",
    );

    let recommendation = "Configure services to suppress version banners where possible:\n\n\
        - **Apache**: `ServerTokens Prod` and `ServerSignature Off`\n\
        - **Nginx**: `server_tokens off;`\n\
        - **SSH**: Not easily suppressible — keep software updated\n\
        - **SMTP**: Configure to show generic banners\n\n\
        Where version suppression is not possible, ensure all services \
        are patched to the latest stable release."
        .to_string();

    let title = format!(
        "Version Disclosure on {} ({} service{})",
        target,
        versioned_ports.len(),
        if versioned_ports.len() == 1 { "" } else { "s" }
    );

    findings::ActiveModel {
        org_id: Set(org_id),
        title: Set(title),
        description: Set(description),
        recommendation: Set(Some(recommendation)),
        severity: Set("low".to_string()),
        category: Set("misconfig".to_string()),
        affected_asset: Set(Some(format!("{}:{}", target, result.ip))),
        status: Set("new".to_string()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(())
}

fn compute_diffs(result: &PortScanResult, previous_run: Option<&job_runs::Model>) -> Vec<JobDiff> {
    let current_ports: std::collections::BTreeSet<String> = result
        .ports
        .iter()
        .filter(|p| p.state == "open")
        .map(|p| format!("{}/{}", p.port, p.protocol))
        .collect();

    let previous_ports: std::collections::BTreeSet<String> = previous_run
        .and_then(|run| run.result_summary.as_deref())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| {
            v["ports"].as_array().map(|arr| {
                arr.iter()
                    .filter(|p| p["state"].as_str() == Some("open"))
                    .filter_map(|p| {
                        let port = p["port"].as_u64()?;
                        let proto = p["protocol"].as_str()?;
                        Some(format!("{port}/{proto}"))
                    })
                    .collect()
            })
        })
        .unwrap_or_default();

    let mut diffs = Vec::new();

    for port in &current_ports {
        if !previous_ports.contains(port) {
            diffs.push(JobDiff {
                diff_type: "added".to_string(),
                entity_key: port.clone(),
                old_value: None,
                new_value: Some(serde_json::Value::String("open".to_string())),
            });
        }
    }

    for port in &previous_ports {
        if !current_ports.contains(port) {
            diffs.push(JobDiff {
                diff_type: "removed".to_string(),
                entity_key: port.clone(),
                old_value: Some(serde_json::Value::String("open".to_string())),
                new_value: Some(serde_json::Value::String("closed".to_string())),
            });
        }
    }

    diffs
}
