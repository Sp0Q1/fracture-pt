//! Scan pipeline orchestrator.
//!
//! Pure function [`next_stages_after`] decides — given a completed job's
//! `job_type` and `result_summary` — what stages should run next. Side-effecting
//! [`enqueue_stages`] inserts the corresponding `job_definitions` and `job_runs`
//! rows; the existing [`crate::workers::job_dispatcher::JobDispatchWorker`]
//! picks them up and runs them on its next tick.
//!
//! ## Today's graph
//!
//! ```text
//! asm_scan ─┬─► port_scan (per resolved IP, deduped)
//!           └─► ip_enum   (per resolved IP, deduped)
//! port_scan ─► (no follow-ups yet — nuclei + sslscan land in later PRs)
//! ip_enum  ─► (no follow-ups)
//! ```
//!
//! ## Why a pure function + a separate enqueue step
//!
//! - `next_stages_after` is trivial to unit-test: hand it a `job_type` and a
//!   summary, assert what it returns. No DB. No mocking.
//! - `enqueue_stages` deals with all the SeaORM mess (finding-or-creating
//!   `job_definitions`, deduping pending runs, marking stuck runs as failed,
//!   inserting new queued runs). It is integration-tested against the test DB.
//!
//! ## Auth gating
//!
//! The current implementation does **not** gate active stages through
//! [`crate::services::scan_authz`]. Wiring that gate in is the next PR;
//! see `docs/SCAN_AUTHZ.md` for the model. Today, behavior matches the
//! pre-refactor inline `enqueue_ip_port_scan` in `jobs::asm_scan`.

use std::collections::HashSet;

use chrono::Utc;
use fracture_core::models::_entities::{job_definitions, job_runs};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde_json::{json, Value};

/// One stage to run next, computed from a completed stage's summary.
///
/// A future PR will add a `mode: ScanMode` field tagging passive vs active,
/// so [`enqueue_stages`] can call into `scan_authz` and skip Active stages
/// against unauthorised targets. Today's pipeline enqueues every stage
/// regardless — that matches the pre-refactor inline behaviour in
/// `jobs::asm_scan` so this PR is behaviour-preserving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextStage {
    pub job_type: String,
    pub name: String,
    pub config: Value,
}

/// Outcome for a single stage submitted to [`enqueue_stages`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Queued {
        job_run_id: i32,
        name: String,
    },
    AlreadyPending {
        name: String,
    },
    /// A previously-running run was found stuck (started > 10 minutes ago,
    /// no completion). It was marked failed and a fresh run queued.
    Requeued {
        job_run_id: i32,
        name: String,
    },
}

/// Pure function: given a just-completed job and its summary, list what
/// should run next.
#[must_use]
pub fn next_stages_after(completed: &job_definitions::Model, summary: &Value) -> Vec<NextStage> {
    match completed.job_type.as_str() {
        "asm_scan" => after_asm_scan(completed.org_id, summary),
        // port_scan, ip_enum, others: no follow-ups yet.
        _ => Vec::new(),
    }
}

/// asm_scan → port_scan + ip_enum, one of each per unique resolved IP.
fn after_asm_scan(org_id: i32, summary: &Value) -> Vec<NextStage> {
    let Some(subdomains) = summary.get("subdomains").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut stages = Vec::new();
    let mut seen = HashSet::<String>::new();

    for sub in subdomains {
        if !sub
            .get("resolved")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let subdomain_name = sub.get("name").and_then(Value::as_str).unwrap_or("");
        let Some(ips) = sub.get("ips").and_then(Value::as_array) else {
            continue;
        };
        for ip_value in ips {
            let Some(ip) = ip_value.as_str() else {
                continue;
            };
            // Dedup: the same IP can be returned for many subdomains; we only
            // need one port_scan + ip_enum per IP.
            if !seen.insert(ip.to_string()) {
                continue;
            }
            stages.push(NextStage {
                job_type: "port_scan".to_string(),
                name: format!("Port Scan: {ip} ({subdomain_name})"),
                config: json!({
                    "hostname": ip,
                    "org_id": org_id,
                    "source": "asm_resolved_ip",
                    "subdomain": subdomain_name,
                }),
            });
            stages.push(NextStage {
                job_type: "ip_enum".to_string(),
                name: format!("IP Enum: {ip}"),
                config: json!({
                    "ip": ip,
                    "org_id": org_id,
                    "source": "asm_resolved_ip",
                }),
            });
        }
    }

    stages
}

/// Stuck-run threshold: any run that started more than this long ago without
/// completing is considered abandoned and re-enqueued.
const STUCK_THRESHOLD_MINUTES: i64 = 10;

/// Insert (or re-use) `job_definitions` and queue `job_runs` for each stage.
///
/// Per stage:
/// - Look up an existing definition by `(org_id, name)`. If absent, insert.
/// - Check for an already-queued or already-running `job_run` for that
///   definition.
///   - If a run is "running" but started > 10 minutes ago, mark it failed
///     (timed out) and create a fresh queued run.
///   - If queued or running normally, skip (return `AlreadyPending`).
/// - Otherwise insert a new `job_run` with status `queued`.
///
/// # Errors
///
/// Returns the first database error encountered; remaining stages are not
/// processed. Caller should surface this as a job-completion warning.
pub async fn enqueue_stages(
    db: &DatabaseConnection,
    stages: &[NextStage],
) -> Result<Vec<EnqueueOutcome>, sea_orm::DbErr> {
    let mut outcomes = Vec::with_capacity(stages.len());
    for stage in stages {
        outcomes.push(enqueue_one(db, stage).await?);
    }
    Ok(outcomes)
}

async fn enqueue_one(
    db: &DatabaseConnection,
    stage: &NextStage,
) -> Result<EnqueueOutcome, sea_orm::DbErr> {
    let org_id = stage
        .config
        .get("org_id")
        .and_then(Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| {
            sea_orm::DbErr::Custom(format!("stage `{}` config missing org_id", stage.name))
        })?;

    let definition = upsert_definition(db, org_id, stage).await?;

    let pending = job_runs::Entity::find()
        .filter(job_runs::Column::JobDefinitionId.eq(definition.id))
        .filter(
            job_runs::Column::Status
                .eq("queued")
                .or(job_runs::Column::Status.eq("running")),
        )
        .one(db)
        .await?;

    if let Some(run) = pending {
        let stuck = run.started_at.as_ref().is_some_and(|started| {
            let age = Utc::now() - started.with_timezone(&Utc);
            age.num_minutes() > STUCK_THRESHOLD_MINUTES
        });
        if stuck {
            let mut active: job_runs::ActiveModel = run.into();
            active.status = Set("failed".to_string());
            active.error_message = Set(Some(format!(
                "Stuck for >{STUCK_THRESHOLD_MINUTES} minutes; re-queued"
            )));
            active.completed_at = Set(Some(Utc::now().into()));
            active.update(db).await?;
            let fresh = job_runs::Model::create_queued(db, definition.id, org_id).await?;
            return Ok(EnqueueOutcome::Requeued {
                job_run_id: fresh.id,
                name: stage.name.clone(),
            });
        }
        return Ok(EnqueueOutcome::AlreadyPending {
            name: stage.name.clone(),
        });
    }

    let run = job_runs::Model::create_queued(db, definition.id, org_id).await?;
    Ok(EnqueueOutcome::Queued {
        job_run_id: run.id,
        name: stage.name.clone(),
    })
}

async fn upsert_definition(
    db: &DatabaseConnection,
    org_id: i32,
    stage: &NextStage,
) -> Result<job_definitions::Model, sea_orm::DbErr> {
    if let Some(existing) = job_definitions::Entity::find()
        .filter(job_definitions::Column::OrgId.eq(org_id))
        .filter(job_definitions::Column::Name.eq(&stage.name))
        .one(db)
        .await?
    {
        return Ok(existing);
    }
    job_definitions::ActiveModel {
        org_id: Set(org_id),
        name: Set(stage.name.clone()),
        job_type: Set(stage.job_type.clone()),
        config: Set(stage.config.to_string()),
        enabled: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(job_type: &str) -> job_definitions::Model {
        job_definitions::Model {
            id: 1,
            pid: uuid::Uuid::new_v4(),
            org_id: 7,
            name: "test".to_string(),
            job_type: job_type.to_string(),
            config: "{}".to_string(),
            enabled: true,
            schedule: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    #[test]
    fn unknown_job_type_yields_no_stages() {
        let stages = next_stages_after(&def("nope"), &json!({}));
        assert!(stages.is_empty());
    }

    #[test]
    fn asm_scan_with_no_subdomains_yields_no_stages() {
        let stages = next_stages_after(&def("asm_scan"), &json!({}));
        assert!(stages.is_empty());
    }

    #[test]
    fn asm_scan_unresolved_subdomains_yield_no_stages() {
        let summary = json!({
            "subdomains": [
                {"name": "x.example.com", "resolved": false, "ips": []},
            ],
        });
        let stages = next_stages_after(&def("asm_scan"), &summary);
        assert!(stages.is_empty());
    }

    #[test]
    fn asm_scan_emits_port_scan_and_ip_enum_per_unique_ip() {
        let summary = json!({
            "subdomains": [
                {"name": "a.example.com", "resolved": true, "ips": ["1.2.3.4"]},
                {"name": "b.example.com", "resolved": true, "ips": ["1.2.3.4"]},
                {"name": "c.example.com", "resolved": true, "ips": ["5.6.7.8"]},
            ],
        });
        let stages = next_stages_after(&def("asm_scan"), &summary);

        // Two unique IPs × two stages each = 4
        assert_eq!(stages.len(), 4, "{stages:#?}");

        let port = stages.iter().filter(|s| s.job_type == "port_scan").count();
        let enum_ = stages.iter().filter(|s| s.job_type == "ip_enum").count();
        assert_eq!(port, 2);
        assert_eq!(enum_, 2);

        // Org id propagated.
        assert!(stages
            .iter()
            .all(|s| s.config["org_id"].as_i64() == Some(7)));
    }

    #[test]
    fn asm_scan_dedupes_repeated_ips() {
        let summary = json!({
            "subdomains": [
                {"name": "a", "resolved": true, "ips": ["1.1.1.1", "1.1.1.1", "1.1.1.1"]},
            ],
        });
        let stages = next_stages_after(&def("asm_scan"), &summary);
        assert_eq!(stages.len(), 2, "one port_scan + one ip_enum");
    }

    #[test]
    fn port_scan_yields_no_follow_ups_yet() {
        // nuclei / sslscan plug in here in later PRs.
        let summary = json!({"ports": [{"port": 80, "state": "open", "service": "http"}]});
        let stages = next_stages_after(&def("port_scan"), &summary);
        assert!(stages.is_empty());
    }
}
