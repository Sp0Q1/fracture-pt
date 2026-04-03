use axum::response::Redirect;
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};

use super::middleware;
use crate::models::_entities::engagements::ActiveModel;
use crate::models::engagement_offers;
use crate::models::engagement_targets;
use crate::models::engagements;
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::models::scan_targets;
use crate::models::services;
use crate::{require_role, require_user, views};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestParams {
    pub service_id: i32,
    pub title: String,
    pub target_systems: String,
    pub ip_ranges: Option<String>,
    pub domains: Option<String>,
    pub exclusions: Option<String>,
    pub test_window_start: Option<String>,
    pub test_window_end: Option<String>,
    pub contact_name: String,
    pub contact_email: String,
    pub contact_phone: Option<String>,
    pub rules_of_engagement: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkTargetParams {
    pub scan_target_id: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RespondParams {
    pub action: String, // "accept" or "negotiate"
    pub response_text: Option<String>,
}

/// `GET /engagements` -- list org engagements.
#[debug_handler]
pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Viewer);
    let items = engagements::Model::find_by_org(&ctx.db, org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    views::engagement::list(&v, &user, &org_ctx, &user_orgs, &items)
}

/// `GET /engagements/new` -- scope submission form.
#[debug_handler]
pub async fn new(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let all_services = services::Model::find_active(&ctx.db).await;
    views::engagement::request_form(&v, &user, &org_ctx, &user_orgs, &all_services)
}

/// `POST /engagements` -- submit pentest request.
#[debug_handler]
pub async fn add(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<RequestParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);

    // Validate the service exists
    let _service = services::Model::find_by_id(&ctx.db, params.service_id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    #[allow(clippy::default_trait_access)]
    let mut item: ActiveModel = Default::default();
    item.org_id = Set(org_ctx.org.id);
    item.service_id = Set(params.service_id);
    item.title = Set(params.title);
    item.status = Set("requested".to_string());
    item.target_systems = Set(params.target_systems);
    item.ip_ranges = Set(params.ip_ranges);
    item.domains = Set(params.domains);
    item.exclusions = Set(params.exclusions);
    item.contact_name = Set(params.contact_name);
    item.contact_email = Set(params.contact_email);
    item.contact_phone = Set(params.contact_phone);
    item.rules_of_engagement = Set(params.rules_of_engagement);
    item.requested_at = Set(chrono::Utc::now().into());
    item.insert(&ctx.db).await?;
    Ok(Redirect::to("/engagements").into_response())
}

/// `GET /engagements/:pid` -- engagement detail (scope, offer, timeline).
#[debug_handler]
pub async fn show(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Viewer);
    let item = engagements::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let offers = engagement_offers::Model::find_by_engagement(&ctx.db, item.id).await;
    let findings = crate::models::findings::Model::find_by_engagement(&ctx.db, item.id).await;
    let non_findings =
        crate::models::non_findings::Model::find_by_engagement(&ctx.db, item.id).await;
    let linked_targets =
        engagement_targets::Model::find_targets_for_engagement(&ctx.db, item.id).await;
    let all_org_targets = scan_targets::Model::find_by_org(&ctx.db, org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    views::engagement::show(
        &v,
        &user,
        &org_ctx,
        &user_orgs,
        &views::engagement::EngagementShowData {
            item: &item,
            offers: &offers,
            findings: &findings,
            non_findings: &non_findings,
            linked_targets: &linked_targets,
            all_org_targets: &all_org_targets,
        },
    )
}

/// `POST /engagements/:pid/respond` -- accept or negotiate an offer.
#[debug_handler]
pub async fn respond(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<RespondParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    // Accepting/negotiating offers is a financial commitment -- require Admin or Owner
    require_role!(org_ctx, OrgRole::Admin);
    let item = engagements::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    // Can only respond when status is offer_sent
    if item.status != "offer_sent" {
        return Err(Error::BadRequest("Cannot respond in current state".into()));
    }

    let latest_offer = engagement_offers::Model::find_latest_by_engagement(&ctx.db, item.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    match params.action.as_str() {
        "accept" => {
            // Update the offer status
            let mut offer = latest_offer.into_active_model();
            offer.status = Set("accepted".to_string());
            offer.update(&ctx.db).await?;

            // Update the engagement status and copy price from offer
            let accepted_offer =
                engagement_offers::Model::find_latest_by_engagement(&ctx.db, item.id)
                    .await
                    .ok_or_else(|| Error::NotFound)?;
            let mut engagement = item.into_active_model();
            engagement.status = Set("accepted".to_string());
            engagement.price_cents = Set(Some(accepted_offer.amount_cents));
            engagement.currency = Set(Some(accepted_offer.currency.clone()));
            engagement.update(&ctx.db).await?;
        }
        "negotiate" => {
            // Update the offer with client response
            let mut offer = latest_offer.into_active_model();
            offer.status = Set("rejected".to_string());
            offer.client_response = Set(params.response_text);
            offer.update(&ctx.db).await?;

            // Update engagement status
            let mut engagement = item.into_active_model();
            engagement.status = Set("negotiating".to_string());
            engagement.update(&ctx.db).await?;
        }
        _ => {
            return Err(Error::BadRequest("Invalid action".into()));
        }
    }

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

/// `POST /engagements/:pid/targets` -- link a scan target to an engagement.
#[debug_handler]
pub async fn link_target(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<LinkTargetParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);
    let item = engagements::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    // Verify the target belongs to the same org
    let _target = scan_targets::Model::find_by_org(&ctx.db, org_ctx.org.id)
        .await
        .into_iter()
        .find(|t| t.id == params.scan_target_id)
        .ok_or_else(|| Error::NotFound)?;

    // Link (ignore unique constraint errors -- already linked)
    let _ = engagement_targets::Model::link(&ctx.db, item.id, params.scan_target_id).await;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

/// `POST /engagements/:pid/targets/:target_pid/remove` -- unlink a scan target.
#[debug_handler]
pub async fn unlink_target(
    Path((pid, target_pid)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Member);
    let item = engagements::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let target = scan_targets::Model::find_by_pid_and_org(&ctx.db, &target_pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    engagement_targets::Model::unlink(&ctx.db, item.id, target.id)
        .await
        .map_err(|e| Error::BadRequest(e.to_string().into()))?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/engagements")
        .add("/", get(list))
        .add("/", post(add))
        .add("/new", get(new))
        .add("/{pid}", get(show))
        .add("/{pid}/respond", post(respond))
        .add("/{pid}/targets", post(link_target))
        .add("/{pid}/targets/{target_pid}/remove", post(unlink_target))
}
