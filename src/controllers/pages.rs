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

/// `GET /robots.txt`
#[debug_handler]
pub async fn robots_txt() -> Result<Response> {
    let content = include_str!("../../assets/static/robots.txt");
    Ok(Response::builder()
        .header("content-type", "text/plain")
        .body(axum::body::Body::from(content))
        .expect("robots.txt response")
        .into_response())
}

/// `GET /sitemap.xml`
#[debug_handler]
pub async fn sitemap_xml() -> Result<Response> {
    let content = include_str!("../../assets/static/sitemap.xml");
    Ok(Response::builder()
        .header("content-type", "application/xml")
        .body(axum::body::Body::from(content))
        .expect("sitemap.xml response")
        .into_response())
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
        .add("/robots.txt", get(robots_txt))
        .add("/sitemap.xml", get(sitemap_xml))
}
