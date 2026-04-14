use loco_rs::prelude::*;

use crate::controllers::middleware::OrgContext;
use crate::models::_entities::{
    engagement_offers, engagements, findings, non_findings, organizations, reports, scan_targets,
    services, users,
};

/// Pre-rendered comment for template consumption.
#[derive(Clone, Debug)]
pub struct CommentData {
    pub pid: String,
    pub user_id: i32,
    pub author_name: String,
    pub content_html: String,
    pub created_at: String,
}

/// Render the engagement list.
pub fn list(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    items: &[engagements::Model],
) -> Result<Response> {
    let mut ctx = super::base_context(user, Some(org_ctx), user_orgs);
    ctx["items"] = serde_json::json!(items);
    format::render().view(v, "engagement/list.html", data!(ctx))
}

/// Render the scope submission (new engagement request) form.
pub fn request_form(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &OrgContext,
    user_orgs: &[organizations::Model],
    all_services: &[services::Model],
    org_targets: &[scan_targets::Model],
    preselect_target_id: Option<i32>,
) -> Result<Response> {
    let mut ctx = super::base_context(user, Some(org_ctx), user_orgs);
    ctx["services"] = serde_json::json!(all_services);
    ctx["targets"] = serde_json::json!(org_targets);
    ctx["preselect_target_id"] = serde_json::json!(preselect_target_id.unwrap_or(0));
    format::render().view(v, "engagement/request.html", data!(ctx))
}

pub struct EngagementShowData<'a> {
    pub item: &'a engagements::Model,
    pub offers: &'a [engagement_offers::Model],
    pub findings: &'a [findings::Model],
    pub non_findings: &'a [non_findings::Model],
    pub linked_targets: &'a [scan_targets::Model],
    pub all_org_targets: &'a [scan_targets::Model],
    pub comments: &'a [CommentData],
    pub reports: &'a [reports::Model],
    pub can_edit_findings: bool,
    pub can_comment: bool,
}

/// Render the unified engagement detail page.
pub fn show(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    data: &EngagementShowData<'_>,
) -> Result<Response> {
    let mut ctx = super::base_context(user, org_ctx.as_ref(), user_orgs);
    ctx["item"] = serde_json::json!(data.item);
    ctx["offers"] = serde_json::json!(data.offers);
    ctx["findings"] = serde_json::json!(data.findings);
    ctx["non_findings"] = serde_json::json!(data.non_findings);
    ctx["linked_targets"] = serde_json::json!(data.linked_targets);
    ctx["reports"] = serde_json::json!(data.reports);
    ctx["can_edit_findings"] = serde_json::json!(data.can_edit_findings);
    ctx["can_comment"] = serde_json::json!(data.can_comment);
    ctx["current_user_id"] = serde_json::json!(user.id);

    // Compute unlinked targets (org targets not yet linked to this engagement)
    let linked_ids: std::collections::HashSet<i32> =
        data.linked_targets.iter().map(|t| t.id).collect();
    let unlinked: Vec<&scan_targets::Model> = data
        .all_org_targets
        .iter()
        .filter(|t| !linked_ids.contains(&t.id))
        .collect();
    ctx["unlinked_targets"] = serde_json::json!(unlinked);

    // Serialize comments as JSON for the template
    let comments_json: Vec<serde_json::Value> = data
        .comments
        .iter()
        .map(|c| {
            serde_json::json!({
                "pid": c.pid,
                "user_id": c.user_id,
                "author_name": c.author_name,
                "content_html": c.content_html,
                "created_at": c.created_at,
            })
        })
        .collect();
    ctx["comments"] = serde_json::json!(comments_json);

    format::render().view(v, "engagement/show.html", data!(ctx))
}

/// Render the finding create/edit form.
pub fn finding_form(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    engagement: &engagements::Model,
    finding: Option<&findings::Model>,
) -> Result<Response> {
    let template = if finding.is_some() {
        "engagement/finding/edit.html"
    } else {
        "engagement/finding/create.html"
    };
    let mut ctx = super::base_context(user, org_ctx.as_ref(), user_orgs);
    ctx["engagement"] = serde_json::json!(engagement);
    ctx["finding"] = serde_json::json!(finding);
    format::render().view(v, template, data!(ctx))
}

/// Render the non-finding create/edit form.
pub fn non_finding_form(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    engagement: &engagements::Model,
    non_finding: Option<&non_findings::Model>,
) -> Result<Response> {
    let template = if non_finding.is_some() {
        "engagement/non_finding/edit.html"
    } else {
        "engagement/non_finding/create.html"
    };
    let mut ctx = super::base_context(user, org_ctx.as_ref(), user_orgs);
    ctx["engagement"] = serde_json::json!(engagement);
    ctx["non_finding"] = serde_json::json!(non_finding);
    format::render().view(v, template, data!(ctx))
}

/// Data for the report generation page.
pub struct ReportPageData<'a> {
    pub item: &'a engagements::Model,
    pub finding_count: usize,
    pub non_finding_count: usize,
    pub reports_list: &'a [reports::Model],
    pub report_job_status: Option<&'a str>,
}

/// Render the report generation page.
pub fn report_page(
    v: &impl ViewRenderer,
    user: &users::Model,
    org_ctx: &Option<OrgContext>,
    user_orgs: &[organizations::Model],
    data: &ReportPageData<'_>,
) -> Result<Response> {
    let mut ctx = super::base_context(user, org_ctx.as_ref(), user_orgs);
    ctx["item"] = serde_json::json!(data.item);
    ctx["finding_count"] = serde_json::json!(data.finding_count);
    ctx["non_finding_count"] = serde_json::json!(data.non_finding_count);
    ctx["reports"] = serde_json::json!(data.reports_list);
    ctx["report_job_status"] = serde_json::json!(data.report_job_status);
    format::render().view(v, "engagement/report.html", data!(ctx))
}
