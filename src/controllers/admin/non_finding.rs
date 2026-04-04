use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{EntityTrait, QueryOrder};

use crate::controllers::middleware;
use crate::models::_entities::{engagements, non_findings, organizations};
use crate::models::organizations as org_model;
use crate::{require_user, views};

/// `GET /admin/non-findings` -- list all non-findings cross-org.
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
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;

    let items = non_findings::Entity::find()
        .order_by_desc(non_findings::Column::CreatedAt)
        .all(&ctx.db)
        .await
        .unwrap_or_default();

    // Resolve org names and engagement titles
    let mut items_json: Vec<serde_json::Value> = Vec::new();
    for item in &items {
        let org = organizations::Entity::find_by_id(item.org_id)
            .one(&ctx.db)
            .await
            .ok()
            .flatten();
        let engagement = engagements::Entity::find_by_id(item.engagement_id)
            .one(&ctx.db)
            .await
            .ok()
            .flatten();
        items_json.push(serde_json::json!({
            "pid": item.pid,
            "org_name": org.as_ref().map_or("Unknown", |o| o.name.as_str()),
            "engagement_title": engagement.as_ref().map_or("Unknown", |e| e.title.as_str()),
            "title": item.title,
        }));
    }

    views::admin::non_finding::list(&v, &user, &org_ctx, &user_orgs, &items_json)
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![("/non-findings".to_string(), get(list))]
}
