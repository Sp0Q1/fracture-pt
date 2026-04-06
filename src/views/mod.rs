pub mod admin;
pub mod engagement;
pub mod finding;
pub mod free_scan;
pub mod home;
pub mod invoice;
pub mod pages;
pub mod pentester;
pub mod report;
pub mod scan_target;
pub mod service;
pub mod subscription;

pub use fracture_core::views::{base_context, org};

/// Flatten subdomain JSON values into Tera-friendly objects.
///
/// Tera cannot traverse nested `serde_json::Value` objects in arrays,
/// so we extract and rebuild each subdomain as a flat JSON object.
pub fn flatten_subdomains(subs: &[serde_json::Value]) -> Vec<serde_json::Value> {
    subs.iter()
        .map(|s| {
            serde_json::json!({
                "name": s.get("name").and_then(serde_json::Value::as_str).unwrap_or_default(),
                "resolved": s.get("resolved").and_then(serde_json::Value::as_bool).unwrap_or(false),
                "ips": s.get("ips").and_then(serde_json::Value::as_array)
                    .map(|arr| arr.iter().filter_map(serde_json::Value::as_str).collect::<Vec<_>>())
                    .unwrap_or_default(),
            })
        })
        .collect()
}
