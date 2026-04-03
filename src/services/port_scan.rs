use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::process::Command;
use std::time::Duration;

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
    let validated = validate_target(target)?;

    let target_clone = validated.clone();
    let handle = tokio::task::spawn_blocking(move || {
        Command::new("nmap")
            .args([
                "-sT",
                "-sV",
                "-T4",
                "--top-ports",
                "1000",
                "-oX",
                "-",
                &target_clone,
            ])
            .output()
    });

    let result = tokio::time::timeout(Duration::from_secs(300), handle)
        .await
        .map_err(|_| "Port scan timed out after 300 seconds".to_string())?
        .map_err(|e| format!("Task join error: {e}"))?
        .map_err(|e| format!("Failed to execute nmap: {e}"))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("nmap exited with error: {stderr}"));
    }

    let xml = String::from_utf8_lossy(&result.stdout);
    Ok(parse_nmap_xml(&xml, &validated))
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
