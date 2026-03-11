use loco_rs::prelude::*;

use crate::models::pricing_tiers;
use crate::models::services;
use crate::views;

/// `GET /services` -- public service catalog.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let items = services::Model::find_active(&ctx.db).await;
    views::service::list(&v, &items)
}

/// `GET /services/:slug` -- service detail + pricing.
#[debug_handler]
pub async fn show(
    Path(slug): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let service = services::Model::find_by_slug(&ctx.db, &slug)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let tiers = pricing_tiers::Model::find_active_by_service(&ctx.db, service.id).await;
    views::service::show(&v, &service, &tiers)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/services")
        .add("/", get(list))
        .add("/{slug}", get(show))
}
