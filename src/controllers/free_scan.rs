use axum::extract::State;
use axum_extra::extract::Form;
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::services::asm::{self, ScanResult};
use crate::views;

// ---------- form ----------

#[derive(Debug, Deserialize)]
pub struct ScanForm {
    pub domain: String,
}

// ---------- domain validation ----------

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }
    let blocked = [
        "localhost",
        ".local",
        ".test",
        ".example",
        ".invalid",
        ".onion",
        ".internal",
    ];
    let lower = domain.to_lowercase();
    if blocked.iter().any(|b| lower == *b || lower.ends_with(b)) {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

// ---------- handlers ----------

/// `POST /free-scan` -- run a free certificate transparency scan.
#[debug_handler]
pub async fn submit(
    State(_ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    Form(form): Form<ScanForm>,
) -> Result<Response> {
    let domain = form.domain.trim().to_lowercase();

    if !is_valid_domain(&domain) {
        return format::render().view(
            &v,
            "asm/free-scan.html",
            data!({ "error": "Please enter a valid domain name (e.g. example.com)." }),
        );
    }

    let result = match asm::query_crtsh(&domain).await {
        Ok(entries) => asm::process_results(&domain, &entries),
        Err(e) => ScanResult {
            domain: domain.clone(),
            subdomains: vec![],
            subdomain_count: 0,
            certs: vec![],
            cert_count: 0,
            expired_count: 0,
            wildcard_count: 0,
            error: Some(e),
        },
    };

    views::free_scan::results(&v, &result)
}

pub fn routes() -> Routes {
    Routes::new().add("/free-scan", post(submit))
}
