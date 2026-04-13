//! Axum extractors for declarative authorization.
//!
//! Instead of manually calling `middleware::get_current_user` + `require_user!` +
//! `middleware::get_org_context_or_default` + `require_role!` in every handler,
//! use these extractors as handler parameters:
//!
//! ```ignore
//! // Before:
//! pub async fn list(..., jar: CookieJar) -> Result<Response> {
//!     let user = middleware::get_current_user(&jar, &ctx).await;
//!     let user = require_user!(user);
//!     let org_ctx = middleware::get_org_context_or_default(...)...;
//!     require_role!(org_ctx, OrgRole::Viewer);
//!     // ...
//! }
//!
//! // After:
//! pub async fn list(..., auth: OrgAuth<ViewerRole>) -> Result<Response> {
//!     let user = &auth.user;
//!     let org_ctx = &auth.org_ctx;
//!     // ...
//! }
//! ```

use axum::extract::FromRequestParts;
use axum_extra::extract::CookieJar;
use loco_rs::prelude::*;
use std::marker::PhantomData;

use super::middleware::{self, OrgContext};
use crate::models::_entities::users;
use crate::models::org_members::OrgRole;

// ---------------------------------------------------------------------------
// Role marker traits
// ---------------------------------------------------------------------------

/// Marker trait for role-level requirements.
pub trait RoleRequirement: Send + Sync + 'static {
    fn minimum_role() -> OrgRole;
}

/// Viewer access (lowest level).
pub struct ViewerRole;
impl RoleRequirement for ViewerRole {
    fn minimum_role() -> OrgRole {
        OrgRole::Viewer
    }
}

/// Member access.
pub struct MemberRole;
impl RoleRequirement for MemberRole {
    fn minimum_role() -> OrgRole {
        OrgRole::Member
    }
}

/// Admin access.
pub struct AdminRole;
impl RoleRequirement for AdminRole {
    fn minimum_role() -> OrgRole {
        OrgRole::Admin
    }
}

/// Owner access.
pub struct OwnerRole;
impl RoleRequirement for OwnerRole {
    fn minimum_role() -> OrgRole {
        OrgRole::Owner
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn login_redirect() -> Response {
    axum::response::Redirect::temporary("/api/auth/oidc/authorize").into_response()
}

fn not_found() -> Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::NOT_FOUND)
        .body(axum::body::Body::from("Not Found"))
        .unwrap()
        .into_response()
}

fn forbidden() -> Response {
    axum::response::Response::builder()
        .status(axum::http::StatusCode::FORBIDDEN)
        .body(axum::body::Body::from("Forbidden"))
        .unwrap()
        .into_response()
}

// ---------------------------------------------------------------------------
// AuthUser extractor — authenticated user, no org context required
// ---------------------------------------------------------------------------

/// Extracts an authenticated user. Returns a redirect to login if not authenticated.
pub struct AuthUser {
    pub user: users::Model,
}

impl FromRequestParts<AppContext> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(user) = middleware::get_current_user(&jar, state).await else {
            return Err(login_redirect());
        };
        Ok(Self { user })
    }
}

// ---------------------------------------------------------------------------
// OrgAuth<R> extractor — authenticated user + org context + minimum role
// ---------------------------------------------------------------------------

/// Extracts an authenticated user with org context and role authorization.
///
/// Usage: `OrgAuth<ViewerRole>`, `OrgAuth<MemberRole>`, `OrgAuth<AdminRole>`.
pub struct OrgAuth<R: RoleRequirement> {
    pub user: users::Model,
    pub org_ctx: OrgContext,
    _role: PhantomData<R>,
}

impl<R: RoleRequirement> OrgAuth<R> {
    /// Convenience: whether this user is a platform admin.
    pub const fn is_platform_admin(&self) -> bool {
        self.org_ctx.is_platform_admin
    }
}

impl<R: RoleRequirement> FromRequestParts<AppContext> for OrgAuth<R> {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(user) = middleware::get_current_user(&jar, state).await else {
            return Err(login_redirect());
        };
        let Some(org_ctx) = middleware::get_org_context_or_default(&jar, &state.db, &user).await
        else {
            return Err(not_found());
        };
        if !org_ctx.role.at_least(R::minimum_role()) {
            return Err(forbidden());
        }
        Ok(Self {
            user,
            org_ctx,
            _role: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// PlatformAdmin extractor — requires platform admin status
// ---------------------------------------------------------------------------

/// Extracts an authenticated platform admin with org context.
/// Returns 404 (not 403) if the user is not an admin, to avoid leaking endpoint existence.
pub struct PlatformAdmin {
    pub user: users::Model,
    pub org_ctx: OrgContext,
}

impl FromRequestParts<AppContext> for PlatformAdmin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(user) = middleware::get_current_user(&jar, state).await else {
            return Err(login_redirect());
        };
        let Some(org_ctx) = middleware::get_org_context_or_default(&jar, &state.db, &user).await
        else {
            return Err(not_found());
        };
        if !org_ctx.is_platform_admin {
            return Err(not_found());
        }
        Ok(Self { user, org_ctx })
    }
}
