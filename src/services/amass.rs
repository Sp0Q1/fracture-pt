//! Passive subdomain enumeration.
//!
//! ## Implementation note
//!
//! The module name and the public `amass_passive` job-type identifier are
//! historical; the actual binary used is `projectdiscovery/subfinder`.
//! amass v4 removed the `-json` flag (output now goes to a sqlite graph DB
//! only), which is incompatible with the sidecar / stdout-capture model.
//! subfinder fills the same niche — OSINT-only passive subdomain
//! enumeration — with a stable JSONL output format on stdout.
//!
//! ## Why passive only here
//!
//! Active subdomain enumeration (DNS brute force, AXFR attempts) hits real
//! infrastructure and must go through `services::scan_authz`'s active gate.
//! That wiring lands in a follow-up. This module exposes only the passive
//! mode, which uses public CT logs and OSINT only.
//!
//! ## Invocation contract
//!
//! - Domain is validated by [`crate::services::port_scan::validate_target`]
//!   before it reaches us. We re-check the basic shape (host-only, no
//!   slashes / spaces / shell metacharacters) as a defence in depth.
//! - argv: `["-d", <domain>, "-silent", "-oJ", "-timeout", "30"]`. Static; no
//!   caller-supplied flags.
//! - subfinder writes one JSON object per line to stdout, shape:
//!   `{"host": "<sub>", "input": "<domain>", "source": "<osint-source>"}`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::services::tool_runner::{self, RunError, RunSpec};

/// Pinned upstream image. Bumping requires updating both the `Containerfile`
/// FROM line and this constant in lock-step.
pub const IMAGE: &str = "ghcr.io/sp0q1/fracture-pt-subfinder:v2.6.6";

/// Total wall-clock the sidecar is allowed to run (10 minutes). subfinder
/// typically finishes in <60s on free OSINT sources, but a stalled upstream
/// can drag a single source out for minutes.
pub const TIMEOUT: Duration = Duration::from_mins(10);

/// Reserved / non-public suffixes the validator refuses. Mirrors the list
/// in `services::port_scan::validate_target` so the two stages refuse the
/// same set of targets.
const RESERVED_SUFFIXES: &[&str] = &[".local", ".internal", ".onion", ".example", ".invalid"];

/// Structured per-host record extracted from subfinder JSONL. `addresses`
/// is intentionally kept (always empty) to preserve the on-disk shape of
/// older `amass`-era job runs — the topology view in `controllers::home`
/// reads it to mark a subdomain as resolved. With subfinder, resolution
/// status comes from `asm_scan` (DNS lookups happen there).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredHost {
    pub name: String,
    pub addresses: Vec<String>,
    pub sources: Vec<String>,
}

/// Aggregated result persisted as `result_summary` of an `amass_passive` run.
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
    #[error("subfinder exited with status {0:?}")]
    NonZeroExit(Option<i32>),
}

/// Run subfinder against `domain` (passive sources only), return parsed result.
///
/// # Errors
///
/// `InvalidDomain` if `domain` doesn't pass the local validator.
/// `Run`/`NonZeroExit` on container-level failures.
pub async fn run_passive(domain: &str) -> Result<AmassResult, AmassError> {
    let domain = sanitize_domain(domain)?;
    let args = ["-d", &domain, "-silent", "-oJ", "-timeout", "30"];
    let out = tool_runner::run(RunSpec {
        image: IMAGE,
        tool_name: "subfinder",
        args: &args,
        wall_clock: TIMEOUT,
        memory: "512m",
        cpus: "1.0",
        pids: 128,
    })
    .await?;
    if out.exit_code != Some(0) {
        // subfinder writes diagnostics to stderr; surface a short prefix
        // without the user-supplied target (which is already in the
        // job_definition).
        tracing::warn!(
            target: "amass",
            exit = ?out.exit_code,
            stderr_preview = %out.stderr.chars().take(200).collect::<String>(),
            "subfinder exited non-zero"
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
            "passive enum expects a domain, not an IP".into(),
        ));
    }
    Ok(domain)
}

/// Parse subfinder JSONL output into [`AmassResult`].
///
/// subfinder writes one JSON object per line, shape:
/// `{"host": "<name>", "input": "<root>", "source": "<osint-source>"}`.
/// Multiple lines per host are common (one source each) — we deduplicate
/// by hostname and aggregate the source list. Lines that fail to parse
/// or that have an empty `host` are silently skipped.
#[must_use]
pub fn parse_jsonl(domain: &str, stdout: &str) -> AmassResult {
    use std::collections::BTreeMap;

    #[derive(Deserialize)]
    struct SubfinderEvent {
        #[serde(default)]
        host: String,
        #[serde(default)]
        source: String,
    }

    let mut by_name: BTreeMap<String, DiscoveredHost> = BTreeMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<SubfinderEvent>(line) else {
            continue;
        };
        if ev.host.is_empty() {
            continue;
        }
        let entry = by_name
            .entry(ev.host.to_lowercase())
            .or_insert_with(|| DiscoveredHost {
                name: ev.host.to_lowercase(),
                addresses: Vec::new(),
                sources: Vec::new(),
            });
        if !ev.source.is_empty() && !entry.sources.contains(&ev.source) {
            entry.sources.push(ev.source);
        }
    }

    let mut hosts: Vec<DiscoveredHost> = by_name.into_values().collect();
    hosts.sort_by(|a, b| a.name.cmp(&b.name));
    let host_count = hosts.len();

    AmassResult {
        domain: domain.to_string(),
        hosts,
        host_count,
        // subfinder is enumeration-only; address resolution lives in
        // services::asm_scan (DNS A/AAAA lookups).
        unique_ips: Vec::new(),
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
        let stdout = "subfinder v2.6.6 starting\n\
            {\"host\":\"foo.example.com\",\"input\":\"example.com\",\"source\":\"crtsh\"}\n";
        let r = parse_jsonl("example.com", stdout);
        assert_eq!(r.host_count, 1);
        assert_eq!(r.hosts[0].name, "foo.example.com");
        assert_eq!(r.hosts[0].sources, vec!["crtsh"]);
        // subfinder doesn't resolve; addresses stay empty.
        assert!(r.hosts[0].addresses.is_empty());
        assert!(r.unique_ips.is_empty());
    }

    #[test]
    fn parse_jsonl_dedupes_by_name_aggregates_sources() {
        let stdout = "\
            {\"host\":\"FOO.example.com\",\"input\":\"example.com\",\"source\":\"crtsh\"}\n\
            {\"host\":\"foo.example.com\",\"input\":\"example.com\",\"source\":\"dnsdumpster\"}\n\
            {\"host\":\"bar.example.com\",\"input\":\"example.com\",\"source\":\"crtsh\"}\n";
        let r = parse_jsonl("example.com", stdout);
        assert_eq!(r.host_count, 2, "FOO and foo collapse");
        let foo = r
            .hosts
            .iter()
            .find(|h| h.name == "foo.example.com")
            .unwrap();
        assert_eq!(foo.sources, vec!["crtsh", "dnsdumpster"]);
    }

    #[test]
    fn parse_jsonl_ignores_malformed_lines() {
        let stdout = "\
            {bad json\n\
            \n\
            {\"host\":\"\",\"source\":\"crtsh\"}\n\
            {\"host\":\"good.example.com\",\"source\":\"crtsh\"}\n";
        let r = parse_jsonl("example.com", stdout);
        assert_eq!(r.host_count, 1);
        assert_eq!(r.hosts[0].name, "good.example.com");
    }
}
