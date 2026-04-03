use axum::response::Redirect;
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::{CookieJar, Form};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;

use crate::controllers::middleware;
use crate::models::_entities::{job_definitions, job_runs, organizations};
use crate::views;
use crate::workers::job_dispatcher::{JobDispatchArgs, JobDispatchWorker};

#[derive(Debug, Deserialize)]
pub struct ScanForm {
    pub domain: String,
}

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }
    let blocked = [
        "localhost",
        ".local",
        ".test",
        ".example",
        ".invalid",
        ".onion",
        ".internal",
    ];
    let lower = domain.to_lowercase();
    if blocked.iter().any(|b| lower == *b || lower.ends_with(b)) {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

/// `POST /free-scan` — validate domain, create a job, redirect to progress page.
///
/// # Errors
///
/// Returns an error if the domain is invalid or the platform admin org is not configured.
#[debug_handler]
pub async fn submit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    Form(form): Form<ScanForm>,
) -> Result<Response> {
    let domain = form.domain.trim().to_lowercase();

    if !is_valid_domain(&domain) {
        return format::render().view(
            &v,
            "asm/free-scan.html",
            data!({ "error": "Please enter a valid domain name (e.g. example.com)." }),
        );
    }

    let admin_org = organizations::Entity::find()
        .filter(organizations::Column::IsPlatformAdmin.eq(true))
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::BadRequest("Platform admin org not configured".into()))?;

    let def_name = format!("Free scan: {domain}");
    let config = serde_json::json!({
        "hostname": domain,
        "org_id": admin_org.id,
    });

    let definition = job_definitions::ActiveModel {
        org_id: Set(admin_org.id),
        name: Set(def_name),
        job_type: Set("asm_scan".to_string()),
        config: Set(config.to_string()),
        enabled: Set(true),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    let run = job_runs::Model::create_queued(&ctx.db, definition.id, admin_org.id).await?;

    JobDispatchWorker::perform_later(
        &ctx,
        JobDispatchArgs {
            job_run_id: run.id,
            job_definition_id: definition.id,
        },
    )
    .await?;

    // Set access cookie so only the requester can view results
    let run_pid = run.pid.to_string();
    let mut cookie = Cookie::new("free_scan_token", run_pid.clone());
    cookie.set_path(format!("/free-scan/{run_pid}"));
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(true);

    let mut response = Redirect::to(&format!("/free-scan/{run_pid}")).into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        cookie.to_string().parse().expect("cookie is valid ASCII"),
    );
    Ok(response)
}

/// `GET /free-scan/:run_pid` — show scan progress / results.
///
/// # Errors
///
/// Returns an error if the scan is not found or the user is not authorised to view it.
#[debug_handler]
pub async fn results_page(
    Path(run_pid): Path<String>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    // Access control: allow if the user has the cookie OR is a platform admin
    let has_cookie = jar
        .get("free_scan_token")
        .is_some_and(|c| c.value() == run_pid);
    let is_admin = if let Some(user) = middleware::get_current_user(&jar, &ctx).await {
        let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user).await;
        org_ctx.is_some_and(|o| o.is_platform_admin)
    } else {
        false
    };

    if !has_cookie && !is_admin {
        return Err(Error::Unauthorized("Access denied".into()));
    }

    let run = job_runs::Model::find_by_pid(&ctx.db, &run_pid)
        .await
        .ok_or_else(|| Error::NotFound)?;

    let definition = job_definitions::Entity::find_by_id(run.job_definition_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let config: serde_json::Value = serde_json::from_str(&definition.config)
        .map_err(|_| Error::BadRequest("Invalid job config".into()))?;
    let domain = config["hostname"].as_str().unwrap_or("unknown").to_string();

    match run.status.as_str() {
        "completed" => {
            let asm_result = run
                .result_summary
                .as_deref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
            views::free_scan::progress(&v, &domain, "completed", asm_result.as_ref(), None)
        }
        "failed" => {
            views::free_scan::progress(&v, &domain, "failed", None, run.error_message.as_deref())
        }
        _ => views::free_scan::progress(&v, &domain, &run.status, None, None),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/free-scan", post(submit))
        .add("/free-scan/{run_pid}", get(results_page))
}
