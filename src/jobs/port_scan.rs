use fracture_core::jobs::{JobDiff, JobExecutor, JobResult};
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::DatabaseConnection;

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
        _db: &DatabaseConnection,
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
