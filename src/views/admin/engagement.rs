use loco_rs::prelude::*;

use crate::models::_entities::{
    engagement_offers, engagements, findings, pentester_assignments, users,
};

/// Render the admin engagement list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    items: &[engagements::Model],
) -> Result<Response> {
    format::render().view(
        v,
        "admin/engagement/list.html",
        data!({"user_name": user.name, "items": items}),
    )
}

/// Render the admin engagement detail.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    item: &engagements::Model,
    offers: &[engagement_offers::Model],
    assignments: &[pentester_assignments::Model],
    engagement_findings: &[findings::Model],
) -> Result<Response> {
    format::render().view(
        v,
        "admin/engagement/show.html",
        data!({
            "user_name": user.name,
            "item": item,
            "offers": offers,
            "assignments": assignments,
            "findings": engagement_findings,
        }),
    )
}
