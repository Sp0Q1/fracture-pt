//! Tiered authorization gate for scans.
//!
//! Two questions answered here:
//!
//! 1. **Passive** scans (subdomain enumeration via amass-passive / crt.sh,
//!    DNS lookups, viewdns-style intel, IP enumeration via PTR/ASN/RDAP) —
//!    do not touch the target with traffic that would look like an attack.
//!    These require only org `Member+` role.
//!
//! 2. **Active** scans (nmap, nuclei, sslscan, amass-active brute-force) —
//!    send traffic that can trigger IDS or abuse complaints. These additionally
//!    require *proof of scope* over the target:
//!    - the scan target was verified via DNS TXT (`verified_at` is set), OR
//!    - the scan target is included in an engagement that is `accepted` or
//!      `in_progress`, with the test window covering now, OR
//!    - the user is a platform admin invoking an explicit override.
//!
//! Org membership / role is *not* checked here — the calling controller is
//! responsible for that via `require_role!(org_ctx, OrgRole::Member)`. This
//! module only answers "given an authenticated member of the owning org,
//! what may they do to this target right now?".
//!
//! All decisions are decided once and returned as a value object, so the
//! UI can render the same answer (e.g., a disabled "Run nuclei" button
//! with a tooltip explaining why) without re-running the logic.

use chrono::Utc;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QuerySelect, RelationTrait,
};

use crate::models::_entities::{engagement_targets, engagements, scan_targets};

/// Whether a class of scan is allowed against a given target right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// Passive (no/low traffic on the target itself). Examples: crt.sh,
    /// passive DNS, RDAP, viewdns-style intel, amass passive sources.
    Passive,
    /// Active (traffic that hits the target). Examples: nmap, nuclei,
    /// sslscan, amass active brute-force resolution.
    Active,
}

impl ScanMode {
    /// Human-readable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Passive => "passive",
            Self::Active => "active",
        }
    }
}

/// Reasons active scanning may be denied. Surface to the UI verbatim so
/// the user knows what to do to unlock the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenialReason {
    /// Active scans require explicit scope verification or a signed engagement.
    NotVerifiedNoEngagement,
    /// The user lacks the org role to run any scan (caller responsibility,
    /// surfaced here only when the caller asks for a unified answer).
    InsufficientRole,
}

impl DenialReason {
    /// Default user-facing message. Localisation can override per locale.
    #[must_use]
    pub const fn user_message(&self) -> &'static str {
        match self {
            Self::NotVerifiedNoEngagement => {
                "Active scans require either DNS TXT verification of the target, \
                 or a signed engagement covering the target. Verify the target on \
                 the target detail page, or contact an admin."
            }
            Self::InsufficientRole => {
                "Your role does not allow scanning. Ask an org admin to grant Member access."
            }
        }
    }
}

/// Why active scanning is unlocked.
///
/// An active scan is allowed when at least one of these reasons applies. We
/// track them so the UI can show "active scans enabled because: signed
/// engagement covering target until 2026-05-12".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockReason {
    /// Target was verified via DNS TXT (`scan_targets.verified_at` is set).
    Verification,
    /// At least one accepted/in-progress engagement covers this target,
    /// with the test window covering now.
    SignedEngagement,
    /// Platform admin override (logged).
    PlatformAdminOverride,
}

/// What the caller may do with a target right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanAuth {
    pub passive_allowed: bool,
    pub active_allowed: bool,
    /// When `active_allowed` is `false`, why. UI can show this verbatim.
    pub active_denial: Option<DenialReason>,
    /// Why `active_allowed` is `true`. Empty when active is denied.
    pub unlock_reasons: Vec<UnlockReason>,
}

impl ScanAuth {
    /// Empty answer — passive denied, active denied. Used as a base.
    #[must_use]
    pub const fn deny_all(reason: DenialReason) -> Self {
        Self {
            passive_allowed: false,
            active_allowed: false,
            active_denial: Some(reason),
            unlock_reasons: Vec::new(),
        }
    }

    /// Convenience: is the requested mode allowed?
    #[must_use]
    pub const fn allows(&self, mode: ScanMode) -> bool {
        match mode {
            ScanMode::Passive => self.passive_allowed,
            ScanMode::Active => self.active_allowed,
        }
    }
}

/// Caller's role/admin context. Built once per request from the org context
/// the controller already resolved.
#[derive(Debug, Clone, Copy)]
pub struct ScanCaller {
    /// Whether the caller has at least the `Member` org role on the target's
    /// owning org. Determined by the caller (e.g., via `require_role!`).
    pub has_member_role: bool,
    /// Whether the caller is a platform admin. Platform admins can override
    /// the active-scope check — but the override is logged.
    pub is_platform_admin: bool,
}

/// Decide what the caller may run against a target right now.
///
/// # Errors
///
/// Returns an error if a database query fails. Note: a denied decision is
/// returned as `Ok(ScanAuth { ... allowed: false })`, not as `Err`.
pub async fn evaluate_scan_auth(
    db: &DatabaseConnection,
    target: &scan_targets::Model,
    caller: ScanCaller,
) -> Result<ScanAuth, sea_orm::DbErr> {
    if !caller.has_member_role && !caller.is_platform_admin {
        return Ok(ScanAuth::deny_all(DenialReason::InsufficientRole));
    }

    let passive_allowed = true;

    let mut unlock_reasons = Vec::new();
    if target.verified_at.is_some() {
        unlock_reasons.push(UnlockReason::Verification);
    }
    if target_in_signed_engagement(db, target.id).await? {
        unlock_reasons.push(UnlockReason::SignedEngagement);
    }
    if caller.is_platform_admin {
        unlock_reasons.push(UnlockReason::PlatformAdminOverride);
    }

    let active_allowed = !unlock_reasons.is_empty();
    let active_denial = if active_allowed {
        None
    } else {
        Some(DenialReason::NotVerifiedNoEngagement)
    };

    Ok(ScanAuth {
        passive_allowed,
        active_allowed,
        active_denial,
        unlock_reasons,
    })
}

/// True iff there is at least one engagement covering this scan target where
/// the engagement is `accepted` or `in_progress` and the test window (if
/// set) covers now.
async fn target_in_signed_engagement(
    db: &DatabaseConnection,
    scan_target_id: i32,
) -> Result<bool, sea_orm::DbErr> {
    let now = Utc::now();

    let count = engagement_targets::Entity::find()
        .filter(engagement_targets::Column::ScanTargetId.eq(scan_target_id))
        .join(
            JoinType::InnerJoin,
            engagement_targets::Relation::Engagements.def(),
        )
        .filter(
            engagements::Column::Status
                .eq("accepted")
                .or(engagements::Column::Status.eq("in_progress")),
        )
        .filter(
            engagements::Column::TestWindowEnd
                .is_null()
                .or(engagements::Column::TestWindowEnd.gt(now)),
        )
        .filter(
            engagements::Column::TestWindowStart
                .is_null()
                .or(engagements::Column::TestWindowStart.lte(now)),
        )
        .count(db)
        .await?;

    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_all_is_not_passive_or_active() {
        let a = ScanAuth::deny_all(DenialReason::InsufficientRole);
        assert!(!a.allows(ScanMode::Passive));
        assert!(!a.allows(ScanMode::Active));
    }

    #[test]
    fn user_message_is_non_empty() {
        assert!(!DenialReason::NotVerifiedNoEngagement
            .user_message()
            .is_empty());
        assert!(!DenialReason::InsufficientRole.user_message().is_empty());
    }

    #[test]
    fn scan_mode_label() {
        assert_eq!(ScanMode::Passive.label(), "passive");
        assert_eq!(ScanMode::Active.label(), "active");
    }
}
