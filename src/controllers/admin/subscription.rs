use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{EntityTrait, QueryOrder};

use crate::controllers::middleware;
use crate::models::_entities::{organizations, pricing_tiers, subscriptions};
use crate::models::organizations as org_model;
use crate::{require_user, views};

/// `GET /admin/subscriptions` -- list all subscriptions cross-org.
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

    let items = subscriptions::Entity::find()
        .order_by_desc(subscriptions::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .unwrap_or_default();

    // Resolve org names and tier names
    let mut items_json: Vec<serde_json::Value> = Vec::new();
    for item in &items {
        let org = organizations::Entity::find_by_id(item.org_id)
            .one(&ctx.db)
            .await
            .ok()
            .flatten();
        let tier = pricing_tiers::Entity::find_by_id(item.tier_id)
            .one(&ctx.db)
            .await
            .ok()
            .flatten();
        items_json.push(serde_json::json!({
            "pid": item.pid,
            "org_name": org.as_ref().map_or("Unknown", |o| o.name.as_str()),
            "tier_name": tier.as_ref().map_or("Unknown", |t| t.name.as_str()),
            "status": item.status,
            "starts_at": item.starts_at,
            "expires_at": item.expires_at,
        }));
    }

    views::admin::subscription::list(&v, &user, &org_ctx, &user_orgs, &items_json)
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![("/subscriptions".to_string(), get(list))]
}
