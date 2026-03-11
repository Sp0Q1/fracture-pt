use loco_rs::prelude::*;

use crate::models::_entities::{pricing_tiers, services};

/// Render the service catalog.
pub fn list(v: &impl ViewRenderer, items: &[services::Model]) -> Result<Response> {
    format::render().view(v, "service/list.html", data!({"items": items}))
}

/// Render a service detail page with pricing tiers.
pub fn show(
    v: &impl ViewRenderer,
    service: &services::Model,
    tiers: &[pricing_tiers::Model],
) -> Result<Response> {
    format::render().view(
        v,
        "service/show.html",
        data!({"service": service, "tiers": tiers}),
    )
}
