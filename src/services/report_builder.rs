use loco_rs::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::_entities::{engagements, findings, non_findings, reports};
use sea_orm::ActiveValue::Set;

const DOCBUILDER_IMAGE: &str = "pentext-docbuilder:latest";

/// Map gethacked severity string to PenText `threatLevel` string.
fn map_threat_level(severity: &str) -> &str {
    match severity {
        "extreme" => "Extreme",
        "high" => "High",
        "elevated" => "Elevated",
        "moderate" => "Moderate",
        _ => "Low",
    }
}

/// Escape XML special characters in text content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Collect engagement targets from target_systems and domains fields.
fn collect_targets(engagement: &engagements::Model) -> Vec<String> {
    let mut targets = Vec::new();
    for t in engagement.target_systems.lines() {
        let t = t.trim();
        if !t.is_empty() {
            targets.push(t.to_string());
        }
    }
    if let Some(ref domains) = engagement.domains {
        for d in domains.lines() {
            let d = d.trim();
            if !d.is_empty() && !targets.contains(&d.to_string()) {
                targets.push(d.to_string());
            }
        }
    }
    if targets.is_empty() {
        targets.push("target.example.com".to_string());
    }
    targets
}

/// Write the main `source/report.xml` file.
fn write_report_xml(
    engagement: &engagements::Model,
    targets: &[String],
    output_dir: &Path,
) -> std::result::Result<(), String> {
    let targets_xml: String = targets
        .iter()
        .map(|t| format!("      <target>{}</target>", xml_escape(t)))
        .collect::<Vec<_>>()
        .join("\n");

    let title = xml_escape(&engagement.title);
    let today = chrono::Utc::now().format("%Y-%m-%d");
    let report_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pentest_report xmlns:xi="http://www.w3.org/2001/XInclude"
                findingCode=""
                findingNumberingBase="Report"
                xml:lang="en">
  <meta>
    <title>{title}</title>
    <subtitle>Penetration Test Report</subtitle>
    <targets>
{targets_xml}
    </targets>
    <classification>Confidential</classification>
    <version_history>
      <version number="1.0" date="{today}">
        <v_author>GetHacked</v_author>
        <v_description>Generated report</v_description>
      </version>
    </version_history>
  </meta>
  <section id="executiveSummary">
    <title>Executive Summary</title>
    <p>This report presents the findings from the penetration test of {title}.</p>
  </section>
  <section id="findings">
    <title>Findings</title>
    <generate_findings/>
  </section>
  <section id="nonfindings">
    <title>Non-Findings</title>
  </section>
  <appendix id="methodology">
    <title>Methodology</title>
    <p>Standard penetration testing methodology was followed.</p>
  </appendix>
</pentest_report>
"#
    );

    std::fs::write(output_dir.join("source").join("report.xml"), report_xml)
        .map_err(|e| format!("Failed to write report.xml: {e}"))
}

/// Write individual finding XML files.
fn write_findings_xml(
    findings_list: &[findings::Model],
    output_dir: &Path,
) -> std::result::Result<(), String> {
    for (idx, finding) in findings_list.iter().enumerate() {
        let number = idx + 1;
        let finding_id = format!("finding-{}", finding.pid);
        let threat_level = map_threat_level(&finding.severity);
        let status = &finding.status;
        let description = finding.description.as_str();
        let tech_desc = finding
            .technical_description
            .as_deref()
            .unwrap_or("<p>N/A</p>");
        let impact = finding.impact.as_deref().unwrap_or("<p>N/A</p>");
        let recommendation = finding.recommendation.as_deref().unwrap_or("<p>N/A</p>");
        let category = xml_escape(&finding.category);
        let title = xml_escape(&finding.title);
        let id = xml_escape(&finding_id);

        let finding_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<finding id="{id}" threatLevel="{threat_level}" type="{category}" number="{number}" status="{status}">
  <title>{title}</title>
  <description>{description}</description>
  <technicaldescription>{tech_desc}</technicaldescription>
  <impact>{impact}</impact>
  <recommendation>{recommendation}</recommendation>
</finding>
"#
        );

        let filename = format!("{finding_id}.xml");
        std::fs::write(output_dir.join("findings").join(&filename), finding_xml)
            .map_err(|e| format!("Failed to write finding {filename}: {e}"))?;
    }
    Ok(())
}

/// Write individual non-finding XML files.
fn write_non_findings_xml(
    non_findings_list: &[non_findings::Model],
    output_dir: &Path,
) -> std::result::Result<(), String> {
    for (idx, nf) in non_findings_list.iter().enumerate() {
        let number = idx + 1;
        let nf_id = format!("non-finding-{}", nf.pid);
        let content = if nf.content.is_empty() {
            "<p>This area was tested and found secure.</p>"
        } else {
            &nf.content
        };
        let title = xml_escape(&nf.title);
        let id = xml_escape(&nf_id);

        let nf_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<non-finding id="{id}" number="{number}">
  <title>{title}</title>
  {content}
</non-finding>
"#
        );

        let filename = format!("{nf_id}.xml");
        std::fs::write(output_dir.join("non-findings").join(&filename), nf_xml)
            .map_err(|e| format!("Failed to write non-finding {filename}: {e}"))?;
    }
    Ok(())
}

/// Generate the `PenText` XML directory structure for a report.
pub fn generate_pentext_xml(
    engagement: &engagements::Model,
    findings_list: &[findings::Model],
    non_findings_list: &[non_findings::Model],
    output_dir: &Path,
) -> std::result::Result<(), String> {
    for dir in &["source", "findings", "non-findings", "graphics", "target"] {
        std::fs::create_dir_all(output_dir.join(dir))
            .map_err(|e| format!("Failed to create {dir}: {e}"))?;
    }

    let targets = collect_targets(engagement);
    write_report_xml(engagement, &targets, output_dir)?;
    write_findings_xml(findings_list, output_dir)?;
    write_non_findings_xml(non_findings_list, output_dir)?;

    Ok(())
}

/// Build a PDF report using the pentext-docbuilder container.
pub async fn build_pdf(project_dir: &Path) -> std::result::Result<PathBuf, String> {
    let project_abs = project_dir
        .canonicalize()
        .map_err(|e| format!("Cannot resolve project path: {e}"))?;

    let project_path = project_abs.clone();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("podman")
            .args([
                "run",
                "--rm",
                "--network=none",
                "-e",
                "CI_PROJECT_DIR=/pentext",
                "-e",
                "CI_PROJECT_NAME=report",
                "-v",
                &format!("{}:/pentext:Z", project_path.display()),
                DOCBUILDER_IMAGE,
            ])
            .output()
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
    .map_err(|e| format!("podman run failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PDF build failed: {stderr}"));
    }

    let target_dir = project_abs.join("target");
    if let Ok(entries) = std::fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "pdf") {
                return Ok(p);
            }
        }
    }

    Err("PDF build completed but no PDF file found in target/".to_string())
}

/// Orchestrate full report generation.
pub async fn generate_report(
    db: &DatabaseConnection,
    engagement: &engagements::Model,
    findings_list: &[findings::Model],
    non_findings_list: &[non_findings::Model],
) -> std::result::Result<String, String> {
    let temp_dir = std::env::temp_dir().join(format!("pentext-report-{}", engagement.pid));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to clean temp dir: {e}"))?;
    }

    generate_pentext_xml(engagement, findings_list, non_findings_list, &temp_dir)?;

    let pdf_path = build_pdf(&temp_dir).await?;

    let storage_dir = PathBuf::from("storage").join("reports");
    std::fs::create_dir_all(&storage_dir)
        .map_err(|e| format!("Failed to create storage dir: {e}"))?;

    let pdf_filename = format!("report-{}.pdf", engagement.pid);
    let final_path = storage_dir.join(&pdf_filename);
    std::fs::copy(&pdf_path, &final_path)
        .map_err(|e| format!("Failed to copy PDF: {e}"))?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    #[allow(clippy::default_trait_access)]
    let mut report: reports::ActiveModel = Default::default();
    report.org_id = Set(engagement.org_id);
    report.engagement_id = Set(Some(engagement.id));
    report.title = Set(format!("{} - Pentest Report", engagement.title));
    report.report_type = Set("pentest".to_string());
    report.format = Set("pdf".to_string());
    report.storage_path = Set(Some(final_path.to_string_lossy().to_string()));
    report.generated_at = Set(Some(chrono::Utc::now().into()));
    report
        .insert(db)
        .await
        .map_err(|e| format!("Failed to save report record: {e}"))?;

    Ok(final_path.to_string_lossy().to_string())
}
