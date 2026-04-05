use loco_rs::prelude::*;
use serde_json::json;

use crate::services::asm::ScanResult;

pub fn results(v: &impl ViewRenderer, result: &ScanResult) -> Result<Response> {
    format::render().view(
        v,
        "asm/results.html",
        data!({
            "result": json!(result),
            "domain": &result.domain,
            "subdomain_count": result.subdomain_count,
            "cert_count": result.cert_count,
            "expired_count": result.expired_count,
            "wildcard_count": result.wildcard_count,
            "subdomains": &result.subdomains,
            "certs": &result.certs,
            "has_error": result.error.is_some(),
            "error_message": result.error.as_deref().unwrap_or(""),
        }),
    )
}

/// Render the progress page for a free scan job run.
/// Shows spinner (queued/running), results (completed), or error (failed).
pub fn progress(
    v: &impl ViewRenderer,
    domain: &str,
    status: &str,
    asm_result: Option<&serde_json::Value>,
    error_message: Option<&str>,
) -> Result<Response> {
    let mut ctx = serde_json::Map::new();
    ctx.insert("domain".into(), json!(domain));
    ctx.insert("scan_status".into(), json!(status));

    if let Some(asm) = asm_result {
        ctx.insert(
            "subdomain_count".into(),
            json!(asm["subdomain_count"].as_i64().unwrap_or(0)),
        );
        ctx.insert(
            "cert_count".into(),
            json!(asm["cert_count"].as_i64().unwrap_or(0)),
        );
        ctx.insert(
            "expired_count".into(),
            json!(asm["expired_count"].as_i64().unwrap_or(0)),
        );
        ctx.insert(
            "wildcard_count".into(),
            json!(asm["wildcard_count"].as_i64().unwrap_or(0)),
        );
        if let Some(subs) = asm["subdomains"].as_array() {
            let friendly = super::flatten_subdomains(subs);
            ctx.insert("subdomain_count".into(), json!(friendly.len()));
            ctx.insert("subdomains".into(), json!(friendly));
        }
        if let Some(certs) = asm["certs"].as_array() {
            ctx.insert("certs".into(), json!(certs));
        }
        ctx.insert("has_error".into(), json!(false));
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
