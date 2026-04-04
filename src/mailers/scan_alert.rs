use include_dir::{include_dir, Dir};
use loco_rs::mailer::{Args, Mailer, MailerOpts};
use loco_rs::prelude::*;
use serde_json::json;

use fracture_core::jobs::JobDiff;

static SCAN_ALERT_DIR: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/src/mailers/scan_alert/scan_alert");

pub struct ScanAlertMailer;

impl Mailer for ScanAlertMailer {
    fn opts() -> MailerOpts {
        MailerOpts {
            from: "GetHacked <noreply@gethacked.eu>".to_string(),
            ..Default::default()
        }
    }
}

impl ScanAlertMailer {
    /// Send a scan diff alert email.
    ///
    /// # Errors
    ///
    /// Returns an error if the mailer fails to enqueue the email.
    pub async fn send_alert(
        ctx: &AppContext,
        to_email: &str,
        scan_name: &str,
        domain: &str,
        diffs: &[JobDiff],
    ) -> Result<()> {
        let added_subdomains: Vec<String> = diffs
            .iter()
            .filter(|d| d.diff_type == "subdomain_added")
            .map(|d| d.entity_key.clone())
            .collect();
        let removed_subdomains: Vec<String> = diffs
            .iter()
            .filter(|d| d.diff_type == "subdomain_removed")
            .map(|d| d.entity_key.clone())
            .collect();
        let newly_resolved: Vec<String> = diffs
            .iter()
            .filter(|d| d.diff_type == "subdomain_resolved")
            .map(|d| d.entity_key.clone())
            .collect();
        let newly_unresolved: Vec<String> = diffs
            .iter()
            .filter(|d| d.diff_type == "subdomain_unresolved")
            .map(|d| d.entity_key.clone())
            .collect();
        let new_ports: Vec<String> = diffs
            .iter()
            .filter(|d| d.diff_type == "added")
            .map(|d| d.entity_key.clone())
            .collect();

        Self::mail_template(
            ctx,
            &SCAN_ALERT_DIR,
            Args {
                to: to_email.to_string(),
                locals: json!({
                    "scan_name": scan_name,
                    "domain": domain,
                    "added_subdomains": added_subdomains,
                    "removed_subdomains": removed_subdomains,
                    "newly_resolved": newly_resolved,
                    "newly_unresolved": newly_unresolved,
                    "new_ports": new_ports,
                }),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }
}
