use loco_rs::prelude::*;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::models::_entities::{engagements, findings, non_findings, reports};
use sea_orm::ActiveValue::Set;

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

/// Build the report.xml content.
fn build_report_xml(engagement: &engagements::Model, targets: &[String]) -> String {
    let targets_xml: String = targets
        .iter()
        .map(|t| format!("      <target>{}</target>", xml_escape(t)))
        .collect::<Vec<_>>()
        .join("\n");

    let title = xml_escape(&engagement.title);
    let today = chrono::Utc::now().format("%Y-%m-%d");
    format!(
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
    )
}

/// Build finding XML content.
fn build_finding_xml(finding: &findings::Model, number: usize) -> String {
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

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<finding id="{id}" threatLevel="{threat_level}" type="{category}" number="{number}" status="{status}">
  <title>{title}</title>
  <description>{description}</description>
  <technicaldescription>{tech_desc}</technicaldescription>
  <impact>{impact}</impact>
  <recommendation>{recommendation}</recommendation>
</finding>
"#
    )
}

/// Build non-finding XML content.
fn build_non_finding_xml(nf: &non_findings::Model, number: usize) -> String {
    let nf_id = format!("non-finding-{}", nf.pid);
    let content = if nf.content.is_empty() {
        "<p>This area was tested and found secure.</p>"
    } else {
        &nf.content
    };
    let title = xml_escape(&nf.title);
    let id = xml_escape(&nf_id);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<non-finding id="{id}" number="{number}">
  <title>{title}</title>
  {content}
</non-finding>
"#
    )
}

/// Generate a PenText report as a ZIP archive containing all XML files.
///
/// Returns the path to the generated ZIP file.
pub async fn generate_report(
    db: &DatabaseConnection,
    engagement: &engagements::Model,
    findings_list: &[findings::Model],
    non_findings_list: &[non_findings::Model],
) -> std::result::Result<String, String> {
    let storage_dir = PathBuf::from("/app/data/reports");
    std::fs::create_dir_all(&storage_dir)
        .map_err(|e| format!("Failed to create storage dir: {e}"))?;

    let zip_filename = format!("report-{}.zip", engagement.pid);
    let final_path = storage_dir.join(&zip_filename);

    // Build ZIP in memory then write to disk
    let zip_data = build_report_zip(engagement, findings_list, non_findings_list)
        .map_err(|e| format!("Failed to build ZIP: {e}"))?;

    std::fs::write(&final_path, &zip_data).map_err(|e| format!("Failed to write ZIP: {e}"))?;

    // Create report record in database
    #[allow(clippy::default_trait_access)]
    let mut report: reports::ActiveModel = Default::default();
    report.org_id = Set(engagement.org_id);
    report.engagement_id = Set(Some(engagement.id));
    report.title = Set(format!("{} - Pentest Report", engagement.title));
    report.report_type = Set("pentest".to_string());
    report.format = Set("zip".to_string());
    report.storage_path = Set(Some(final_path.to_string_lossy().to_string()));
    report.generated_at = Set(Some(chrono::Utc::now().into()));
    report
        .insert(db)
        .await
        .map_err(|e| format!("Failed to save report record: {e}"))?;

    Ok(final_path.to_string_lossy().to_string())
}

/// Build the ZIP archive containing all PenText XML files.
fn build_report_zip(
    engagement: &engagements::Model,
    findings_list: &[findings::Model],
    non_findings_list: &[non_findings::Model],
) -> std::result::Result<Vec<u8>, std::io::Error> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // report.xml
    let targets = collect_targets(engagement);
    let report_xml = build_report_xml(engagement, &targets);
    zip.start_file("source/report.xml", options)?;
    zip.write_all(report_xml.as_bytes())?;

    // findings
    for (idx, finding) in findings_list.iter().enumerate() {
        let xml = build_finding_xml(finding, idx + 1);
        let filename = format!("findings/finding-{}.xml", finding.pid);
        zip.start_file(filename, options)?;
        zip.write_all(xml.as_bytes())?;
    }

    // non-findings
    for (idx, nf) in non_findings_list.iter().enumerate() {
        let xml = build_non_finding_xml(nf, idx + 1);
        let filename = format!("non-findings/non-finding-{}.xml", nf.pid);
        zip.start_file(filename, options)?;
        zip.write_all(xml.as_bytes())?;
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

/// Generate PenText XML directory structure on disk (for external PDF building).
pub fn generate_pentext_xml(
    engagement: &engagements::Model,
    findings_list: &[findings::Model],
    non_findings_list: &[non_findings::Model],
    output_dir: &Path,
) -> std::result::Result<(), String> {
    for dir in &["source", "findings", "non-findings"] {
        std::fs::create_dir_all(output_dir.join(dir))
            .map_err(|e| format!("Failed to create {dir}: {e}"))?;
    }

    let targets = collect_targets(engagement);
    let report_xml = build_report_xml(engagement, &targets);
    std::fs::write(output_dir.join("source/report.xml"), report_xml)
        .map_err(|e| format!("Failed to write report.xml: {e}"))?;

    for (idx, finding) in findings_list.iter().enumerate() {
        let xml = build_finding_xml(finding, idx + 1);
        let filename = format!("finding-{}.xml", finding.pid);
        std::fs::write(output_dir.join("findings").join(&filename), xml)
            .map_err(|e| format!("Failed to write finding {filename}: {e}"))?;
    }

    for (idx, nf) in non_findings_list.iter().enumerate() {
        let xml = build_non_finding_xml(nf, idx + 1);
        let filename = format!("non-finding-{}.xml", nf.pid);
        std::fs::write(output_dir.join("non-findings").join(&filename), xml)
            .map_err(|e| format!("Failed to write non-finding {filename}: {e}"))?;
    }

    Ok(())
}
