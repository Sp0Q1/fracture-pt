//! Override for org settings endpoints to add tier information
//! and alert-email management.

use axum::response::Redirect;
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};

use super::middleware;
use crate::models::_entities::organizations;
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::services::tier::PlanTier;
use crate::{require_role, require_user};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrgSettingsParams {
    pub name: String,
    pub alert_emails: Option<String>,
    pub plan_tier: Option<String>,
}

/// `GET /orgs/:pid/settings` -- org settings page with tier info.
#[debug_handler]
pub async fn settings(
    Path(pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org = org_model::Model::find_by_pid(&ctx.db, &pid)
        .await?
        .ok_or_else(|| Error::NotFound)?;
    let membership =
        crate::models::org_members::Model::find_membership_or_admin(&ctx.db, org.id, user.id)
            .await?
            .ok_or_else(|| Error::NotFound)?;
    let org_ctx =
        middleware::OrgContext::from_membership(&ctx.db, org.clone(), membership, user.id).await;
    require_role!(org_ctx, OrgRole::Admin);
    let user_orgs = org_model::Model::find_visible_orgs(&ctx.db, user.id).await?;

    let tier = PlanTier::from_org(&org);
    let alert_emails = org
        .get_setting("alert_emails")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    let mut tpl_ctx = crate::views::base_context(&user, Some(&org_ctx), &user_orgs);
    tpl_ctx["org"] = serde_json::json!({
        "name": org.name,
        "pid": org.pid.to_string(),
        "slug": org.slug,
        "is_personal": org.is_personal,
    });
    tpl_ctx["plan_tier"] = serde_json::json!(tier.label());
    tpl_ctx["max_targets"] = serde_json::json!(tier
        .max_targets()
        .map_or_else(|| "Unlimited".to_string(), |n| n.to_string()));
    tpl_ctx["scheduling_enabled"] = serde_json::json!(tier.scheduling_enabled());
    tpl_ctx["email_alerts_enabled"] = serde_json::json!(tier.email_alerts_enabled());
    tpl_ctx["alert_emails"] = serde_json::json!(alert_emails);

    format::render().view(&v, "org/settings.html", data!(tpl_ctx))
}

/// `POST /orgs/:pid/settings` -- update org settings (with alert emails).
#[debug_handler]
pub async fn update_settings(
    Path(pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<OrgSettingsParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org = org_model::Model::find_by_pid(&ctx.db, &pid)
        .await?
        .ok_or_else(|| Error::NotFound)?;
    let membership =
        crate::models::org_members::Model::find_membership_or_admin(&ctx.db, org.id, user.id)
            .await?
            .ok_or_else(|| Error::NotFound)?;
    let org_ctx =
        middleware::OrgContext::from_membership(&ctx.db, org.clone(), membership, user.id).await;
    require_role!(org_ctx, OrgRole::Admin);

    // Update org name
    let mut active: organizations::ActiveModel = org.clone().into();
    active.name = sea_orm::ActiveValue::Set(params.name);
    active.update(&ctx.db).await?;

    // Platform admins can change the org tier
    let valid_tiers = ["free", "recon", "strike", "offensive", "enterprise"];
    if let Some(ref tier_str) = params.plan_tier {
        if org_ctx.is_platform_admin && valid_tiers.contains(&tier_str.as_str()) {
            org_model::Model::set_setting(
                &ctx.db,
                org.id,
                "plan_tier",
                serde_json::Value::String(tier_str.clone()),
            )
            .await
            .map_err(|e| Error::Message(e.to_string()))?;
        }
    }

    // Re-read org to get updated settings (tier may have just changed)
    let updated_org = org_model::Model::find_by_pid(&ctx.db, &pid)
        .await?
        .ok_or_else(|| Error::NotFound)?;
    let tier = PlanTier::from_org(&updated_org);
    if tier.email_alerts_enabled() {
        let emails = params.alert_emails.unwrap_or_default();
        org_model::Model::set_setting(
            &ctx.db,
            org.id,
            "alert_emails",
            serde_json::Value::String(emails),
        )
        .await
        .map_err(|e| Error::Message(e.to_string()))?;
    }

    Ok(Redirect::to(&format!("/orgs/{pid}/settings")).into_response())
}

/// Org routes that replace `fracture_core::controllers::org::routes()`,
/// substituting the tier-aware settings handlers to avoid duplicate routes.
pub fn routes() -> Routes {
    use fracture_core::controllers::org;

    Routes::new()
        .prefix("/orgs")
        .add("/", get(org::list))
        .add("/", post(org::create))
        .add("/new", get(org::new))
        // Tier-aware settings (replaces core's settings handlers)
        .add("/{pid}/settings", get(settings))
        .add("/{pid}/settings", post(update_settings))
        .add("/{pid}/members", get(org::members))
        .add("/{pid}/members/invite", post(org::invite))
        .add("/{pid}/members/{user_pid}/role", post(org::update_role))
        .add("/{pid}/members/{user_pid}/remove", post(org::remove_member))
        .add("/{pid}/delete", post(org::delete))
        .add("/switch/{pid}", get(org::switch))
}
