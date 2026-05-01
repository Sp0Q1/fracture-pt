//! amass — passive subdomain enumeration.
//!
//! Runs the `owasp-amass/amass:v4.2.0` sidecar in passive mode and parses the
//! JSONL output into a structured `AmassResult` consumable by the dashboard.
//!
//! ## Why passive only here
//!
//! Active amass (brute force, DNS resolution at scale) hits real
//! infrastructure and must go through `services::scan_authz`'s active gate.
//! That wiring lands in a follow-up. This module exposes only the passive
//! mode, which uses public CT logs / OSINT only.
//!
//! ## Invocation contract
//!
//! - Domain is validated by [`crate::services::port_scan::validate_target`]
//!   before it reaches us. We re-check the basic shape (host-only, no
//!   slashes / spaces / shell metacharacters) as a defence in depth.
//! - argv: `["enum", "-passive", "-d", <domain>, "-json", "/dev/stdout",
//!    "-timeout", "10"]`. Static; no caller-supplied flags.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::services::tool_runner::{self, RunError, RunSpec};

/// Pinned upstream image. Bumping requires updating both the `Containerfile`
/// digest and this constant in lock-step.
pub const IMAGE: &str = "ghcr.io/sp0q1/fracture-pt-amass:v4.2.0";

/// Total wall-clock the sidecar is allowed to run.
pub const TIMEOUT: Duration = Duration::from_secs(600);

/// Reserved / non-public suffixes the validator refuses. Mirrors the list
/// in `services::port_scan::validate_target` so the two stages refuse the
/// same set of targets.
const RESERVED_SUFFIXES: &[&str] = &[".local", ".internal", ".onion", ".example", ".invalid"];

/// Structured per-host record extracted from amass JSONL. The shape mirrors
/// the parts of amass's output we surface in the UI; unknown fields are
/// dropped on parse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredHost {
    pub name: String,
    pub addresses: Vec<String>,
    pub sources: Vec<String>,
}

/// Aggregated result persisted as `result_summary` of an amass job run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmassResult {
    pub domain: String,
    pub hosts: Vec<DiscoveredHost>,
    pub host_count: usize,
    pub unique_ips: Vec<String>,
}

/// Errors callers may pattern-match on.
#[derive(Debug, thiserror::Error)]
pub enum AmassError {
    #[error("invalid domain: {0}")]
    InvalidDomain(String),
    #[error(transparent)]
    Run(#[from] RunError),
    #[error("amass exited with status {0:?}")]
    NonZeroExit(Option<i32>),
}

/// Run amass passive against `domain`, return the parsed result.
///
/// # Errors
///
/// `InvalidDomain` if `domain` doesn't pass the local validator.
/// `Run`/`NonZeroExit` on container-level failures.
pub async fn run_passive(domain: &str) -> Result<AmassResult, AmassError> {
    let domain = sanitize_domain(domain)?;
    let args = [
        "enum",
        "-passive",
        "-d",
        &domain,
        "-json",
        "/dev/stdout",
        "-timeout",
        "10",
    ];
    let out = tool_runner::run(RunSpec {
        image: IMAGE,
        tool_name: "amass",
        args: &args,
        wall_clock: TIMEOUT,
        memory: "512m",
        cpus: "1.0",
        pids: 128,
    })
    .await?;
    if out.exit_code != Some(0) {
        // amass writes diagnostics to stderr; surface a short prefix without
        // the user-supplied target (which is already in the job_definition).
        tracing::warn!(
            target: "amass",
            exit = ?out.exit_code,
            stderr_preview = %out.stderr.chars().take(200).collect::<String>(),
            "amass exited non-zero"
        );
        return Err(AmassError::NonZeroExit(out.exit_code));
    }
    Ok(parse_jsonl(&domain, &out.stdout))
}

/// Domain validator. Mirrors `services::port_scan::validate_target` shape but
/// hostname-only and with the suffix denylist enforced.
fn sanitize_domain(domain: &str) -> Result<String, AmassError> {
    let domain = domain.trim().to_lowercase();
    if domain.is_empty() {
        return Err(AmassError::InvalidDomain("empty".into()));
    }
    if !domain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err(AmassError::InvalidDomain(
            "contains characters outside [a-z0-9.-]".into(),
        ));
    }
    if domain.starts_with('-') || domain.ends_with('-') || domain.starts_with('.') {
        return Err(AmassError::InvalidDomain("malformed boundary".into()));
    }
    if RESERVED_SUFFIXES.iter().any(|s| domain.ends_with(s)) {
        return Err(AmassError::InvalidDomain(
            "reserved / non-public suffix refused".into(),
        ));
    }
    if domain.parse::<std::net::IpAddr>().is_ok() {
        return Err(AmassError::InvalidDomain(
            "amass passive expects a domain, not an IP".into(),
        ));
    }
    Ok(domain)
}

/// Parse amass JSONL output into [`AmassResult`].
///
/// amass writes one JSON object per line. Each object describes a discovered
/// name with its observed addresses and the OSINT sources that found it.
/// Lines that fail to parse are silently skipped (amass occasionally emits
/// non-JSON status lines on stderr, but we redirect to /dev/stdout so we
/// must be tolerant). The result is deduplicated by hostname.
#[must_use]
pub fn parse_jsonl(domain: &str, stdout: &str) -> AmassResult {
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Deserialize)]
    struct AmassEvent {
        #[serde(default)]
        name: String,
        #[serde(default)]
        addresses: Vec<AmassAddr>,
        #[serde(default)]
        sources: Vec<String>,
    }
    #[derive(Deserialize)]
    struct AmassAddr {
        #[serde(default)]
        ip: String,
    }

    let mut by_name: BTreeMap<String, DiscoveredHost> = BTreeMap::new();
    let mut all_ips: BTreeSet<String> = BTreeSet::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<AmassEvent>(line) else {
            continue;
        };
        if ev.name.is_empty() {
            continue;
        }
        let entry = by_name
            .entry(ev.name.to_lowercase())
            .or_insert_with(|| DiscoveredHost {
                name: ev.name.to_lowercase(),
                addresses: Vec::new(),
                sources: Vec::new(),
            });
        for addr in &ev.addresses {
            if !addr.ip.is_empty() && !entry.addresses.contains(&addr.ip) {
                entry.addresses.push(addr.ip.clone());
                all_ips.insert(addr.ip.clone());
            }
        }
        for src in &ev.sources {
            if !src.is_empty() && !entry.sources.contains(src) {
                entry.sources.push(src.clone());
            }
        }
    }

    let mut hosts: Vec<DiscoveredHost> = by_name.into_values().collect();
    hosts.sort_by(|a, b| a.name.cmp(&b.name));
    let host_count = hosts.len();
    let unique_ips: Vec<String> = all_ips.into_iter().collect();

    AmassResult {
        domain: domain.to_string(),
        hosts,
        host_count,
        unique_ips,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_domain_accepts_normal() {
        assert_eq!(sanitize_domain("example.com").unwrap(), "example.com");
        assert_eq!(sanitize_domain("  Example.COM  ").unwrap(), "example.com");
        assert_eq!(
            sanitize_domain("api-v2.example.co.uk").unwrap(),
            "api-v2.example.co.uk"
        );
    }

    #[test]
    fn sanitize_domain_rejects_shell_metacharacters() {
        for bad in [
            "a;ls",
            "$(whoami).example.com",
            "`id`",
            "a b.com",
            "a/b.com",
            "a\\b.com",
            "a|b.com",
        ] {
            assert!(sanitize_domain(bad).is_err(), "expected reject: {bad}");
        }
    }

    #[test]
    fn sanitize_domain_rejects_reserved_suffixes() {
        for bad in [
            "router.local",
            "vault.internal",
            "foo.onion",
            "example.example",
            "x.invalid",
        ] {
            assert!(sanitize_domain(bad).is_err(), "expected reject: {bad}");
        }
    }

    #[test]
    fn sanitize_domain_rejects_ip_addresses() {
        assert!(sanitize_domain("8.8.8.8").is_err());
    }

    #[test]
    fn parse_jsonl_handles_empty_output() {
        let r = parse_jsonl("example.com", "");
        assert_eq!(r.domain, "example.com");
        assert_eq!(r.host_count, 0);
        assert!(r.hosts.is_empty());
    }

    #[test]
    fn parse_jsonl_skips_non_json_noise() {
        let stdout = "OWASP Amass v4.2.0\n\
            Querying sources...\n\
            {\"name\":\"foo.example.com\",\"addresses\":[{\"ip\":\"1.2.3.4\"}],\"sources\":[\"crtsh\"]}\n";
        let r = parse_jsonl("example.com", stdout);
        assert_eq!(r.host_count, 1);
        assert_eq!(r.hosts[0].name, "foo.example.com");
        assert_eq!(r.hosts[0].addresses, vec!["1.2.3.4"]);
        assert_eq!(r.hosts[0].sources, vec!["crtsh"]);
        assert_eq!(r.unique_ips, vec!["1.2.3.4"]);
    }

    #[test]
    fn parse_jsonl_dedupes_by_name_aggregates_addresses_and_sources() {
        let stdout = "\
            {\"name\":\"FOO.example.com\",\"addresses\":[{\"ip\":\"1.2.3.4\"}],\"sources\":[\"crtsh\"]}\n\
            {\"name\":\"foo.example.com\",\"addresses\":[{\"ip\":\"5.6.7.8\"}],\"sources\":[\"dnsdumpster\"]}\n\
            {\"name\":\"bar.example.com\",\"addresses\":[{\"ip\":\"5.6.7.8\"}],\"sources\":[\"crtsh\"]}\n";
        let r = parse_jsonl("example.com", stdout);
        assert_eq!(r.host_count, 2, "FOO and foo collapse");
        let foo = r
            .hosts
            .iter()
            .find(|h| h.name == "foo.example.com")
            .unwrap();
        assert_eq!(foo.addresses, vec!["1.2.3.4", "5.6.7.8"]);
        assert_eq!(foo.sources, vec!["crtsh", "dnsdumpster"]);
        assert_eq!(r.unique_ips, vec!["1.2.3.4", "5.6.7.8"]);
    }

    #[test]
    fn parse_jsonl_ignores_malformed_lines() {
        let stdout = "\
            {bad json\n\
            \n\
            {\"name\":\"\",\"addresses\":[],\"sources\":[]}\n\
            {\"name\":\"good.example.com\",\"addresses\":[],\"sources\":[]}\n";
        let r = parse_jsonl("example.com", stdout);
        assert_eq!(r.host_count, 1);
        assert_eq!(r.hosts[0].name, "good.example.com");
    }
}
