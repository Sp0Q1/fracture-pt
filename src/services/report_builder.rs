use loco_rs::prelude::*;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::models::_entities::{engagements, findings, non_findings, reports};
use crate::services::markdown;
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

/// Escape XML special characters in text content (for titles, IDs, attributes).
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

// ---------------------------------------------------------------------------
// HTML report generation (primary download format — print-friendly PDF via browser)
// ---------------------------------------------------------------------------

/// Build a self-contained HTML report suitable for print-to-PDF.
#[allow(clippy::too_many_lines)]
fn build_report_html(
    engagement: &engagements::Model,
    findings_list: &[findings::Model],
    non_findings_list: &[non_findings::Model],
) -> String {
    let title = xml_escape(&engagement.title);
    let today = chrono::Utc::now().format("%Y-%m-%d");
    let targets = collect_targets(engagement);
    let targets_html: String = targets
        .iter()
        .map(|t| format!("<li><code>{}</code></li>", xml_escape(t)))
        .collect::<Vec<_>>()
        .join("\n");

    let mut findings_html = String::new();
    for (idx, f) in findings_list.iter().enumerate() {
        let sev_class = match f.severity.as_str() {
            "extreme" => "extreme",
            "high" => "high",
            "elevated" => "elevated",
            "moderate" => "moderate",
            _ => "low",
        };
        let desc = markdown::render(&f.description);
        let tech = f
            .technical_description
            .as_deref()
            .map_or(String::new(), |t| {
                format!(
                    "<h4>Technical Description</h4>\n<div class=\"md\">{}</div>",
                    markdown::render(t)
                )
            });
        let impact = f.impact.as_deref().map_or(String::new(), |t| {
            format!(
                "<h4>Impact</h4>\n<div class=\"md\">{}</div>",
                markdown::render(t)
            )
        });
        let rec = f.recommendation.as_deref().map_or(String::new(), |t| {
            format!(
                "<h4>Recommendation</h4>\n<div class=\"md\">{}</div>",
                markdown::render(t)
            )
        });
        let evidence = f.evidence.as_deref().map_or(String::new(), |t| {
            format!(
                "<h4>Evidence</h4>\n<div class=\"md\">{}</div>",
                markdown::render(t)
            )
        });
        let cve = f
            .cve_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(String::new(), |c| {
                format!("<span class=\"cve\">{}</span>", xml_escape(c))
            });
        let asset = f
            .affected_asset
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(String::new(), |a| {
                format!(
                    "<p class=\"asset\">Affected: <code>{}</code></p>",
                    xml_escape(a)
                )
            });

        let _ = write!(
            findings_html,
            r#"<div class="finding" id="finding-{num}">
<h3>{num}. {title} <span class="sev {sev_class}">{severity}</span> {cve}</h3>
{asset}
<h4>Description</h4>
<div class="md">{desc}</div>
{tech}
{impact}
{rec}
{evidence}
</div>
"#,
            num = idx + 1,
            title = xml_escape(&f.title),
            severity = map_threat_level(&f.severity),
            sev_class = sev_class,
            cve = cve,
            asset = asset,
            desc = desc,
            tech = tech,
            impact = impact,
            rec = rec,
            evidence = evidence,
        );
    }

    let mut non_findings_html = String::new();
    for (idx, nf) in non_findings_list.iter().enumerate() {
        let content = if nf.content.is_empty() {
            "<p>This area was tested and found secure.</p>".to_string()
        } else {
            markdown::render(&nf.content)
        };
        let _ = write!(
            non_findings_html,
            "<div class=\"nonfinding\"><h3>{num}. {title}</h3>\n{content}</div>\n",
            num = idx + 1,
            title = xml_escape(&nf.title),
            content = content,
        );
    }

    // Severity summary counts
    let extreme = findings_list
        .iter()
        .filter(|f| f.severity == "extreme")
        .count();
    let high = findings_list
        .iter()
        .filter(|f| f.severity == "high")
        .count();
    let elevated = findings_list
        .iter()
        .filter(|f| f.severity == "elevated")
        .count();
    let moderate = findings_list
        .iter()
        .filter(|f| f.severity == "moderate")
        .count();
    let low = findings_list.iter().filter(|f| f.severity == "low").count();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Penetration Test Report</title>
<style>
:root {{ --dark:#0f172a; --text:#e2e8f0; --muted:#94a3b8; --accent:#2563eb; --border:#1e293b;
  --extreme:#dc2626; --high:#f97316; --elevated:#eab308; --moderate:#3b82f6; --low:#22c55e; }}
* {{ margin:0; padding:0; box-sizing:border-box; }}
body {{ font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;
  background:var(--dark); color:var(--text); line-height:1.7; padding:2rem; max-width:900px; margin:0 auto; }}
h1 {{ font-size:1.8rem; margin-bottom:.25rem; }}
h2 {{ font-size:1.4rem; margin:2rem 0 1rem; border-bottom:1px solid var(--border); padding-bottom:.5rem; }}
h3 {{ font-size:1.1rem; margin-bottom:.5rem; }}
h4 {{ font-size:.95rem; margin:1rem 0 .3rem; color:var(--muted); }}
p {{ margin:.5rem 0; }}
code {{ background:rgba(255,255,255,.08); padding:.1rem .3rem; border-radius:3px; font-size:.9em; }}
pre {{ background:rgba(0,0,0,.3); padding:1rem; border-radius:6px; overflow-x:auto; font-size:.85rem; margin:.5rem 0; }}
pre code {{ background:none; padding:0; }}
img {{ max-width:100%; border-radius:6px; border:1px solid var(--border); }}
table {{ width:100%; border-collapse:collapse; margin:.5rem 0; }}
th,td {{ padding:.4rem .6rem; border:1px solid var(--border); text-align:left; font-size:.9rem; }}
.meta {{ color:var(--muted); margin-bottom:2rem; }}
.meta p {{ margin:.15rem 0; }}
.targets {{ margin:.5rem 0; }}
.targets li {{ font-size:.9rem; }}
.sev {{ display:inline-block; padding:.1rem .5rem; border-radius:4px; font-size:.8rem; font-weight:600; color:#fff; }}
.sev.extreme {{ background:var(--extreme); }}
.sev.high {{ background:var(--high); color:#000; }}
.sev.elevated {{ background:var(--elevated); color:#000; }}
.sev.moderate {{ background:var(--moderate); }}
.sev.low {{ background:var(--low); color:#000; }}
.cve {{ font-size:.8rem; color:var(--muted); margin-left:.5rem; }}
.asset {{ font-size:.9rem; color:var(--muted); }}
.finding,.nonfinding {{ background:rgba(255,255,255,.03); border:1px solid var(--border);
  border-radius:8px; padding:1.25rem; margin:1rem 0; }}
.summary-grid {{ display:grid; grid-template-columns:repeat(5,1fr); gap:.5rem; margin:1rem 0; }}
.summary-card {{ text-align:center; padding:.75rem; border-radius:6px; }}
.summary-card .count {{ font-size:1.5rem; font-weight:700; }}
.summary-card .label {{ font-size:.75rem; text-transform:uppercase; letter-spacing:.05em; }}
.summary-card.extreme {{ background:rgba(220,38,38,.15); color:var(--extreme); }}
.summary-card.high {{ background:rgba(249,115,22,.15); color:var(--high); }}
.summary-card.elevated {{ background:rgba(234,179,8,.15); color:var(--elevated); }}
.summary-card.moderate {{ background:rgba(59,130,246,.15); color:var(--moderate); }}
.summary-card.low {{ background:rgba(34,197,94,.15); color:var(--low); }}
.footer {{ margin-top:3rem; padding-top:1rem; border-top:1px solid var(--border);
  font-size:.8rem; color:var(--muted); text-align:center; }}
.md ul,.md ol {{ padding-left:1.5rem; }}
@media print {{
  body {{ background:#fff; color:#000; padding:1cm; }}
  .finding,.nonfinding {{ border-color:#ddd; page-break-inside:avoid; }}
  .sev {{ print-color-adjust:exact; -webkit-print-color-adjust:exact; }}
  .summary-card {{ print-color-adjust:exact; -webkit-print-color-adjust:exact; }}
}}
</style>
</head>
<body>
<h1>{title}</h1>
<p class="meta">Penetration Test Report &mdash; Generated {today} &mdash; Confidential</p>

<div class="meta">
<p><strong>Classification:</strong> Confidential</p>
<p><strong>Targets:</strong></p>
<ul class="targets">{targets_html}</ul>
</div>

<h2>Executive Summary</h2>
<p>This report presents the findings from the penetration test of {title}.
A total of <strong>{total}</strong> finding{s} {were} identified.</p>

<div class="summary-grid">
<div class="summary-card extreme"><div class="count">{extreme}</div><div class="label">Extreme</div></div>
<div class="summary-card high"><div class="count">{high}</div><div class="label">High</div></div>
<div class="summary-card elevated"><div class="count">{elevated}</div><div class="label">Elevated</div></div>
<div class="summary-card moderate"><div class="count">{moderate}</div><div class="label">Moderate</div></div>
<div class="summary-card low"><div class="count">{low}</div><div class="label">Low</div></div>
</div>

<h2>Findings</h2>
{findings_html}

{non_findings_section}

<div class="footer">
Generated by GetHacked.eu &mdash; Open-Source Offensive Security Platform
</div>
</body>
</html>"#,
        title = title,
        today = today,
        targets_html = targets_html,
        total = findings_list.len(),
        s = if findings_list.len() == 1 { "" } else { "s" },
        were = if findings_list.len() == 1 {
            "was"
        } else {
            "were"
        },
        extreme = extreme,
        high = high,
        elevated = elevated,
        moderate = moderate,
        low = low,
        findings_html = findings_html,
        non_findings_section = if non_findings_list.is_empty() {
            String::new()
        } else {
            format!("<h2>Non-Findings</h2>\n{non_findings_html}")
        },
    )
}

// ---------------------------------------------------------------------------
// PenText XML generation (for docbuilder PDF pipeline)
// ---------------------------------------------------------------------------

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

/// Build finding XML content with markdown rendered to HTML.
fn build_finding_xml(finding: &findings::Model, number: usize) -> String {
    let finding_id = format!("finding-{}", finding.pid);
    let threat_level = map_threat_level(&finding.severity);
    let status = xml_escape(&finding.status);
    let category = xml_escape(&finding.category);
    let title = xml_escape(&finding.title);
    let id = xml_escape(&finding_id);

    let description = markdown::render_for_xml(&finding.description);
    let tech_desc =
        markdown::render_for_xml(finding.technical_description.as_deref().unwrap_or("N/A"));
    let impact = markdown::render_for_xml(finding.impact.as_deref().unwrap_or("N/A"));
    let recommendation =
        markdown::render_for_xml(finding.recommendation.as_deref().unwrap_or("N/A"));

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

/// Build non-finding XML content with markdown rendered to HTML.
fn build_non_finding_xml(nf: &non_findings::Model, number: usize) -> String {
    let nf_id = format!("non-finding-{}", nf.pid);
    let content = if nf.content.is_empty() {
        "<p>This area was tested and found secure.</p>".to_string()
    } else {
        markdown::render(&nf.content)
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

// ---------------------------------------------------------------------------
// Report generation orchestration
// ---------------------------------------------------------------------------

/// Generate a pentest report: self-contained HTML (primary) + PenText XML ZIP (secondary).
///
/// Returns the path to the generated HTML report file.
pub async fn generate_report(
    db: &DatabaseConnection,
    engagement: &engagements::Model,
    findings_list: &[findings::Model],
    non_findings_list: &[non_findings::Model],
) -> std::result::Result<String, String> {
    let storage_dir = PathBuf::from("/app/data/reports");
    std::fs::create_dir_all(&storage_dir)
        .map_err(|e| format!("Failed to create storage dir: {e}"))?;

    // 1. Generate self-contained HTML report (primary download)
    let html_filename = format!("report-{}.html", engagement.pid);
    let html_path = storage_dir.join(&html_filename);
    let html_data = build_report_html(engagement, findings_list, non_findings_list);
    std::fs::write(&html_path, html_data.as_bytes())
        .map_err(|e| format!("Failed to write HTML report: {e}"))?;

    // 2. Generate PenText XML ZIP (secondary artifact for docbuilder pipeline)
    let zip_filename = format!("report-{}.zip", engagement.pid);
    let zip_path = storage_dir.join(&zip_filename);
    let zip_data = build_report_zip(engagement, findings_list, non_findings_list)
        .map_err(|e| format!("Failed to build ZIP: {e}"))?;
    std::fs::write(&zip_path, &zip_data).map_err(|e| format!("Failed to write ZIP: {e}"))?;

    // Create report record in database (HTML as primary format)
    #[allow(clippy::default_trait_access)]
    let mut report: reports::ActiveModel = Default::default();
    report.org_id = Set(engagement.org_id);
    report.engagement_id = Set(Some(engagement.id));
    report.title = Set(format!("{} - Pentest Report", engagement.title));
    report.report_type = Set("pentest".to_string());
    report.format = Set("html".to_string());
    report.storage_path = Set(Some(html_path.to_string_lossy().to_string()));
    report.generated_at = Set(Some(chrono::Utc::now().into()));
    report
        .insert(db)
        .await
        .map_err(|e| format!("Failed to save report record: {e}"))?;

    Ok(html_path.to_string_lossy().to_string())
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

    let targets = collect_targets(engagement);
    let report_xml = build_report_xml(engagement, &targets);
    zip.start_file("source/report.xml", options)?;
    zip.write_all(report_xml.as_bytes())?;

    for (idx, finding) in findings_list.iter().enumerate() {
        let xml = build_finding_xml(finding, idx + 1);
        let filename = format!("findings/finding-{}.xml", finding.pid);
        zip.start_file(filename, options)?;
        zip.write_all(xml.as_bytes())?;
    }

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
