use axum::response::Redirect;
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use super::middleware;
use crate::models::_entities::engagements::ActiveModel;
use crate::models::_entities::findings::ActiveModel as FindingActiveModel;
use crate::models::_entities::non_findings::ActiveModel as NonFindingActiveModel;
use crate::models::engagement_comments;
use crate::models::engagement_offers;
use crate::models::engagement_targets;
use crate::models::engagements;
use crate::models::findings;
use crate::models::non_findings;
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::models::pentester_assignments;
use crate::models::reports;
use crate::models::scan_targets;
use crate::models::services;
use crate::{require_user, views};

// ---------------------------------------------------------------------------
// Access control
// ---------------------------------------------------------------------------

/// Role-based access flags for a unified engagement view.
pub struct EngagementAccess {
    pub can_view: bool,
    pub can_edit_findings: bool,
    pub can_comment: bool,
}

/// Compute access for a user on a given engagement.
///
/// - Org members (Viewer+): can view, can comment.
/// - Assigned pentesters: can view, can edit findings, can comment.
/// - Platform admins: can view, can edit findings, can comment.
/// - Everyone else: nothing (return 404).
async fn compute_access(
    db: &DatabaseConnection,
    user_id: i32,
    engagement: &engagements::Model,
) -> EngagementAccess {
    let is_org_member =
        fracture_core::models::org_members::Model::find_membership(db, engagement.org_id, user_id)
            .await
            .is_some();
    let is_assigned = pentester_assignments::Model::is_assigned(db, user_id, engagement.id).await;
    let is_admin =
        fracture_core::models::organizations::Model::is_user_platform_admin(db, user_id).await;

    EngagementAccess {
        can_view: is_org_member || is_assigned || is_admin,
        can_edit_findings: is_assigned || is_admin,
        can_comment: is_org_member || is_assigned || is_admin,
    }
}

/// Load an engagement by PID with access check.  Returns 404 for both
/// nonexistent engagements and ones the user cannot view (no existence leak).
async fn load_engagement_with_access(
    db: &DatabaseConnection,
    pid: &str,
    user_id: i32,
) -> Result<(engagements::Model, EngagementAccess)> {
    let item = engagements::Model::find_by_pid(db, pid)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let access = compute_access(db, user_id, &item).await;
    if !access.can_view {
        return Err(Error::NotFound);
    }
    Ok((item, access))
}

/// Require edit-findings access, returning 404 otherwise.
async fn load_engagement_for_edit(
    db: &DatabaseConnection,
    pid: &str,
    user_id: i32,
) -> Result<engagements::Model> {
    let (item, access) = load_engagement_with_access(db, pid, user_id).await?;
    if !access.can_edit_findings {
        return Err(Error::NotFound);
    }
    Ok(item)
}

// ---------------------------------------------------------------------------
// Request params
// ---------------------------------------------------------------------------

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FindingParams {
    pub title: String,
    pub description: String,
    pub technical_description: Option<String>,
    pub impact: Option<String>,
    pub recommendation: Option<String>,
    pub severity: String,
    pub cve_id: Option<String>,
    pub category: String,
    pub evidence: Option<String>,
    pub affected_asset: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NonFindingParams {
    pub title: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommentParams {
    pub content: String,
}

// ---------------------------------------------------------------------------
// Engagement list / create / show
// ---------------------------------------------------------------------------

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
    crate::require_role!(org_ctx, OrgRole::Viewer);
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
    crate::require_role!(org_ctx, OrgRole::Member);
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
    crate::require_role!(org_ctx, OrgRole::Member);

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

/// `GET /engagements/:pid` -- unified engagement detail (role-based).
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
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;

    let (item, access) = load_engagement_with_access(&ctx.db, &pid, user.id).await?;

    let offers = engagement_offers::Model::find_by_engagement(&ctx.db, item.id).await;
    let engagement_findings = findings::Model::find_by_engagement(&ctx.db, item.id).await;
    let engagement_non_findings = non_findings::Model::find_by_engagement(&ctx.db, item.id).await;
    let linked_targets =
        engagement_targets::Model::find_targets_for_engagement(&ctx.db, item.id).await;

    // Only load org targets if the user is an org member (needs org_ctx)
    let all_org_targets = if let Some(ref oc) = org_ctx {
        scan_targets::Model::find_by_org(&ctx.db, oc.org.id).await
    } else {
        vec![]
    };

    // Load comments with author names
    let raw_comments = engagement_comments::Model::find_by_engagement(&ctx.db, item.id).await;
    let user_ids: Vec<i32> = raw_comments.iter().map(|c| c.user_id).collect();
    let user_map: std::collections::HashMap<i32, String> = if user_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        fracture_core::models::_entities::users::Entity::find()
            .filter(
                fracture_core::models::_entities::users::Column::Id.is_in(user_ids.iter().copied()),
            )
            .all(&ctx.db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|u| (u.id, u.name))
            .collect()
    };

    let comments: Vec<views::engagement::CommentData> = raw_comments
        .iter()
        .map(|c| views::engagement::CommentData {
            pid: c.pid.to_string(),
            user_id: c.user_id,
            author_name: user_map
                .get(&c.user_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            content_html: crate::services::markdown::render(&c.content),
            created_at: c.created_at.to_string(),
        })
        .collect();

    // Load reports for the engagement
    let engagement_reports = reports::Model::find_by_engagement(&ctx.db, item.id).await;

    views::engagement::show(
        &v,
        &user,
        &org_ctx,
        &user_orgs,
        &views::engagement::EngagementShowData {
            item: &item,
            offers: &offers,
            findings: &engagement_findings,
            non_findings: &engagement_non_findings,
            linked_targets: &linked_targets,
            all_org_targets: &all_org_targets,
            comments: &comments,
            reports: &engagement_reports,
            can_edit_findings: access.can_edit_findings,
            can_comment: access.can_comment,
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
    crate::require_role!(org_ctx, OrgRole::Member);
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

// ---------------------------------------------------------------------------
// Target linking
// ---------------------------------------------------------------------------

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
    crate::require_role!(org_ctx, OrgRole::Member);
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
    crate::require_role!(org_ctx, OrgRole::Member);
    let item = engagements::Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    let target = scan_targets::Model::find_by_pid_and_org(&ctx.db, &target_pid, org_ctx.org.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    engagement_targets::Model::unlink(&ctx.db, item.id, target.id)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

// ---------------------------------------------------------------------------
// Findings CRUD
// ---------------------------------------------------------------------------

/// `GET /engagements/:pid/findings/new` -- new finding form.
#[debug_handler]
pub async fn new_finding(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot add findings in current state".into(),
        ));
    }
    views::engagement::finding_form(&v, &user, &org_ctx, &user_orgs, &item, None)
}

/// `POST /engagements/:pid/findings` -- create a finding.
#[debug_handler]
pub async fn add_finding(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<FindingParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot add findings in current state".into(),
        ));
    }

    // Validate severity against the approved enum
    let valid_severities = ["extreme", "high", "elevated", "moderate", "low"];
    if !valid_severities.contains(&params.severity.as_str()) {
        return Err(Error::BadRequest("Invalid severity level".into()));
    }

    #[allow(clippy::default_trait_access)]
    let mut finding: FindingActiveModel = Default::default();
    finding.org_id = Set(item.org_id);
    finding.engagement_id = Set(Some(item.id));
    finding.created_by_user_id = Set(Some(user.id));
    finding.title = Set(params.title);
    finding.description = Set(params.description);
    finding.technical_description = Set(params.technical_description);
    finding.impact = Set(params.impact);
    finding.recommendation = Set(params.recommendation);
    finding.severity = Set(params.severity);
    finding.cve_id = Set(params.cve_id);
    finding.category = Set(params.category);
    finding.evidence = Set(params.evidence);
    finding.affected_asset = Set(params.affected_asset);
    let valid_statuses = ["new", "resolved", "unresolved", "not_retested"];
    let status = params
        .status
        .as_deref()
        .filter(|s| valid_statuses.contains(s))
        .unwrap_or("new");
    finding.status = Set(status.to_string());
    finding.insert(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

/// `GET /engagements/:pid/findings/:finding_pid/edit` -- edit finding form.
#[debug_handler]
pub async fn edit_finding(
    Path((pid, finding_pid)): Path<(String, String)>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    let finding = findings::Model::find_by_pid_and_engagement(&ctx.db, &finding_pid, item.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    views::engagement::finding_form(&v, &user, &org_ctx, &user_orgs, &item, Some(&finding))
}

/// `POST /engagements/:pid/findings/:finding_pid` -- update a finding.
#[debug_handler]
pub async fn update_finding(
    Path((pid, finding_pid)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<FindingParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot edit findings in current state".into(),
        ));
    }

    // Validate severity against the approved enum
    let valid_severities = ["extreme", "high", "elevated", "moderate", "low"];
    if !valid_severities.contains(&params.severity.as_str()) {
        return Err(Error::BadRequest("Invalid severity level".into()));
    }

    let finding = findings::Model::find_by_pid_and_engagement(&ctx.db, &finding_pid, item.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    let mut finding = finding.into_active_model();
    finding.title = Set(params.title);
    finding.description = Set(params.description);
    finding.technical_description = Set(params.technical_description);
    finding.impact = Set(params.impact);
    finding.recommendation = Set(params.recommendation);
    finding.severity = Set(params.severity);
    finding.cve_id = Set(params.cve_id);
    finding.category = Set(params.category);
    finding.evidence = Set(params.evidence);
    finding.affected_asset = Set(params.affected_asset);

    let valid_statuses = ["new", "resolved", "unresolved", "not_retested"];
    if let Some(ref s) = params.status {
        if valid_statuses.contains(&s.as_str()) {
            finding.status = Set(s.clone());
        }
    }

    finding.update(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

/// `POST /engagements/:pid/findings/:finding_pid/delete` -- delete a finding.
#[debug_handler]
pub async fn delete_finding(
    Path((pid, finding_pid)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot delete findings in current state".into(),
        ));
    }

    let finding = findings::Model::find_by_pid_and_engagement(&ctx.db, &finding_pid, item.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    finding.delete(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

// ---------------------------------------------------------------------------
// Non-findings CRUD
// ---------------------------------------------------------------------------

/// `GET /engagements/:pid/non-findings/new` -- new non-finding form.
#[debug_handler]
pub async fn new_non_finding(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot add non-findings in current state".into(),
        ));
    }
    views::engagement::non_finding_form(&v, &user, &org_ctx, &user_orgs, &item, None)
}

/// `POST /engagements/:pid/non-findings` -- create a non-finding.
#[debug_handler]
pub async fn add_non_finding(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<NonFindingParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot add non-findings in current state".into(),
        ));
    }

    #[allow(clippy::default_trait_access)]
    let mut nf: NonFindingActiveModel = Default::default();
    nf.org_id = Set(item.org_id);
    nf.engagement_id = Set(item.id);
    nf.created_by_user_id = Set(Some(user.id));
    nf.title = Set(params.title);
    nf.content = Set(params.content);
    nf.insert(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

/// `GET /engagements/:pid/non-findings/:nf_pid/edit` -- edit non-finding form.
#[debug_handler]
pub async fn edit_non_finding(
    Path((pid, nf_pid)): Path<(String, String)>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    let nf = non_findings::Model::find_by_pid_and_engagement(&ctx.db, &nf_pid, item.id)
        .await
        .ok_or_else(|| Error::NotFound)?;
    views::engagement::non_finding_form(&v, &user, &org_ctx, &user_orgs, &item, Some(&nf))
}

/// `POST /engagements/:pid/non-findings/:nf_pid` -- update a non-finding.
#[debug_handler]
pub async fn update_non_finding(
    Path((pid, nf_pid)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<NonFindingParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot edit non-findings in current state".into(),
        ));
    }

    let nf = non_findings::Model::find_by_pid_and_engagement(&ctx.db, &nf_pid, item.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    let mut nf = nf.into_active_model();
    nf.title = Set(params.title);
    nf.content = Set(params.content);
    nf.update(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

/// `POST /engagements/:pid/non-findings/:nf_pid/delete` -- delete a non-finding.
#[debug_handler]
pub async fn delete_non_finding(
    Path((pid, nf_pid)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if item.status != "in_progress" {
        return Err(Error::BadRequest(
            "Cannot delete non-findings in current state".into(),
        ));
    }

    let nf = non_findings::Model::find_by_pid_and_engagement(&ctx.db, &nf_pid, item.id)
        .await
        .ok_or_else(|| Error::NotFound)?;

    nf.delete(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

// ---------------------------------------------------------------------------
// Comments
// ---------------------------------------------------------------------------

/// `POST /engagements/:pid/comments` -- add a comment.
#[debug_handler]
pub async fn add_comment(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<CommentParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let (item, access) = load_engagement_with_access(&ctx.db, &pid, user.id).await?;
    if !access.can_comment {
        return Err(Error::NotFound);
    }

    if params.content.trim().is_empty() {
        return Err(Error::BadRequest("Comment cannot be empty".into()));
    }

    #[allow(clippy::default_trait_access)]
    let mut comment: crate::models::_entities::engagement_comments::ActiveModel =
        Default::default();
    comment.engagement_id = Set(item.id);
    comment.user_id = Set(user.id);
    comment.content = Set(params.content);
    comment.insert(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

/// `POST /engagements/:pid/comments/:comment_pid/delete` -- delete a comment.
#[debug_handler]
pub async fn delete_comment(
    Path((pid, comment_pid)): Path<(String, String)>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let (item, access) = load_engagement_with_access(&ctx.db, &pid, user.id).await?;
    if !access.can_comment {
        return Err(Error::NotFound);
    }

    let comment_uuid = uuid::Uuid::parse_str(&comment_pid).map_err(|_| Error::NotFound)?;
    let comment = crate::models::_entities::engagement_comments::Entity::find()
        .filter(crate::models::_entities::engagement_comments::Column::EngagementId.eq(item.id))
        .filter(crate::models::_entities::engagement_comments::Column::Pid.eq(comment_uuid))
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    // Only the author or a platform admin can delete
    let is_admin =
        fracture_core::models::organizations::Model::is_user_platform_admin(&ctx.db, user.id).await;
    if comment.user_id != user.id && !is_admin {
        return Err(Error::NotFound);
    }

    comment.delete(&ctx.db).await?;

    Ok(Redirect::to(&format!("/engagements/{pid}")).into_response())
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// `GET /engagements/:pid/report` -- show report generation page.
#[debug_handler]
pub async fn report_page(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if !matches!(item.status.as_str(), "in_progress" | "review" | "delivered") {
        return Err(Error::BadRequest(
            "Reports can only be generated for active or delivered engagements".into(),
        ));
    }
    let engagement_findings = findings::Model::find_by_engagement(&ctx.db, item.id).await;
    let engagement_non_findings = non_findings::Model::find_by_engagement(&ctx.db, item.id).await;
    let engagement_reports = reports::Model::find_by_engagement(&ctx.db, item.id).await;

    // Check for pending report build job
    let def_name = format!("Report: {}", item.title);
    let report_job_status = if let Some(def) =
        crate::models::_entities::job_definitions::Entity::find()
            .filter(crate::models::_entities::job_definitions::Column::OrgId.eq(item.org_id))
            .filter(crate::models::_entities::job_definitions::Column::Name.eq(&def_name))
            .one(&ctx.db)
            .await?
    {
        let pending = crate::models::_entities::job_runs::Entity::find()
            .filter(crate::models::_entities::job_runs::Column::JobDefinitionId.eq(def.id))
            .filter(
                crate::models::_entities::job_runs::Column::Status
                    .eq("queued")
                    .or(crate::models::_entities::job_runs::Column::Status.eq("running")),
            )
            .one(&ctx.db)
            .await
            .ok()
            .flatten();
        if pending.is_some() {
            Some("running")
        } else {
            let failed = crate::models::_entities::job_runs::Entity::find()
                .filter(crate::models::_entities::job_runs::Column::JobDefinitionId.eq(def.id))
                .filter(crate::models::_entities::job_runs::Column::Status.eq("failed"))
                .order_by_desc(crate::models::_entities::job_runs::Column::CompletedAt)
                .one(&ctx.db)
                .await
                .ok()
                .flatten();
            if failed.is_some() && engagement_reports.is_empty() {
                Some("failed")
            } else {
                None
            }
        }
    } else {
        None
    };

    views::engagement::report_page(
        &v,
        &user,
        &org_ctx,
        &user_orgs,
        &views::engagement::ReportPageData {
            item: &item,
            finding_count: engagement_findings.len(),
            non_finding_count: engagement_non_findings.len(),
            reports_list: &engagement_reports,
            report_job_status,
        },
    )
}

/// `POST /engagements/:pid/report` -- trigger report generation as a background job.
#[debug_handler]
pub async fn generate_report(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let item = load_engagement_for_edit(&ctx.db, &pid, user.id).await?;
    if !matches!(item.status.as_str(), "in_progress" | "review" | "delivered") {
        return Err(Error::BadRequest(
            "Reports can only be generated for active or delivered engagements".into(),
        ));
    }

    let engagement_findings = findings::Model::find_by_engagement(&ctx.db, item.id).await;
    if engagement_findings.is_empty() {
        return Err(Error::BadRequest(
            "Cannot generate report without findings".into(),
        ));
    }

    let def_name = format!("Report: {}", item.title);
    let config = serde_json::json!({
        "engagement_id": item.id,
        "org_id": item.org_id,
    });

    let definition = if let Some(existing) =
        crate::models::_entities::job_definitions::Entity::find()
            .filter(crate::models::_entities::job_definitions::Column::OrgId.eq(item.org_id))
            .filter(crate::models::_entities::job_definitions::Column::Name.eq(&def_name))
            .one(&ctx.db)
            .await?
    {
        existing
    } else {
        crate::models::_entities::job_definitions::ActiveModel {
            org_id: sea_orm::ActiveValue::Set(item.org_id),
            name: sea_orm::ActiveValue::Set(def_name),
            job_type: sea_orm::ActiveValue::Set("report_build".to_string()),
            config: sea_orm::ActiveValue::Set(config.to_string()),
            enabled: sea_orm::ActiveValue::Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?
    };

    let run =
        crate::models::job_runs::Model::create_queued(&ctx.db, definition.id, item.org_id).await?;

    crate::workers::job_dispatcher::JobDispatchWorker::perform_later(
        &ctx,
        crate::workers::job_dispatcher::JobDispatchArgs {
            job_run_id: run.id,
            job_definition_id: definition.id,
        },
    )
    .await?;

    Ok(Redirect::to(&format!("/engagements/{pid}/report")).into_response())
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

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
        // Findings
        .add("/{pid}/findings/new", get(new_finding))
        .add("/{pid}/findings", post(add_finding))
        .add("/{pid}/findings/{finding_pid}/edit", get(edit_finding))
        .add("/{pid}/findings/{finding_pid}", post(update_finding))
        .add("/{pid}/findings/{finding_pid}/delete", post(delete_finding))
        // Non-findings
        .add("/{pid}/non-findings/new", get(new_non_finding))
        .add("/{pid}/non-findings", post(add_non_finding))
        .add("/{pid}/non-findings/{nf_pid}/edit", get(edit_non_finding))
        .add("/{pid}/non-findings/{nf_pid}", post(update_non_finding))
        .add(
            "/{pid}/non-findings/{nf_pid}/delete",
            post(delete_non_finding),
        )
        // Comments
        .add("/{pid}/comments", post(add_comment))
        .add("/{pid}/comments/{comment_pid}/delete", post(delete_comment))
        // Report
        .add("/{pid}/report", get(report_page).post(generate_report))
}
