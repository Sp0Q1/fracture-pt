use async_trait::async_trait;
use axum::{
    http::{header, HeaderName},
    middleware,
    response::Response,
    Router as AxumRouter,
};
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

async fn set_security_headers(request: axum::extract::Request, next: middleware::Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
         font-src 'self'; connect-src 'self'; form-action 'self'; base-uri 'self'; \
         frame-ancestors 'none'"
            .parse()
            .unwrap(),
    );
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());
    headers.insert(
        header::REFERRER_POLICY,
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    headers.insert(
        HeaderName::from_static("x-permitted-cross-domain-policies"),
        "none".parse().unwrap(),
    );
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        "max-age=63072000; includeSubDomains".parse().unwrap(),
    );

    // Remove framework information leakage
    headers.remove(HeaderName::from_static("x-powered-by"));

    response
}

pub struct SecurityHeadersInitializer;

#[async_trait]
impl Initializer for SecurityHeadersInitializer {
    fn name(&self) -> String {
        "security-headers".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        Ok(router.layer(middleware::from_fn(set_security_headers)))
    }
}
