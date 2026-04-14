use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;

use crate::controllers::middleware;
use crate::models::_entities::organizations;
use crate::models::organizations as org_model;
use crate::models::reports;
use crate::{require_user, views};

/// `GET /admin/reports` -- list all reports cross-org.
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
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await?;
    let items = reports::Model::find_all(&ctx.db).await;

    // Batch-resolve org names
    let org_ids: Vec<i32> = items.iter().map(|i| i.org_id).collect();
    let org_map: HashMap<i32, String> = if org_ids.is_empty() {
        HashMap::new()
    } else {
        organizations::Entity::find()
            .filter(organizations::Column::Id.is_in(org_ids.iter().copied()))
            .all(&ctx.db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|o| (o.id, o.name))
            .collect()
    };

    let items_json: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let mut j = serde_json::json!(item);
            j["org_name"] =
                serde_json::json!(org_map.get(&item.org_id).map_or("Unknown", String::as_str));
            j
        })
        .collect();

    views::admin::report::list(&v, &user, &org_ctx, &user_orgs, &items_json)
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![("/reports".to_string(), get(list))]
}
