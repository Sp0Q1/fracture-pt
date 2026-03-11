use loco_rs::prelude::*;

/// Render the pricing page.
pub fn pricing(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "pricing/index.html", data!({}))
}

/// Render the about page.
pub fn about(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "about/index.html", data!({}))
}

/// Render the contact page.
pub fn contact(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "contact/index.html", data!({}))
}

/// Render the privacy policy page.
pub fn privacy(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "legal/privacy.html", data!({}))
}

/// Render the terms of service page.
pub fn terms(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "legal/terms.html", data!({}))
}

/// Render the imprint page.
pub fn imprint(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "legal/imprint.html", data!({}))
}

/// Render the incident response page.
pub fn incident_response(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "incident-response/index.html", data!({}))
}

/// Render the free ASM scan funnel page.
pub fn free_scan(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "asm/free-scan.html", data!({}))
}

/// Render the scope wizard page.
pub fn scope_wizard(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "scope/wizard.html", data!({}))
}
