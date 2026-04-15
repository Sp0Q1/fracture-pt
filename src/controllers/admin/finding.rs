use axum::response::Redirect;
use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use std::collections::HashMap;

use crate::controllers::middleware;
use crate::models::_entities::{findings as findings_entity, organizations};
use crate::models::findings;
use crate::models::organizations as org_model;
use crate::{require_user, views};

/// `GET /admin/findings` -- list all findings cross-org.
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
    let items = findings::Model::find_all(&ctx.db).await;

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

    views::admin::finding::list(&v, &user, &org_ctx, &user_orgs, &items_json)
}

/// `GET /admin/findings/:pid` -- show finding detail (admin view).
#[debug_handler]
pub async fn show(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await?;
    let item = findings::Model::find_by_pid(&ctx.db, &pid)
        .await
        .ok_or_else(|| Error::NotFound)?;

    // Resolve org name
    let org_name = organizations::Entity::find_by_id(item.org_id)
        .one(&ctx.db)
        .await
        .ok()
        .flatten()
        .map_or_else(|| "Unknown".to_string(), |o| o.name);

    // Resolve engagement title if linked
    let engagement_title = if let Some(eid) = item.engagement_id {
        crate::models::_entities::engagements::Entity::find_by_id(eid)
            .one(&ctx.db)
            .await
            .ok()
            .flatten()
            .map(|e| e.title)
    } else {
        None
    };

    views::admin::finding::show(
        &v,
        &user,
        &org_ctx,
        &user_orgs,
        &item,
        &org_name,
        engagement_title.as_deref(),
    )
}

#[derive(Debug, Deserialize)]
pub struct BulkDeleteParams {
    #[serde(default, rename = "pids[]")]
    pub pids: Vec<String>,
}

/// `POST /admin/findings/bulk-delete` -- delete multiple findings cross-org (admin only).
#[debug_handler]
pub async fn bulk_delete(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<BulkDeleteParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);

    let uuids: Vec<sea_orm::prelude::Uuid> = params
        .pids
        .iter()
        .filter_map(|pid| sea_orm::prelude::Uuid::parse_str(pid).ok())
        .collect();

    if !uuids.is_empty() {
        findings_entity::Entity::delete_many()
            .filter(findings_entity::Column::Pid.is_in(uuids))
            .exec(&ctx.db)
            .await?;
    }

    Ok(Redirect::to("/admin/findings").into_response())
}

/// `POST /admin/findings/:pid/delete` -- delete single finding (admin only).
#[debug_handler]
pub async fn delete(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);

    let item = findings::Model::find_by_pid(&ctx.db, &pid)
        .await
        .ok_or_else(|| Error::NotFound)?;
    item.delete(&ctx.db).await?;

    Ok(Redirect::to("/admin/findings").into_response())
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![
        ("/findings".to_string(), get(list)),
        ("/findings/bulk-delete".to_string(), post(bulk_delete)),
        ("/findings/{pid}".to_string(), get(show)),
        ("/findings/{pid}/delete".to_string(), post(delete)),
    ]
}
