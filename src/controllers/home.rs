use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

use super::middleware;
use crate::models::_entities::{engagements, scan_jobs, scan_targets};
use crate::models::organizations as org_model;
use crate::views;

/// Render the home page (authenticated or guest).
#[debug_handler]
pub async fn index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    match user {
        Some(user) => {
            let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
            let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;

            // Gather dashboard stats if org context is available
            let (scan_count, engagement_count, asm_summary): (u64, u64, Option<serde_json::Value>) =
                if let Some(ref oc) = org_ctx {
                    let org_id = oc.org.id;
                    let scans = scan_targets::Entity::find()
                        .filter(scan_targets::Column::OrgId.eq(org_id))
                        .count(&ctx.db)
                        .await
                        .unwrap_or(0);
                    let engs = engagements::Entity::find()
                        .filter(engagements::Column::OrgId.eq(org_id))
                        .count(&ctx.db)
                        .await
                        .unwrap_or(0);

                    // Latest completed ASM scan job
                    let latest_job = scan_jobs::Entity::find()
                        .filter(scan_jobs::Column::OrgId.eq(org_id))
                        .filter(scan_jobs::Column::Status.eq("completed"))
                        .filter(scan_jobs::Column::ResultSummary.is_not_null())
                        .order_by_desc(scan_jobs::Column::CompletedAt)
                        .one(&ctx.db)
                        .await
                        .ok()
                        .flatten();

                    let asm = latest_job.and_then(|job| {
                        job.result_summary
                            .as_deref()
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                            .map(|v| {
                                serde_json::json!({
                                    "domain": v.get("domain").and_then(serde_json::Value::as_str).unwrap_or(""),
                                    "subdomain_count": v.get("subdomain_count").and_then(serde_json::Value::as_u64).unwrap_or(0),
                                    "cert_count": v.get("cert_count").and_then(serde_json::Value::as_u64).unwrap_or(0),
                                    "expired_count": v.get("expired_count").and_then(serde_json::Value::as_u64).unwrap_or(0),
                                    "wildcard_count": v.get("wildcard_count").and_then(serde_json::Value::as_u64).unwrap_or(0),
                                    "status": "completed",
                                    "finding_count": job.finding_count,
                                })
                            })
                    });

                    (scans, engs, asm)
                } else {
                    (0, 0, None)
                };

            views::home::index(
                &v,
                &user,
                &org_ctx,
                &user_orgs,
                scan_count,
                engagement_count,
                asm_summary.as_ref(),
            )
        }
        None => views::home::index_guest(&v),
    }
}

pub fn routes() -> Routes {
    Routes::new().prefix("/").add("", get(index))
}
