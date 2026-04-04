use crate::models::_entities::organizations;

/// Plan tier for an organization, read from the `plan_tier` org setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanTier {
    Free,
    Pro,
    Enterprise,
}

impl PlanTier {
    /// Determine the plan tier from an organization's settings.
    /// Reads the `"plan_tier"` key; defaults to `Free` if missing.
    #[must_use]
    pub fn from_org(org: &organizations::Model) -> Self {
        org.get_setting("plan_tier")
            .and_then(|v| v.as_str().map(String::from))
            .map(|s| match s.as_str() {
                "pro" => Self::Pro,
                "enterprise" => Self::Enterprise,
                _ => Self::Free,
            })
            .unwrap_or(Self::Free)
    }

    /// Maximum number of scan targets allowed.
    /// `None` means unlimited.
    #[must_use]
    pub fn max_targets(&self) -> Option<usize> {
        match self {
            Self::Free => Some(1),
            Self::Pro => Some(10),
            Self::Enterprise => None,
        }
    }

    /// Whether auto-scheduling of recurring jobs is enabled.
    #[must_use]
    pub fn scheduling_enabled(&self) -> bool {
        matches!(self, Self::Pro | Self::Enterprise)
    }

    /// Whether email alerts on scan diffs are enabled.
    #[must_use]
    pub fn email_alerts_enabled(&self) -> bool {
        !matches!(self, Self::Free)
    }

    /// Whether port scans are enabled.
    #[must_use]
    pub fn port_scans_enabled(&self) -> bool {
        !matches!(self, Self::Free)
    }

    /// Human-readable label for the tier.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Pro => "Pro",
            Self::Enterprise => "Enterprise",
        }
    }
}
