use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;

use crate::controllers::middleware;
use crate::models::_entities::{organizations, scan_targets};
use crate::models::organizations as org_model;
use crate::{require_user, views};

/// Batch-load org names for a set of org IDs.
async fn load_org_names(db: &sea_orm::DatabaseConnection, org_ids: &[i32]) -> HashMap<i32, String> {
    if org_ids.is_empty() {
        return HashMap::new();
    }
    let orgs = organizations::Entity::find()
        .filter(organizations::Column::Id.is_in(org_ids.iter().copied()))
        .all(db)
        .await
        .unwrap_or_default();
    orgs.into_iter().map(|o| (o.id, o.name)).collect()
}

/// `GET /admin/scan-targets` -- list all scan targets cross-org.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await;

    let items = scan_targets::Entity::find()
        .order_by_desc(scan_targets::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .unwrap_or_default();

    // Batch-resolve org names instead of N+1
    let org_ids: Vec<i32> = items.iter().map(|i| i.org_id).collect();
    let org_names = load_org_names(&ctx.db, &org_ids).await;

    let items_json: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            serde_json::json!({
                "pid": item.pid,
                "org_name": org_names.get(&item.org_id).map_or("Unknown", String::as_str),
                "hostname": item.hostname,
                "ip_address": item.ip_address,
                "target_type": item.target_type,
                "verified_at": item.verified_at,
            })
        })
        .collect();

    views::admin::scan_target::list(&v, &user, &org_ctx, &user_orgs, &items_json)
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![("/scan-targets".to_string(), get(list))]
}
