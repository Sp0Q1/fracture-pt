use axum::extract::State;
use axum_extra::extract::Form;
use loco_rs::{mailer::Email, prelude::*};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub message: String,
    // Scope wizard fields
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub wizard_approach: String,
    #[serde(default)]
    pub wizard_scope: String,
    #[serde(default)]
    pub wizard_duration: String,
    #[serde(default)]
    pub wizard_estimate: String,
}

fn contact_recipient(ctx: &AppContext) -> String {
    ctx.config
        .settings
        .as_ref()
        .and_then(|s| s.get("contact_email"))
        .and_then(|v| v.as_str())
        .unwrap_or("hello@gethacked.eu")
        .to_string()
}

/// `POST /api/contact` — handle contact form and scope wizard submissions.
#[debug_handler]
pub async fn submit(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    Form(form): Form<ContactForm>,
) -> Result<Response> {
    // Basic validation — reject empty required fields
    let name = form.name.trim();
    let email = form.email.trim();
    if name.is_empty() || email.is_empty() {
        return format::render().view(
            &v,
            "contact/success.html",
            data!({ "error": "Name and email are required." }),
        );
    }
    if !email.contains('@') || !email.contains('.') {
        return format::render().view(
            &v,
            "contact/success.html",
            data!({ "error": "Please enter a valid email address." }),
        );
    }

    let to = contact_recipient(&ctx);
    let is_scope = form.subject == "scope-wizard";

    let subject = if is_scope {
        format!(
            "[GetHacked] Scope request from {} ({})",
            form.name, form.email
        )
    } else {
        format!("[GetHacked] Contact: {} — {}", form.subject, form.name)
    };

    let body_text = if is_scope {
        format!(
            "Name: {}\nEmail: {}\nCompany: {}\nPhone: {}\n\n\
             --- Scope Wizard ---\n\
             Approach: {}\nTargets: {}\nDuration: {} man-days\n\
             Estimate: EUR {}\n\n\
             --- Details ---\n{}",
            form.name,
            form.email,
            form.company,
            form.phone,
            form.wizard_approach,
            form.wizard_scope,
            form.wizard_duration,
            form.wizard_estimate,
            form.details,
        )
    } else {
        format!(
            "Name: {}\nEmail: {}\nCompany: {}\nSubject: {}\n\n\
             --- Message ---\n{}",
            form.name, form.email, form.company, form.subject, form.message,
        )
    };

    let body_html = body_text.replace('\n', "<br>");

    let from = ctx
        .config
        .settings
        .as_ref()
        .and_then(|s| s.get("mailer_from"))
        .and_then(|v| v.as_str())
        .unwrap_or("noreply@gethacked.eu")
        .to_string();

    if let Some(ref mailer) = ctx.mailer {
        let email = Email {
            from: Some(from),
            to,
            subject,
            text: body_text,
            html: body_html,
            reply_to: Some(form.email.clone()),
            ..Default::default()
        };
        if let Err(e) = mailer.mail(&email).await {
            tracing::error!(error = %e, "failed to send contact email");
        }
    } else {
        tracing::warn!("mailer not configured — contact form submission logged only");
        tracing::info!(from = %form.email, subject = %subject, "contact form submission");
    }

    format::render().view(
        &v,
        "contact/success.html",
        data!({ "name": form.name, "is_scope": is_scope }),
    )
}

pub fn routes() -> Routes {
    Routes::new().add("/api/contact", post(submit))
}
