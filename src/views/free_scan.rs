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
