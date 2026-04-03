use axum::response::Redirect;
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::{EntityTrait, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::controllers::middleware;
use crate::models::_entities::engagement_offers::ActiveModel as OfferActiveModel;
use crate::models::_entities::pentester_assignments::ActiveModel as AssignmentActiveModel;
use crate::models::_entities::users;
use crate::models::engagement_offers;
use crate::models::engagements;
use crate::models::findings;
use crate::models::organizations as org_model;
use crate::models::pentester_assignments;
use crate::{require_user, views};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OfferParams {
    pub amount_cents: i32,
    pub currency: String,
    pub timeline_days: i32,
    pub deliverables: String,
    pub terms: Option<String>,
    pub valid_until: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssignParams {
    pub user_id: i32,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusParams {
    pub status: String,
    pub admin_notes: Option<String>,
}

/// `GET /admin/engagements` -- list all engagement requests (cross-org).
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
    let items = engagements::Model::find_all_pending(&ctx.db).await;
    views::admin::engagement::list(&v, &user, &org_ctx, &user_orgs, &items)
}

/// `GET /admin/engagements/all` -- list all engagements across all statuses.
#[debug_handler]
pub async fn list_all(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let items = engagements::Entity::find()
        .order_by(engagements::Column::Id, Order::Desc)
        .all(&ctx.db)
        .await
        .unwrap_or_default();
    views::admin::engagement::list(&v, &user, &org_ctx, &user_orgs, &items)
}

/// `GET /admin/engagements/:pid` -- show engagement detail (admin view).
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
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let item = engagements::Model::find_by_pid(&ctx.db, &pid)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let offers = engagement_offers::Model::find_by_engagement(&ctx.db, item.id).await;
    let assignments = pentester_assignments::Model::find_by_engagement(&ctx.db, item.id).await;
    let engagement_findings = findings::Model::find_by_engagement(&ctx.db, item.id).await;

    // Resolve user names for assigned pentesters
    let mut assignment_users: Vec<(pentester_assignments::Model, String, String)> = Vec::new();
    for a in &assignments {
        if let Ok(Some(u)) = users::Entity::find_by_id(a.user_id).one(&ctx.db).await {
            assignment_users.push((a.clone(), u.name.clone(), u.email.clone()));
        }
    }

    // Load all users for pentester assignment dropdown, filtering out already-assigned
    let assigned_user_ids: Vec<i32> = assignments.iter().map(|a| a.user_id).collect();
    let all_users = users::Entity::find()
        .order_by_asc(users::Column::Name)
        .all(&ctx.db)
        .await
        .unwrap_or_default();
    let available_users: Vec<users::Model> = all_users
        .into_iter()
        .filter(|u| !assigned_user_ids.contains(&u.id))
        .collect();

    views::admin::engagement::show(
        &v,
        &user,
        &org_ctx,
        &user_orgs,
        &views::admin::engagement::ShowViewData {
            item: &item,
            offers: &offers,
            assignments: &assignments,
            assignment_users: &assignment_users,
            findings: &engagement_findings,
            available_users: &available_users,
        },
    )
}

/// `POST /admin/engagements/:pid/offer` -- create an offer for an engagement.
#[debug_handler]
pub async fn create_offer(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<OfferParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let item = engagements::Model::find_by_pid(&ctx.db, &pid)
        .await
        .ok_or_else(|| Error::NotFound)?;

    // Can create offers when status is requested or negotiating
    if item.status != "requested" && item.status != "negotiating" {
        return Err(Error::BadRequest(
            "Cannot create offer in current state".into(),
        ));
    }

    // Supersede any previous pending offers
    let existing_offers = engagement_offers::Model::find_by_engagement(&ctx.db, item.id).await;
    for existing in existing_offers {
        if existing.status == "pending" {
            let mut offer = existing.into_active_model();
            offer.status = Set("superseded".to_string());
            offer.update(&ctx.db).await?;
        }
    }

    // Parse valid_until date
    let valid_until =
        chrono::DateTime::parse_from_rfc3339(&format!("{}T23:59:59Z", params.valid_until))
            .map_err(|_| Error::BadRequest("Invalid date format for valid_until".into()))?;

    // Create the new offer
    #[allow(clippy::default_trait_access)]
    let mut offer: OfferActiveModel = Default::default();
    offer.engagement_id = Set(item.id);
    offer.created_by_user_id = Set(Some(user.id));
    offer.amount_cents = Set(params.amount_cents);
    offer.currency = Set(params.currency);
    offer.timeline_days = Set(params.timeline_days);
    offer.deliverables = Set(params.deliverables);
    offer.terms = Set(params.terms);
    offer.valid_until = Set(valid_until);
    offer.status = Set("pending".to_string());
    offer.insert(&ctx.db).await?;

    // Update engagement status to offer_sent
    let mut engagement = item.into_active_model();
    engagement.status = Set("offer_sent".to_string());
    engagement.update(&ctx.db).await?;

    Ok(Redirect::to(&format!("/admin/engagements/{pid}")).into_response())
}

/// `POST /admin/engagements/:pid/assign` -- assign a pentester to an engagement.
#[debug_handler]
pub async fn assign_pentester(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<AssignParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let item = engagements::Model::find_by_pid(&ctx.db, &pid)
        .await
        .ok_or_else(|| Error::NotFound)?;

    // Can assign pentesters when status is accepted or in_progress
    if item.status != "accepted" && item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot assign pentester in current state".into(),
        ));
    }

    // Verify the user exists
    let pentester_user = crate::models::_entities::users::Entity::find_by_id(params.user_id)
        .one(&ctx.db)
        .await
        .map_err(|_| Error::NotFound)?
        .ok_or_else(|| Error::BadRequest("User not found".into()))?;

    // Verify the user is not assigning a client org member to their own engagement
    // (prevents accidentally granting a client user pentester-level access)
    let is_client_member = crate::models::_entities::org_members::Entity::find()
        .filter(crate::models::_entities::org_members::Column::OrgId.eq(item.org_id))
        .filter(crate::models::_entities::org_members::Column::UserId.eq(pentester_user.id))
        .one(&ctx.db)
        .await
        .ok()
        .flatten();
    if is_client_member.is_some() {
        return Err(Error::BadRequest(
            "Cannot assign a client org member as pentester to their own engagement".into(),
        ));
    }

    // Check the pentester isn't already assigned
    if pentester_assignments::Model::is_assigned(&ctx.db, params.user_id, item.id).await {
        return Err(Error::BadRequest(
            "Pentester already assigned to this engagement".into(),
        ));
    }

    // Create the assignment
    #[allow(clippy::default_trait_access)]
    let mut assignment: AssignmentActiveModel = Default::default();
    assignment.engagement_id = Set(item.id);
    assignment.user_id = Set(params.user_id);
    assignment.assigned_by_user_id = Set(Some(user.id));
    assignment.role = Set(params.role.unwrap_or_else(|| "member".to_string()));
    assignment.insert(&ctx.db).await?;

    // Move engagement to in_progress if still accepted
    if item.status == "accepted" {
        let mut engagement = item.into_active_model();
        engagement.status = Set("in_progress".to_string());
        engagement.starts_at = Set(Some(chrono::Utc::now().into()));
        engagement.update(&ctx.db).await?;
    }

    Ok(Redirect::to(&format!("/admin/engagements/{pid}")).into_response())
}

/// `POST /admin/engagements/:pid/status` -- update engagement status (workflow transitions).
#[debug_handler]
pub async fn update_status(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<StatusParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    fracture_core::require_platform_admin!(org_ctx);
    let item = engagements::Model::find_by_pid(&ctx.db, &pid)
        .await
        .ok_or_else(|| Error::NotFound)?;

    // Validate status transitions (no wildcard -- delivered/review cannot be cancelled)
    let valid_transition = matches!(
        (item.status.as_str(), params.status.as_str()),
        ("in_progress", "review")
            | ("review", "delivered" | "in_progress")
            | (
                "requested" | "offer_sent" | "negotiating" | "accepted" | "in_progress",
                "cancelled"
            )
    );

    if !valid_transition {
        return Err(Error::BadRequest(format!(
            "Invalid transition from '{}' to '{}'",
            item.status, params.status
        )));
    }

    let mut engagement = item.into_active_model();
    engagement.status = Set(params.status.clone());
    if let Some(notes) = params.admin_notes {
        engagement.admin_notes = Set(Some(notes));
    }
    if params.status == "delivered" || params.status == "cancelled" {
        engagement.completed_at = Set(Some(chrono::Utc::now().into()));
    }
    engagement.update(&ctx.db).await?;

    Ok(Redirect::to(&format!("/admin/engagements/{pid}")).into_response())
}

pub fn route_list() -> Vec<(String, axum::routing::MethodRouter<AppContext>)> {
    vec![
        ("/engagements".to_string(), get(list)),
        ("/engagements/all".to_string(), get(list_all)),
        ("/engagements/{pid}".to_string(), get(show)),
        ("/engagements/{pid}/offer".to_string(), post(create_offer)),
        (
            "/engagements/{pid}/assign".to_string(),
            post(assign_pentester),
        ),
        ("/engagements/{pid}/status".to_string(), post(update_status)),
    ]
}
