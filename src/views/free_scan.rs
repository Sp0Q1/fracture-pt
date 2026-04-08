use loco_rs::prelude::*;
use serde_json::json;

/// Render the progress page for a free scan job run.
pub fn progress(
    v: &impl ViewRenderer,
    domain: &str,
    status: &str,
    asm_result: Option<&serde_json::Value>,
    error_message: Option<&str>,
    port_scans: &[serde_json::Value],
) -> Result<Response> {
    let mut ctx = serde_json::Map::new();
    ctx.insert("domain".into(), json!(domain));
    ctx.insert("scan_status".into(), json!(status));
    ctx.insert("port_scans".into(), json!(port_scans));
    ctx.insert("port_scan_count".into(), json!(port_scans.len()));

    if let Some(asm) = asm_result {
        ctx.insert(
            "subdomain_count".into(),
            json!(asm["subdomain_count"].as_i64().unwrap_or(0)),
        );
        ctx.insert(
            "cert_count".into(),
            json!(asm["cert_count"].as_i64().unwrap_or(0)),
        );
        if let Some(subs) = asm["subdomains"].as_array() {
            let friendly = super::flatten_subdomains(subs);
            ctx.insert("subdomain_count".into(), json!(friendly.len()));
            ctx.insert("subdomains".into(), json!(friendly));
        }
    }

    if let Some(err) = error_message {
        ctx.insert("error_message".into(), json!(err));
        ctx.insert("has_error".into(), json!(true));
    }

    format::render().view(
        v,
        "asm/free-scan-progress.html",
        data!(serde_json::Value::Object(ctx)),
    )
}
