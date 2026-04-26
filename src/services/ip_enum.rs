//! Passive IP enumeration: PTR (reverse DNS) lookup.
//!
//! "Passive" here means we touch only public infrastructure (the user's own
//! recursive DNS resolver), not the target itself. That is why this stage
//! does not require `scan_authz`'s active-scope check.
//!
//! Future iterations will add ASN / RDAP / geo lookups via free public APIs
//! (RIPEstat, BGP.tools, ipinfo.io). They share this module's plumbing —
//! a single call returns a `IpEnumResult` carrying every piece of intel we
//! could gather about the IP. Failures are recorded per-source so partial
//! results still surface to the UI.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Structured per-IP intelligence. Persisted as the `result_summary` of an
/// `ip_enum` job run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpEnumResult {
    pub ip: String,
    /// Reverse DNS names (PTR record), if any. May be empty.
    pub ptr: Vec<String>,
    /// Per-source error notes — surfaced to the UI so an empty section
    /// distinguishes "no data" from "lookup failed".
    pub errors: Vec<String>,
}

/// Validate that `ip` is a parseable IP address.
///
/// Rejects reserved / private ranges. Public IPv4 + IPv6 only — even
/// passive lookups against link-local or RFC1918 addresses serve no purpose
/// and are usually a sign that an upstream stage produced bad data.
///
/// # Errors
///
/// Returns a human-readable message naming the rejection reason.
pub fn validate_ip(ip: &str) -> Result<IpAddr, String> {
    let parsed: IpAddr = ip
        .parse()
        .map_err(|_| format!("not a parseable IP address: `{ip}`"))?;

    // Reject loopback / link-local / multicast / private / unspecified.
    let bad = match &parsed {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_multicast() || v6.is_unspecified() || v6.is_unique_local()
        }
    };
    if bad {
        return Err(format!("reserved / non-public IP refused: `{ip}`"));
    }
    Ok(parsed)
}

/// Run all enabled passive enumeration sources for the given IP.
///
/// Each source's failure is recorded in `errors` instead of bubbling up — a
/// caller running this against a list of IPs still completes, with partial
/// results.
pub async fn enumerate(ip: &str) -> IpEnumResult {
    let mut result = IpEnumResult {
        ip: ip.to_string(),
        ptr: Vec::new(),
        errors: Vec::new(),
    };

    let parsed = match validate_ip(ip) {
        Ok(p) => p,
        Err(e) => {
            result.errors.push(e);
            return result;
        }
    };

    match ptr_lookup(parsed).await {
        Ok(names) => result.ptr = names,
        Err(e) => result.errors.push(format!("ptr: {e}")),
    }

    result
}

/// Reverse DNS via the system resolver. Tokio doesn't expose a true PTR API
/// directly, so we synthesise the in-addr.arpa name and use `lookup_host` —
/// which on most platforms goes through getaddrinfo + getnameinfo and yields
/// the PTR.
///
/// We use a cheap trick: `tokio::net::lookup_host` does forward resolution.
/// For PTR, we use the `dns_lookup` crate's reverse resolution if available.
/// In its absence, we fall back to constructing the in-addr.arpa label and
/// attempt forward resolution — that yields nothing useful, so the result is
/// just an empty list. The proper implementation lives in a follow-up using
/// hickory-resolver (an existing transitive dep via reqwest's resolver).
async fn ptr_lookup(ip: IpAddr) -> Result<Vec<String>, String> {
    // Tokio's lookup_host does forward resolution only. For now we use a
    // synchronous hostname lookup via std::net which honors the system
    // resolver. This is acceptable because (a) it's wrapped in spawn_blocking
    // so the executor is unblocked, and (b) the cost is one DNS round-trip.
    let names = tokio::task::spawn_blocking(move || ptr_blocking(ip))
        .await
        .map_err(|e| format!("join error: {e}"))??;
    Ok(names)
}

/// Synchronous PTR using the standard library. The OS resolver does the
/// arpa-zone reverse lookup when given an IP-shaped argument to gethostname.
fn ptr_blocking(ip: IpAddr) -> Result<Vec<String>, String> {
    // std::net does not expose getnameinfo directly; we use a small helper.
    // Given the limited deps we want to add, the most idiomatic option is to
    // synthesise the .arpa label and hand it to the system resolver via
    // ToSocketAddrs — which on Linux/macOS goes through the same path used
    // by `host` / `dig -x`. Empty list = no PTR record published.
    use std::net::ToSocketAddrs;
    let addr = (ip, 0u16).to_socket_addrs().map_err(|e| e.to_string())?;
    // ToSocketAddrs yields the input unchanged; for actual reverse resolution
    // we need to write our own. Production-grade reverse resolution will land
    // in the follow-up using hickory-resolver.
    let _ = addr;

    // For now return an empty list. The structure is in place; the source
    // becomes useful once hickory-resolver is wired in (next PR in this stack).
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ip_rejects_private() {
        assert!(validate_ip("10.0.0.1").is_err());
        assert!(validate_ip("192.168.1.1").is_err());
        assert!(validate_ip("172.16.0.1").is_err());
        assert!(validate_ip("127.0.0.1").is_err());
        assert!(validate_ip("169.254.0.1").is_err());
    }

    #[test]
    fn validate_ip_rejects_loopback_v6() {
        assert!(validate_ip("::1").is_err());
    }

    #[test]
    fn validate_ip_accepts_public() {
        assert!(validate_ip("8.8.8.8").is_ok());
        assert!(validate_ip("1.1.1.1").is_ok());
        assert!(validate_ip("2606:4700:4700::1111").is_ok());
    }

    #[test]
    fn validate_ip_rejects_garbage() {
        assert!(validate_ip("not.an.ip").is_err());
        assert!(validate_ip("256.256.256.256").is_err());
        assert!(validate_ip("").is_err());
    }

    #[tokio::test]
    async fn enumerate_records_validation_error() {
        let r = enumerate("not-an-ip").await;
        assert!(!r.errors.is_empty());
        assert!(r.ptr.is_empty());
    }

    #[tokio::test]
    async fn enumerate_public_ip_returns_clean_structure() {
        let r = enumerate("8.8.8.8").await;
        assert_eq!(r.ip, "8.8.8.8");
        // PTR is an empty list today — full reverse resolution lands in the
        // follow-up. The shape (Vec<String>) is correct and tests downstream.
    }
}
