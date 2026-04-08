use crate::models::_entities::organizations;

/// Plan tier for an organization, read from the `plan_tier` org setting.
/// Matches the pricing page: Free → Recon → Strike → Offensive → Enterprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanTier {
    /// Free personal plan — 1 target, 1 scan, no scheduling.
    Free,
    /// Recon (€99/mo) — 1 domain, continuous ASM, email alerts.
    Recon,
    /// Strike (€299/mo) — weekly scans, scheduled scans, 2hrs AI pentesting.
    Strike,
    /// Offensive (€499/mo) — 10 domains, all features, 4hrs AI + 1hr manual.
    Offensive,
    /// Enterprise — unlimited, custom.
    Enterprise,
}

impl PlanTier {
    /// Determine the plan tier from an organization's settings.
    /// Reads the `"plan_tier"` key; defaults to `Free` if missing.
    #[must_use]
    pub fn from_org(org: &organizations::Model) -> Self {
        org.get_setting("plan_tier")
            .and_then(|v| v.as_str().map(String::from))
            .map_or(Self::Free, |s| match s.as_str() {
                "recon" => Self::Recon,
                "strike" => Self::Strike,
                "offensive" | "pro" => Self::Offensive,
                "enterprise" => Self::Enterprise,
                _ => Self::Free,
            })
    }

    /// Maximum number of scan targets allowed.
    #[must_use]
    pub const fn max_targets(&self) -> Option<usize> {
        match self {
            Self::Free | Self::Recon => Some(1),
            Self::Strike => Some(3),
            Self::Offensive => Some(10),
            Self::Enterprise => None,
        }
    }

    /// Whether auto-scheduling of recurring jobs is enabled.
    #[must_use]
    pub const fn scheduling_enabled(&self) -> bool {
        matches!(self, Self::Strike | Self::Offensive | Self::Enterprise)
    }

    /// Whether email alerts on scan diffs are enabled.
    #[must_use]
    pub const fn email_alerts_enabled(&self) -> bool {
        !matches!(self, Self::Free)
    }

    /// Human-readable label for the tier.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Recon => "Recon",
            Self::Strike => "Strike",
            Self::Offensive => "Offensive",
            Self::Enterprise => "Enterprise",
        }
    }
}
