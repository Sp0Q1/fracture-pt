use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::Semaphore;

// ---------- crt.sh types ----------

#[derive(Debug, Deserialize)]
pub struct CrtEntry {
    #[serde(default)]
    pub issuer_name: String,
    #[serde(default)]
    pub common_name: String,
    #[serde(default)]
    pub name_value: String,
    #[serde(default)]
    pub not_before: String,
    #[serde(default)]
    pub not_after: String,
}

// ---------- result types for template ----------

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub domain: String,
    pub subdomains: Vec<String>,
    pub subdomain_count: usize,
    pub certs: Vec<CertInfo>,
    pub cert_count: usize,
    pub expired_count: usize,
    pub wildcard_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CertInfo {
    pub common_name: String,
    pub issuer: String,
    pub not_before: String,
    pub not_after: String,
    pub is_expired: bool,
    pub is_wildcard: bool,
    pub san_names: Vec<String>,
}

// ---------- crt.sh query ----------

pub async fn query_crtsh(domain: &str) -> std::result::Result<Vec<CrtEntry>, String> {
    // Domain is already validated to contain only [a-zA-Z0-9.-], safe to interpolate
    let url = format!("https://crt.sh/?q=%25.{domain}&output=json");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp = client
        .get(&url)
        .header("User-Agent", "GetHacked-FreeScan/1.0")
        .send()
        .await
        .map_err(|e| format!("crt.sh request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("crt.sh returned status {}", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    if text.trim().is_empty() || text.trim() == "[]" {
        return Ok(vec![]);
    }

    serde_json::from_str::<Vec<CrtEntry>>(&text)
        .map_err(|e| format!("Failed to parse crt.sh response: {e}"))
}

// ---------- process results ----------

/// Check if a name belongs to the queried domain (exact match or subdomain).
pub fn belongs_to_domain(name: &str, dot_domain: &str) -> bool {
    // dot_domain is ".example.com"; name must equal "example.com" or end with ".example.com"
    name.len() + 1 == dot_domain.len() && dot_domain.ends_with(name) || name.ends_with(dot_domain)
}

pub fn process_results(domain: &str, entries: &[CrtEntry]) -> ScanResult {
    let now = chrono::Utc::now().naive_utc();
    let dot_domain = format!(".{domain}");
    let mut subdomains = BTreeSet::new();
    // Deduplicate certs by (common_name, not_before, not_after)
    let mut seen_certs: BTreeMap<(String, String, String), CertInfo> = BTreeMap::new();
    let mut expired_count = 0;
    let mut wildcard_count_set = BTreeSet::new();

    for entry in entries {
        // Extract subdomains from name_value (can contain multiple lines)
        for name in entry.name_value.split('\n') {
            let name = name.trim().to_lowercase();
            if !name.is_empty() && name != "*" && belongs_to_domain(&name, &dot_domain) {
                subdomains.insert(name);
            }
        }

        // Skip certs that don't belong to this domain
        let cn = entry.common_name.trim().to_lowercase();
        let cn_base = cn.trim_start_matches("*.");
        if !belongs_to_domain(cn_base, &dot_domain) {
            continue;
        }
        let key = (
            cn.clone(),
            entry.not_before.clone(),
            entry.not_after.clone(),
        );

        if seen_certs.contains_key(&key) {
            // Add SANs to existing cert
            if let Some(cert) = seen_certs.get_mut(&key) {
                for name in entry.name_value.split('\n') {
                    let name = name.trim().to_lowercase();
                    if !name.is_empty() && !cert.san_names.contains(&name) {
                        cert.san_names.push(name);
                    }
                }
            }
            continue;
        }

        let is_wildcard = cn.starts_with("*.");
        let is_expired =
            chrono::NaiveDateTime::parse_from_str(&entry.not_after, "%Y-%m-%dT%H:%M:%S")
                .map(|dt| dt < now)
                .unwrap_or(false);

        if is_expired {
            expired_count += 1;
        }
        if is_wildcard {
            wildcard_count_set.insert(cn.clone());
        }

        let san_names: Vec<String> = entry
            .name_value
            .split('\n')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty() && belongs_to_domain(s, &dot_domain))
            .collect();

        // Extract short issuer name
        let issuer = extract_issuer_cn(&entry.issuer_name);

        seen_certs.insert(
            key,
            CertInfo {
                common_name: cn,
                issuer,
                not_before: entry.not_before.clone(),
                not_after: entry.not_after.clone(),
                is_expired,
                is_wildcard,
                san_names,
            },
        );
    }

    // Sort certs: unexpired first, then by not_after descending
    let mut certs: Vec<CertInfo> = seen_certs.into_values().collect();
    certs.sort_by(|a, b| {
        a.is_expired
            .cmp(&b.is_expired)
            .then_with(|| b.not_after.cmp(&a.not_after))
    });

    // Limit to most recent 50 certs for display
    certs.truncate(50);

    let subdomains: Vec<String> = subdomains.into_iter().collect();
    let subdomain_count = subdomains.len();
    let cert_count = certs.len();

    ScanResult {
        domain: domain.to_string(),
        subdomains,
        subdomain_count,
        certs,
        cert_count,
        expired_count,
        wildcard_count: wildcard_count_set.len(),
        error: None,
    }
}

// ---------- DNS resolution ----------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubdomainInfo {
    pub name: String,
    pub resolved: bool,
    pub ips: Vec<String>,
}

/// Resolve a list of subdomains concurrently with a concurrency limit of 10.
/// Returns a `SubdomainInfo` for each subdomain indicating whether it resolved
/// and what IP addresses were found.
pub async fn resolve_subdomains(subdomains: &[String]) -> Vec<SubdomainInfo> {
    let semaphore = Arc::new(Semaphore::new(10));
    let mut handles = Vec::with_capacity(subdomains.len());

    for subdomain in subdomains {
        let sem = semaphore.clone();
        let name = subdomain.clone();
        handles.push(tokio::spawn(async move {
            let Ok(_permit) = sem.acquire().await else {
                tracing::warn!(subdomain = %name, "DNS resolver semaphore closed, skipping");
                return SubdomainInfo {
                    name,
                    resolved: false,
                    ips: vec![],
                };
            };
            // Append port 0 because lookup_host requires a socket address
            let lookup_addr = format!("{name}:0");
            let lookup_result = tokio::net::lookup_host(lookup_addr).await;
            match lookup_result {
                Ok(addrs) => {
                    // Deduplicate, converting IPv4-mapped IPv6 to plain IPv4
                    let mut unique: Vec<String> = addrs
                        .map(|a| match a.ip() {
                            std::net::IpAddr::V4(v4) => v4.to_string(),
                            std::net::IpAddr::V6(v6) => v6
                                .to_ipv4_mapped()
                                .map_or_else(|| v6.to_string(), |v4| v4.to_string()),
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    // Sort IPv4 addresses first, then IPv6
                    unique.sort_by(|a, b| {
                        let a_is_v4 = a.parse::<std::net::Ipv4Addr>().is_ok();
                        let b_is_v4 = b.parse::<std::net::Ipv4Addr>().is_ok();
                        b_is_v4.cmp(&a_is_v4).then_with(|| a.cmp(b))
                    });
                    SubdomainInfo {
                        name,
                        resolved: !unique.is_empty(),
                        ips: unique,
                    }
                }
                Err(_) => SubdomainInfo {
                    name,
                    resolved: false,
                    ips: vec![],
                },
            }
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(info) = handle.await {
            results.push(info);
        }
    }
    results
}

fn extract_issuer_cn(issuer_name: &str) -> String {
    // Parse "C=US, O=Let's Encrypt, CN=R3" -> "R3"
    for part in issuer_name.split(',') {
        let part = part.trim();
        if let Some(cn) = part.strip_prefix("CN=") {
            return cn.to_string();
        }
    }
    issuer_name.to_string()
}
