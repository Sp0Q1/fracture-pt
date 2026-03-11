use loco_rs::prelude::*;

use crate::views;

/// `GET /pricing` -- pricing page.
#[debug_handler]
pub async fn pricing(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::pricing(&v)
}

/// `GET /about` -- about page.
#[debug_handler]
pub async fn about(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::about(&v)
}

/// `GET /contact` -- contact page.
#[debug_handler]
pub async fn contact(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::contact(&v)
}

/// `GET /legal/privacy` -- privacy policy.
#[debug_handler]
pub async fn privacy(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::privacy(&v)
}

/// `GET /legal/terms` -- terms of service.
#[debug_handler]
pub async fn terms(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::terms(&v)
}

/// `GET /legal/imprint` -- impressum.
#[debug_handler]
pub async fn imprint(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::imprint(&v)
}

/// `GET /incident-response` -- incident response services.
#[debug_handler]
pub async fn incident_response(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::incident_response(&v)
}

/// `GET /free-scan` -- free ASM scan funnel page.
#[debug_handler]
pub async fn free_scan(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::free_scan(&v)
}

/// `GET /scope` -- interactive pentest scope wizard.
#[debug_handler]
pub async fn scope_wizard(ViewEngine(v): ViewEngine<TeraView>) -> Result<Response> {
    views::pages::scope_wizard(&v)
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/pricing", get(pricing))
        .add("/about", get(about))
        .add("/contact", get(contact))
        .add("/legal/privacy", get(privacy))
        .add("/legal/terms", get(terms))
        .add("/legal/imprint", get(imprint))
        .add("/incident-response", get(incident_response))
        .add("/free-scan", get(free_scan))
        .add("/scope", get(scope_wizard))
}
