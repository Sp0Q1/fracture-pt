use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;
use tokio::process::Command;

/// Information about a single discovered port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: u16,
    pub protocol: String,
    pub state: String,
    pub service: String,
    pub version: String,
}

/// Result of an nmap port scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortScanResult {
    pub target: String,
    pub ip: String,
    pub ports: Vec<PortInfo>,
    pub os_guess: String,
    pub scan_time: String,
    pub total_open: usize,
    pub raw_output: String,
}

/// Validates a scan target (hostname or IP) to prevent command injection and
/// ensure we only scan legitimate external targets.
fn validate_target(target: &str) -> Result<String, String> {
    let target = target.trim().to_lowercase();
    if target.is_empty() || target.len() > 253 {
        return Err("Invalid target length".to_string());
    }

    // Try parsing as IP first
    if let Ok(addr) = target.parse::<IpAddr>() {
        match addr {
            IpAddr::V4(ipv4) => {
                if ipv4.is_loopback()
                    || ipv4.is_private()
                    || ipv4.is_link_local()
                    || ipv4.is_broadcast()
                    || ipv4.is_unspecified()
                {
                    return Err("IP address is in a reserved or private range".to_string());
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_loopback() || ipv6.is_unspecified() {
                    return Err("IP address is in a reserved range".to_string());
                }
            }
        }
        return Ok(target);
    }

    // Hostname validation (only safe characters to prevent command injection)
    if !target
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("Hostname contains invalid characters".to_string());
    }

    let reserved = ["localhost", "localhost.localdomain", "broadcasthost"];
    if reserved.contains(&target.as_str()) {
        return Err("Reserved hostname".to_string());
    }

    let reserved_suffixes = [
        ".local",
        ".internal",
        ".test",
        ".example",
        ".invalid",
        ".onion",
    ];
    for suffix in &reserved_suffixes {
        if target.ends_with(suffix) {
            return Err("Reserved hostname suffix".to_string());
        }
    }

    Ok(target)
}

/// Runs an nmap TCP connect scan against the target.
pub async fn run_nmap(target: &str) -> Result<PortScanResult, String> {
    use tokio::io::AsyncReadExt;

    let validated = validate_target(target)?;

    let is_ipv6 = validated.contains(':');
    let mut args = vec![
        "--unprivileged",
        "-sT", // TCP connect scan (no raw sockets needed)
        "-sV", // Service/version detection
        "--version-intensity",
        "2",   // Light probing (0-9, default 7 — too slow)
        "-T4", // Aggressive timing
        "--top-ports",
        "1000",
        "--host-timeout",
        "120s", // Per-host timeout (catches filtered ports)
        "-oX",
        "-",
    ];
    if is_ipv6 {
        args.push("-6");
    }
    args.push(&validated);

    let mut child = Command::new("nmap")
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute nmap: {e}"))?;

    // Take stdout/stderr handles before waiting, so `child` stays alive for
    // kill() on timeout.
    let mut child_stdout = child.stdout.take();
    let mut child_stderr = child.stderr.take();

    let status =
        if let Ok(result) = tokio::time::timeout(Duration::from_secs(300), child.wait()).await {
            result.map_err(|e| format!("Failed to wait on nmap: {e}"))?
        } else {
            // Timeout — kill the nmap process to prevent zombies
            let _ = child.kill().await;
            return Err("Port scan timed out after 300 seconds".to_string());
        };

    // Read captured output (process has exited, so these reads complete immediately)
    let stdout = if let Some(ref mut out) = child_stdout {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf).await;
        String::from_utf8_lossy(&buf).to_string()
    } else {
        String::new()
    };
    let stderr = if let Some(ref mut err) = child_stderr {
        let mut buf = Vec::new();
        let _ = err.read_to_end(&mut buf).await;
        String::from_utf8_lossy(&buf).to_string()
    } else {
        String::new()
    };

    if !status.success() {
        return Err(format!("nmap exited with error: {stderr}"));
    }

    let mut parsed = parse_nmap_xml(&stdout, &validated);
    // Store raw output (truncated to avoid bloating the DB)
    let mut raw = stdout;
    if !stderr.is_empty() {
        raw.push_str("\n--- stderr ---\n");
        raw.push_str(&stderr);
    }
    // Cap at 100KB to prevent DB bloat
    raw.truncate(100_000);
    parsed.raw_output = raw;
    Ok(parsed)
}

/// Parses nmap XML output using simple string matching.
fn parse_nmap_xml(xml: &str, target: &str) -> PortScanResult {
    let mut ports = Vec::new();
    let mut ip = String::new();
    let mut os_guess = String::new();
    let mut scan_time = String::new();

    // Extract IP from <address addr="..." addrtype="ipv4"/>
    if let Some(addr_start) = xml.find("<address addr=\"") {
        let rest = &xml[addr_start + 15..];
        if let Some(end) = rest.find('"') {
            ip = rest[..end].to_string();
        }
    }

    // Extract elapsed time from <runstats><finished ... elapsed="X"/>
    if let Some(elapsed_start) = xml.find("elapsed=\"") {
        let rest = &xml[elapsed_start + 9..];
        if let Some(end) = rest.find('"') {
            scan_time = format!("{}s", &rest[..end]);
        }
    }

    // Extract OS guess from <osmatch name="..." .../>
    if let Some(os_start) = xml.find("<osmatch name=\"") {
        let rest = &xml[os_start + 15..];
        if let Some(end) = rest.find('"') {
            os_guess = rest[..end].to_string();
        }
    }

    // Parse port entries: <port protocol="tcp" portid="80">
    let mut search_from = 0;
    while let Some(port_start) = xml[search_from..].find("<port ") {
        let abs_start = search_from + port_start;
        // Find the closing </port> or end of self-closing port block
        let block_end = xml[abs_start..].find("</port>").map_or_else(
            || {
                xml[abs_start..]
                    .find("/>")
                    .map_or(xml.len(), |e| abs_start + e + 2)
            },
            |e| abs_start + e + 7,
        );

        let block = &xml[abs_start..block_end];

        let protocol = extract_attr(block, "protocol").unwrap_or_default();
        let port_num: u16 = extract_attr(block, "portid")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let state = block
            .find("<state ")
            .map_or_else(String::new, |state_start| {
                extract_attr(&block[state_start..], "state").unwrap_or_default()
            });

        let (service, version) = block.find("<service ").map_or_else(
            || (String::new(), String::new()),
            |svc_start| {
                let svc_block = &block[svc_start..];
                let svc_name = extract_attr(svc_block, "name").unwrap_or_default();
                let svc_product = extract_attr(svc_block, "product").unwrap_or_default();
                let svc_version = extract_attr(svc_block, "version").unwrap_or_default();
                let version_str = if svc_version.is_empty() {
                    svc_product
                } else if svc_product.is_empty() {
                    svc_version
                } else {
                    format!("{svc_product} {svc_version}")
                };
                (svc_name, version_str)
            },
        );

        if port_num > 0 {
            ports.push(PortInfo {
                port: port_num,
                protocol,
                state,
                service,
                version,
            });
        }

        search_from = block_end;
    }

    let total_open = ports.iter().filter(|p| p.state == "open").count();

    PortScanResult {
        target: target.to_string(),
        ip,
        ports,
        os_guess,
        scan_time,
        total_open,
        raw_output: String::new(),
    }
}

/// Extracts an XML attribute value by name from a tag string.
fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{attr_name}=\"");
    let start = tag.find(&pattern)?;
    let rest = &tag[start + pattern.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
