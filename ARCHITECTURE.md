# gethacked.eu Architecture

## Overview

gethacked.eu is an EU-focused pentesting and security services portal built on **fracture-core** as a library dependency. It follows the same patterns as fracture-cms: Loco framework, SeaORM entities, Tera server-rendered views, OIDC authentication, and multi-tenant org-scoped data.

The application adds domain-specific models for services, pricing, engagements (with full pentest workflow), scans, findings, reports, and invoices on top of fracture-core's users, organizations, org_members, and org_invites.

---

## Account Types and Role Model

The platform has three distinct account types, all built on fracture-core's existing `org_members.role` field (Owner > Admin > Member > Viewer):

### Client Organizations
Regular orgs created via OIDC signup. Client users have standard fracture-core roles within their own org:
- **Owner** -- org settings, billing, full access
- **Admin** -- invite members, manage subscriptions, view invoices
- **Member** -- request pentests, manage targets, launch scans, view findings
- **Viewer** -- read-only access to scans, findings, reports

### Platform Admin Organization
A special org (slug: `gethacked-admin`, `is_personal: false`) where gethacked staff are members. Platform admins are identified by membership in this org, NOT by any special user flag. This avoids modifying fracture-core's user model.

Admin capabilities (checked via a helper `is_platform_admin(db, user) -> bool`):
- Review pentest scope requests from any client org
- Create and send offers/quotes
- Approve engagements
- Assign pentesters to engagements
- View all engagements across all client orgs
- Manage the service catalog and pricing tiers

### Pentesters
Pentesters are users who are assigned to specific engagements via the `pentester_assignments` table. A pentester can ONLY access engagements they are explicitly assigned to -- they cannot see other client data, other engagements, or any org data beyond their assignment scope.

Key security invariant: **A pentester viewing an engagement sees ONLY that engagement's scope and findings. They have no access to the client org's targets, scans, subscriptions, invoices, or other engagements.**

```
                  ┌─────────────────────────────────────────────────────────┐
                  │                  Authorization Model                    │
                  ├─────────────────────────────────────────────────────────┤
                  │                                                         │
                  │  Client Org Member                                      │
                  │  ├── Checked via: org_members.role + org_ctx.org.id     │
                  │  └── Sees: own org's targets, scans, findings, reports  │
                  │                                                         │
                  │  Platform Admin                                          │
                  │  ├── Checked via: is_platform_admin(db, user)           │
                  │  │   (membership in 'gethacked-admin' org)              │
                  │  └── Sees: all client engagements, can create offers    │
                  │                                                         │
                  │  Pentester                                               │
                  │  ├── Checked via: pentester_assignments table           │
                  │  │   (user_id + engagement_id)                          │
                  │  └── Sees: ONLY assigned engagement scope + findings    │
                  │                                                         │
                  └─────────────────────────────────────────────────────────┘
```

### Authorization Helper Functions

```rust
/// Check if a user is a platform admin (member of the gethacked-admin org).
pub async fn is_platform_admin(db: &DatabaseConnection, user_id: i32) -> bool {
    if let Some(admin_org) = organizations::Model::find_by_slug(db, "gethacked-admin").await {
        org_members::Model::find_membership(db, admin_org.id, user_id)
            .await
            .is_some()
    } else {
        false
    }
}

/// Check if a user is an assigned pentester for a specific engagement.
pub async fn is_assigned_pentester(
    db: &DatabaseConnection,
    user_id: i32,
    engagement_id: i32,
) -> bool {
    PentesterAssignment::find()
        .filter(pentester_assignments::Column::UserId.eq(user_id))
        .filter(pentester_assignments::Column::EngagementId.eq(engagement_id))
        .one(db)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// Macro: require the user to be a platform admin.
macro_rules! require_platform_admin {
    ($db:expr, $user:expr) => {
        if !crate::models::auth::is_platform_admin($db, $user.id).await {
            return Ok(axum::response::Response::builder()
                .status(axum::http::StatusCode::FORBIDDEN)
                .body(axum::body::Body::from("Forbidden"))
                .unwrap()
                .into_response());
        }
    };
}
```

---

## Pentest Workflow

The pentest engagement follows a strict state machine:

```
  Client                    Admin                     Pentester
    │                         │                          │
    │  1. Submit scope        │                          │
    │  ──────────────>        │                          │
    │  (status: requested)    │                          │
    │                         │                          │
    │                    2. Review scope                  │
    │                    Create offer                     │
    │  <──────────────        │                          │
    │  (status: offer_sent)   │                          │
    │                         │                          │
    │  3. Accept/Negotiate    │                          │
    │  ──────────────>        │                          │
    │  (status: accepted)     │                          │
    │                         │                          │
    │                    4. Assign pentester              │
    │                    ──────────────>                  │
    │                    (status: in_progress)            │
    │                         │                          │
    │                         │    5. Add findings        │
    │                         │    ──────────────>        │
    │                         │                          │
    │                    6. Mark review                   │
    │                    (status: review)                 │
    │                         │                          │
    │                    7. Deliver report                │
    │  <──────────────        │                          │
    │  (status: delivered)    │                          │
    │                         │                          │
    │  8. Close               │                          │
    │  (status: closed)       │                          │
```

### Engagement Status Values

| Status | Set By | Description |
|--------|--------|-------------|
| `requested` | Client | Client submitted scope, awaiting admin review |
| `offer_sent` | Admin | Admin created offer with pricing/timeline |
| `negotiating` | Client | Client responded with counter-proposal |
| `accepted` | Client | Client accepted the offer |
| `in_progress` | Admin | Pentester assigned, testing underway |
| `review` | Admin | Testing complete, report under review |
| `delivered` | Admin | Final report delivered to client |
| `closed` | Client | Client acknowledged delivery |
| `cancelled` | Either | Engagement cancelled |

---

## Project Structure

```
gethacked/
├── Cargo.toml
├── migration/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── m20260310_000001_create_services.rs
│       ├── m20260310_000002_create_pricing_tiers.rs
│       ├── m20260310_000003_create_subscriptions.rs
│       ├── m20260310_000004_create_scan_targets.rs
│       ├── m20260310_000005_create_engagements.rs
│       ├── m20260310_000006_create_engagement_offers.rs
│       ├── m20260310_000007_create_pentester_assignments.rs
│       ├── m20260310_000008_create_scan_jobs.rs
│       ├── m20260310_000009_create_findings.rs
│       ├── m20260310_000010_create_reports.rs
│       └── m20260310_000011_create_invoices.rs
├── src/
│   ├── bin/main.rs
│   ├── lib.rs
│   ├── app.rs
│   ├── controllers/
│   │   ├── mod.rs
│   │   ├── fallback.rs
│   │   ├── home.rs               -- landing page / dashboard
│   │   ├── service.rs            -- public service catalog
│   │   ├── subscription.rs       -- org subscription management
│   │   ├── scan_target.rs        -- target management + verification
│   │   ├── engagement.rs         -- pentest workflow (client side)
│   │   ├── scan_job.rs           -- automated scan management
│   │   ├── finding.rs            -- finding views (client + pentester)
│   │   ├── report.rs             -- report download
│   │   ├── invoice.rs            -- invoice management
│   │   ├── admin/
│   │   │   ├── mod.rs            -- admin routes prefix
│   │   │   ├── engagement.rs     -- review requests, create offers, assign pentesters
│   │   │   ├── finding.rs        -- admin finding management
│   │   │   ├── report.rs         -- admin report generation
│   │   │   ├── service.rs        -- service catalog CRUD
│   │   │   └── pricing_tier.rs   -- pricing tier CRUD
│   │   ├── pentester/
│   │   │   ├── mod.rs            -- pentester routes prefix
│   │   │   ├── engagement.rs     -- view assigned engagements
│   │   │   └── finding.rs        -- add/edit findings
│   │   └── api/
│   │       ├── mod.rs
│   │       ├── scan.rs
│   │       └── report.rs
│   ├── models/
│   │   ├── mod.rs
│   │   ├── auth.rs               -- is_platform_admin, is_assigned_pentester helpers
│   │   ├── _entities/
│   │   │   ├── mod.rs
│   │   │   ├── prelude.rs
│   │   │   ├── services.rs
│   │   │   ├── pricing_tiers.rs
│   │   │   ├── subscriptions.rs
│   │   │   ├── scan_targets.rs
│   │   │   ├── engagements.rs
│   │   │   ├── engagement_offers.rs
│   │   │   ├── pentester_assignments.rs
│   │   │   ├── scan_jobs.rs
│   │   │   ├── findings.rs
│   │   │   ├── reports.rs
│   │   │   └── invoices.rs
│   │   ├── services.rs
│   │   ├── pricing_tiers.rs
│   │   ├── subscriptions.rs
│   │   ├── scan_targets.rs
│   │   ├── engagements.rs
│   │   ├── engagement_offers.rs
│   │   ├── pentester_assignments.rs
│   │   ├── scan_jobs.rs
│   │   ├── findings.rs
│   │   ├── reports.rs
│   │   └── invoices.rs
│   ├── views/
│   │   ├── mod.rs
│   │   ├── home.rs
│   │   ├── service.rs
│   │   ├── subscription.rs
│   │   ├── scan_target.rs
│   │   ├── engagement.rs
│   │   ├── scan_job.rs
│   │   ├── finding.rs
│   │   ├── report.rs
│   │   ├── invoice.rs
│   │   ├── admin/
│   │   │   ├── mod.rs
│   │   │   ├── engagement.rs
│   │   │   └── finding.rs
│   │   └── pentester/
│   │       ├── mod.rs
│   │       ├── engagement.rs
│   │       └── finding.rs
│   ├── initializers/
│   │   └── mod.rs
│   ├── mailers/
│   │   └── mod.rs
│   └── workers/
│       ├── mod.rs
│       └── scan_runner.rs
├── assets/
│   └── views/
│       ├── home/
│       │   ├── index.html              (landing page - guest)
│       │   └── dashboard.html          (authenticated dashboard)
│       ├── service/
│       │   ├── list.html               (service catalog)
│       │   └── show.html               (service detail + pricing)
│       ├── subscription/
│       │   ├── list.html
│       │   └── show.html
│       ├── scan_target/
│       │   ├── list.html
│       │   ├── create.html
│       │   └── show.html
│       ├── engagement/
│       │   ├── list.html               (client: my engagements)
│       │   ├── request.html            (client: scope submission form)
│       │   ├── show.html               (client: engagement detail + offer)
│       │   └── offer_response.html     (client: accept/negotiate offer)
│       ├── scan_job/
│       │   ├── list.html
│       │   └── show.html
│       ├── finding/
│       │   ├── list.html
│       │   └── show.html
│       ├── report/
│       │   ├── list.html
│       │   └── show.html
│       ├── invoice/
│       │   ├── list.html
│       │   └── show.html
│       ├── admin/
│       │   ├── engagement/
│       │   │   ├── list.html           (all client requests)
│       │   │   ├── show.html           (review scope)
│       │   │   ├── offer.html          (create offer form)
│       │   │   └── assign.html         (assign pentester)
│       │   └── finding/
│       │       └── review.html         (review findings before delivery)
│       ├── pentester/
│       │   ├── engagement/
│       │   │   ├── list.html           (my assignments)
│       │   │   └── show.html           (engagement scope + findings)
│       │   └── finding/
│       │       ├── create.html         (add finding form)
│       │       └── edit.html           (edit finding)
│       └── shared/
│           ├── base.html               (layout with nav, footer)
│           └── pricing.html            (pricing comparison table)
├── config/
│   ├── development.yaml
│   └── production.yaml
├── Containerfile
└── compose.yaml
```

---

## Data Models

### Entity Relationship Diagram

```
fracture-core (provided):
┌────────────┐     ┌──────────────┐     ┌───────────────┐
│   users     │────<│ org_members   │>────│ organizations │
│             │     │              │     │               │
│ id, pid     │     │ id           │     │ id, pid       │
│ email       │     │ org_id (FK)  │     │ name          │
│ name        │     │ user_id (FK) │     │ slug          │
│ oidc_*      │     │ role         │     │ is_personal   │
└────────────┘     └──────────────┘     └───────────────┘
      │                                        │
      │ (pentester)                            │ has_many (all org-scoped tables)
      ▼                                        ▼
gethacked (new):

GLOBAL TABLES (not org-scoped):
┌───────────────┐     ┌────────────────┐
│  services     │────<│ pricing_tiers  │
│               │     │                │
│ id, pid       │     │ id, pid        │
│ name          │     │ service_id(FK) │
│ slug          │     │ name           │
│ category      │     │ price_cents    │
│ description   │     │ billing_period │
│ is_automated  │     │ max_targets    │
│ is_active     │     │ max_scans_month│
│ sort_order    │     │ features (JSON)│
└───────────────┘     │ is_active      │
                      └────────────────┘

ORG-SCOPED TABLES:
┌────────────────┐
│ subscriptions  │
│                │
│ id, pid        │
│ org_id (FK)    │
│ tier_id (FK)   │
│ status         │
│ starts_at      │
│ expires_at     │
│ stripe_sub_id  │
└────────────────┘

┌────────────────┐     ┌──────────────────────┐     ┌──────────────────────────┐
│ scan_targets   │────<│  scan_jobs            │────<│  findings                │
│                │     │                       │     │  (scan OR engagement)    │
│ id, pid        │     │ id, pid               │     │                          │
│ org_id (FK)    │     │ org_id (FK)           │     │ id, pid                  │
│ hostname       │     │ target_id (FK)        │     │ org_id (FK)              │
│ ip_address     │     │ service_id (FK)       │     │ engagement_id (FK, null) │
│ target_type    │     │ status                │     │ job_id (FK, null)        │
│ verified_at    │     │ scheduled_at          │     │ created_by_user_id (FK)  │
│ verification   │     │ started_at            │     │ title                    │
│ _method        │     │ completed_at          │     │ description (business)   │
│ label          │     │ result_summary        │     │ technical_description    │
└────────────────┘     │ finding_count         │     │ impact                   │
                       └───────────────────────┘     │ recommendation           │
                                                     │ severity                 │
PENTEST WORKFLOW:                                    │ cve_id                   │
┌────────────────┐     ┌────────────────────┐        │ category                 │
│ engagements    │────<│ engagement_offers   │        │ status                   │
│                │     │                     │        │ affected_asset           │
│ id, pid        │     │ id, pid             │        │ evidence                 │
│ org_id (FK)    │     │ engagement_id (FK)  │        └──────────────────────────┘
│ service_id(FK) │     │ created_by (FK)     │
│ title          │     │ amount_cents        │
│ status         │     │ currency            │     ┌──────────────────────────┐
│                │     │ timeline_days       │     │ pentester_assignments    │
│ -- SCOPE --    │     │ deliverables (Text) │     │                          │
│ target_systems │     │ valid_until         │     │ id                       │
│ ip_ranges      │     │ status              │     │ engagement_id (FK)       │
│ domains        │     │ client_response     │     │ user_id (FK)             │
│ test_window_*  │     │                     │     │ assigned_by_user_id (FK) │
│ exclusions     │     └─────────────────────┘     │ role                     │
│ contact_name   │                                 │                          │
│ contact_email  │                                 └──────────────────────────┘
│ contact_phone  │
│ rules_of_      │     ┌────────────────┐     ┌────────────────┐
│  engagement    │     │  reports       │     │  invoices      │
│                │     │                │     │                │
│ -- ADMIN --    │     │ id, pid        │     │ id, pid        │
│ requested_at   │     │ org_id (FK)    │     │ org_id (FK)    │
│ starts_at      │     │ engagement_id  │     │ subscription_id│
│ completed_at   │     │  (FK, nullable)│     │  (FK, nullable)│
│ price_cents    │     │ job_id         │     │ engagement_id  │
│ currency       │     │  (FK, nullable)│     │  (FK, nullable)│
│ admin_notes    │     │ title          │     │ amount_cents   │
└────────────────┘     │ report_type    │     │ currency       │
                       │ format         │     │ status         │
                       │ storage_path   │     │ issued_at      │
                       │ generated_at   │     │ due_at         │
                       └────────────────┘     │ paid_at        │
                                              │ stripe_inv_id  │
                                              │ pdf_path       │
                                              └────────────────┘
```

---

## Detailed Entity Definitions

### 1. `services` -- Service Catalog

Global table (not org-scoped). Defines the services gethacked offers.

```rust
// src/models/_entities/services.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "services")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub name: String,                    // "Vulnerability Scanning"
    #[sea_orm(unique)]
    pub slug: String,                    // "vulnerability-scanning"
    pub category: String,               // "automated" | "manual" | "compliance"
    #[sea_orm(column_type = "Text")]
    pub description: String,            // Markdown description
    pub is_automated: bool,             // true = scan-based, false = manual engagement
    pub is_active: bool,                // visible in catalog
    pub sort_order: i32,                // display ordering
}
```

**Relations:** `has_many` PricingTiers, ScanJobs, Engagements.

**Services to seed:**
| Name | Category | Automated |
|------|----------|-----------|
| Vulnerability Scanning | automated | true |
| Attack Surface Mapping | automated | true |
| Web Application Security Assessment | manual | false |
| API Security Testing | manual | false |
| Penetration Testing | manual | false |
| Cloud Security Review | manual | false |
| GDPR Compliance Gap Analysis | compliance | false |
| NIS2 Compliance Gap Analysis | compliance | false |
| ISO 27001 Gap Analysis | compliance | false |

### 2. `pricing_tiers` -- Pricing Plans

Global table. Each service can have multiple pricing tiers.

```rust
// src/models/_entities/pricing_tiers.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "pricing_tiers")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub service_id: i32,                // FK -> services
    pub name: String,                   // "Starter", "Professional", "Business", "Enterprise"
    pub slug: String,                   // "starter"
    pub price_cents: i32,               // 9900 = EUR 99.00 (0 for "Contact us")
    pub billing_period: String,         // "monthly" | "yearly" | "one-time" | "custom"
    pub max_targets: i32,               // max IPs/domains, 0 = unlimited
    pub max_scans_per_month: i32,       // 0 = unlimited
    #[sea_orm(column_type = "Text")]
    pub features: String,               // JSON array of feature strings
    pub is_active: bool,
    pub sort_order: i32,
}
```

**Relations:** `belongs_to` Services, `has_many` Subscriptions.

**Reference tiers (for automated scanning services):**
| Tier | Price | Targets | Scans/mo |
|------|-------|---------|----------|
| Starter | EUR 99/yr | 5 | 10 |
| Professional | EUR 249/yr | 25 | 50 |
| Business | EUR 499/yr | 100 | unlimited |
| Enterprise | custom | unlimited | unlimited |

### 3. `subscriptions` -- Org Subscriptions

Org-scoped. Links an org to a pricing tier with billing state.

```rust
// src/models/_entities/subscriptions.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "subscriptions")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,                    // FK -> organizations
    pub tier_id: i32,                   // FK -> pricing_tiers
    pub status: String,                 // "active" | "past_due" | "cancelled" | "trialing"
    pub starts_at: DateTimeWithTimeZone,
    pub expires_at: Option<DateTimeWithTimeZone>,
    pub cancelled_at: Option<DateTimeWithTimeZone>,
    pub stripe_subscription_id: Option<String>,
    pub stripe_customer_id: Option<String>,
}
```

**Relations:** `belongs_to` Organizations, PricingTiers. `has_many` Invoices.

### 4. `scan_targets` -- Verified Scan Targets

Org-scoped. Hosts/IPs the org has registered and verified ownership of.

```rust
// src/models/_entities/scan_targets.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "scan_targets")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,                    // FK -> organizations
    pub hostname: Option<String>,       // "example.com"
    pub ip_address: Option<String>,     // "93.184.216.34"
    pub target_type: String,            // "domain" | "ip" | "cidr" | "url"
    pub verified_at: Option<DateTimeWithTimeZone>,
    pub verification_method: Option<String>, // "dns_txt" | "http_file" | "manual"
    pub verification_token: Option<String>,  // random token for DNS/HTTP verification
    pub label: Option<String>,          // user-friendly label
}
```

**Relations:** `belongs_to` Organizations. `has_many` ScanJobs.

### 5. `engagements` -- Pentest Engagements (Full Workflow)

Org-scoped. The central model for the pentest workflow. Contains both client-submitted scope AND admin-managed fields.

```rust
// src/models/_entities/engagements.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "engagements")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,                    // FK -> organizations (client org)
    pub service_id: i32,                // FK -> services
    pub title: String,                  // "Q1 2026 Pentest - Production"

    // -- Workflow status --
    pub status: String,                 // "requested" | "offer_sent" | "negotiating" |
                                        // "accepted" | "in_progress" | "review" |
                                        // "delivered" | "closed" | "cancelled"

    // -- Client-submitted scope (filled at request time) --
    #[sea_orm(column_type = "Text")]
    pub target_systems: String,         // description of systems to test
    #[sea_orm(column_type = "Text", nullable)]
    pub ip_ranges: Option<String>,      // IP ranges in scope (one per line)
    #[sea_orm(column_type = "Text", nullable)]
    pub domains: Option<String>,        // domains in scope (one per line)
    #[sea_orm(column_type = "Text", nullable)]
    pub exclusions: Option<String>,     // systems/areas excluded from testing
    pub test_window_start: Option<DateTimeWithTimeZone>,  // preferred testing window
    pub test_window_end: Option<DateTimeWithTimeZone>,
    pub contact_name: String,           // primary contact for the engagement
    pub contact_email: String,
    pub contact_phone: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub rules_of_engagement: Option<String>,  // special rules, constraints

    // -- Admin-managed fields --
    pub requested_at: DateTimeWithTimeZone,
    pub starts_at: Option<DateTimeWithTimeZone>,      // actual start date
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub price_cents: Option<i32>,                     // agreed price (from accepted offer)
    pub currency: Option<String>,                     // "EUR"
    #[sea_orm(column_type = "Text", nullable)]
    pub admin_notes: Option<String>,    // internal notes (never shown to client or pentester)
}
```

**Relations:** `belongs_to` Organizations, Services. `has_many` EngagementOffers, PentesterAssignments, Findings, Reports, Invoices.

### 6. `engagement_offers` -- Offers/Quotes

Linked to engagements. Admin creates offers; client accepts or negotiates. Multiple offers can exist per engagement (negotiation history).

```rust
// src/models/_entities/engagement_offers.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "engagement_offers")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub engagement_id: i32,             // FK -> engagements
    pub created_by_user_id: i32,        // FK -> users (admin who created the offer)
    pub amount_cents: i32,              // price in cents (e.g., 500000 = EUR 5,000.00)
    pub currency: String,               // "EUR"
    pub timeline_days: i32,             // estimated duration in business days
    #[sea_orm(column_type = "Text")]
    pub deliverables: String,           // what the client will receive (Markdown)
    #[sea_orm(column_type = "Text", nullable)]
    pub terms: Option<String>,          // additional terms/conditions
    pub valid_until: DateTimeWithTimeZone, // offer expiration
    pub status: String,                 // "pending" | "accepted" | "rejected" | "superseded"
    #[sea_orm(column_type = "Text", nullable)]
    pub client_response: Option<String>, // client's response if negotiating
}
```

**Relations:** `belongs_to` Engagements, Users.

### 7. `pentester_assignments` -- Pentester-to-Engagement Mapping

Links a pentester user to a specific engagement. This is the ONLY way a pentester gains access to engagement data.

```rust
// src/models/_entities/pentester_assignments.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "pentester_assignments")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    pub engagement_id: i32,             // FK -> engagements
    pub user_id: i32,                   // FK -> users (the pentester)
    pub assigned_by_user_id: i32,       // FK -> users (the admin who assigned)
    pub role: String,                   // "lead" | "member" (lead can manage other pentesters)
}
```

**Relations:** `belongs_to` Engagements, Users (x2).

**Unique constraint:** `(engagement_id, user_id)` -- a pentester can only be assigned once per engagement.

### 8. `scan_jobs` -- Automated Scan Runs

Org-scoped. Each execution of an automated scan against a target.

```rust
// src/models/_entities/scan_jobs.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "scan_jobs")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,                    // FK -> organizations
    pub target_id: i32,                 // FK -> scan_targets
    pub service_id: i32,                // FK -> services
    pub status: String,                 // "queued" | "running" | "completed" | "failed" | "cancelled"
    pub scheduled_at: Option<DateTimeWithTimeZone>,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    #[sea_orm(column_type = "Text", nullable)]
    pub result_summary: Option<String>, // JSON summary of findings count by severity
    pub finding_count: i32,             // denormalized count
    pub error_message: Option<String>,  // if status = "failed"
}
```

**Relations:** `belongs_to` Organizations, ScanTargets, Services. `has_many` Findings, Reports.

### 9. `findings` -- Vulnerability Findings

Org-scoped. Supports BOTH automated scan findings AND manual pentest findings. Exactly one of `job_id` or `engagement_id` must be set.

```rust
// src/models/_entities/findings.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "findings")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,                    // FK -> organizations

    // -- Source: exactly one of these must be set --
    pub engagement_id: Option<i32>,     // FK -> engagements (manual pentest finding)
    pub job_id: Option<i32>,            // FK -> scan_jobs (automated scan finding)

    // -- Who created it --
    pub created_by_user_id: Option<i32>, // FK -> users (pentester who added it, null for automated)

    // -- Finding content (enriched for pentest workflow) --
    pub title: String,                  // "SQL Injection in Login Form"

    #[sea_orm(column_type = "Text")]
    pub description: String,            // Business-level explanation: what was found, why it matters

    #[sea_orm(column_type = "Text", nullable)]
    pub technical_description: Option<String>,  // Reproduction steps, technical details,
                                                // request/response examples

    #[sea_orm(column_type = "Text", nullable)]
    pub impact: Option<String>,         // What could happen if exploited (business impact)

    #[sea_orm(column_type = "Text", nullable)]
    pub recommendation: Option<String>, // How to fix it (remediation guidance)

    // -- Classification --
    pub severity: String,               // "extreme" | "high" | "elevated" | "moderate" | "low"
    pub cve_id: Option<String>,         // "CVE-2024-1234"
    pub category: String,              // "injection" | "broken_auth" | "xss" | "idor" |
                                        // "misconfig" | "crypto" | "ssrf" | "insecure_design" |
                                        // "vulnerable_components" | "logging_monitoring" | "other"

    // -- Evidence --
    #[sea_orm(column_type = "Text", nullable)]
    pub evidence: Option<String>,       // Proof: screenshots refs, raw output, request/response

    pub affected_asset: Option<String>, // specific host:port or URL

    // -- Status tracking --
    pub status: String,                 // "draft" | "confirmed" | "open" | "acknowledged" |
                                        // "mitigated" | "false_positive" | "accepted_risk"
}
```

**Relations:** `belongs_to` Organizations, Engagements (optional), ScanJobs (optional), Users (optional).

**Finding categories** (aligned with OWASP Top 10 2021):
| Category Slug | Display Name |
|---------------|-------------|
| `injection` | Injection (SQL, NoSQL, OS, LDAP) |
| `broken_auth` | Broken Authentication |
| `xss` | Cross-Site Scripting |
| `idor` | Insecure Direct Object References |
| `misconfig` | Security Misconfiguration |
| `crypto` | Cryptographic Failures |
| `ssrf` | Server-Side Request Forgery |
| `insecure_design` | Insecure Design |
| `vulnerable_components` | Vulnerable and Outdated Components |
| `logging_monitoring` | Security Logging and Monitoring Failures |
| `other` | Other |

### 10. `reports` -- Generated Reports

Org-scoped. PDF/HTML reports from scans or engagements.

```rust
// src/models/_entities/reports.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "reports")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,                    // FK -> organizations
    pub engagement_id: Option<i32>,     // FK -> engagements (nullable)
    pub job_id: Option<i32>,            // FK -> scan_jobs (nullable)
    pub title: String,                  // "Penetration Test Report - Acme Corp - 2026-03"
    pub report_type: String,            // "scan_summary" | "pentest" | "compliance" | "executive"
    pub format: String,                 // "pdf" | "html"
    pub storage_path: Option<String>,   // path to generated file
    pub generated_at: Option<DateTimeWithTimeZone>,
}
```

**Relations:** `belongs_to` Organizations, Engagements (optional), ScanJobs (optional).

### 11. `invoices` -- Billing Records

Org-scoped. Invoices for subscriptions and one-off engagements.

```rust
// src/models/_entities/invoices.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "invoices")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub pid: Uuid,
    pub org_id: i32,                    // FK -> organizations
    pub subscription_id: Option<i32>,   // FK -> subscriptions (nullable)
    pub engagement_id: Option<i32>,     // FK -> engagements (nullable)
    pub amount_cents: i32,              // 49900 = EUR 499.00
    pub currency: String,              // "EUR"
    pub status: String,                // "draft" | "issued" | "paid" | "overdue" | "cancelled"
    pub issued_at: Option<DateTimeWithTimeZone>,
    pub due_at: Option<DateTimeWithTimeZone>,
    pub paid_at: Option<DateTimeWithTimeZone>,
    pub stripe_invoice_id: Option<String>,
    pub pdf_path: Option<String>,       // path to generated PDF
    #[sea_orm(column_type = "Text", nullable)]
    pub line_items: Option<String>,     // JSON array of line items
}
```

**Relations:** `belongs_to` Organizations, Subscriptions (optional), Engagements (optional).

---

## Migration Plan

Migrations chain after fracture-core's migrations. Each migration follows the exact pattern from `m20260220_000004_create_projects.rs`:

```
fracture-core migrations (provided):
  m20220101_000001_users
  m20260215_000001_add_oidc_to_users
  m20260217_000001_add_pid_to_movies      (legacy, no-op)
  m20260218_000001_add_session_invalidated_at
  m20260220_000001_create_organizations
  m20260220_000002_create_org_members
  m20260220_000003_create_org_invites
  m20260220_000006_drop_movies            (legacy cleanup)

gethacked migrations (new):
  m20260310_000001_create_services
  m20260310_000002_create_pricing_tiers       (FK -> services)
  m20260310_000003_create_subscriptions       (FK -> organizations, pricing_tiers)
  m20260310_000004_create_scan_targets        (FK -> organizations)
  m20260310_000005_create_engagements         (FK -> organizations, services)
  m20260310_000006_create_engagement_offers   (FK -> engagements, users)
  m20260310_000007_create_pentester_assignments (FK -> engagements, users x2)
  m20260310_000008_create_scan_jobs           (FK -> organizations, scan_targets, services)
  m20260310_000009_create_findings            (FK -> organizations, engagements?, scan_jobs?, users?)
  m20260310_000010_create_reports             (FK -> organizations, engagements?, scan_jobs?)
  m20260310_000011_create_invoices            (FK -> organizations, subscriptions?, engagements?)
```

**Foreign key cascade rules:**
- All `org_id` FKs: `ON DELETE CASCADE` (deleting an org removes all its data)
- `service_id` FKs: `ON DELETE RESTRICT` (cannot delete a service with linked data)
- `engagement_id` on offers/assignments: `ON DELETE CASCADE` (deleting engagement removes its offers and assignments)
- `target_id`, `tier_id`: `ON DELETE CASCADE`
- Nullable FKs (`engagement_id`, `job_id` on findings/reports; `subscription_id`, `engagement_id` on invoices): `ON DELETE SET NULL`
- `created_by_user_id`, `assigned_by_user_id`: `ON DELETE SET NULL` (preserve data if user is deleted)

**Indexes:**
- Every table: unique index on `pid`
- `scan_targets`: index on `(org_id, hostname)`
- `scan_jobs`: index on `(org_id, status)`, index on `(target_id)`
- `findings`: index on `(org_id, severity)`, index on `(job_id)`, index on `(engagement_id)`
- `subscriptions`: index on `(org_id, status)`
- `engagements`: index on `(org_id, status)`
- `engagement_offers`: index on `(engagement_id, status)`
- `pentester_assignments`: unique index on `(engagement_id, user_id)`, index on `(user_id)`

---

## migration/src/lib.rs

```rust
#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod m20260310_000001_create_services;
mod m20260310_000002_create_pricing_tiers;
mod m20260310_000003_create_subscriptions;
mod m20260310_000004_create_scan_targets;
mod m20260310_000005_create_engagements;
mod m20260310_000006_create_engagement_offers;
mod m20260310_000007_create_pentester_assignments;
mod m20260310_000008_create_scan_jobs;
mod m20260310_000009_create_findings;
mod m20260310_000010_create_reports;
mod m20260310_000011_create_invoices;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        fracture_core_migration::Migrator::migrations()
            .into_iter()
            .chain(vec![
                Box::new(m20260310_000001_create_services::Migration) as Box<dyn MigrationTrait>,
                Box::new(m20260310_000002_create_pricing_tiers::Migration),
                Box::new(m20260310_000003_create_subscriptions::Migration),
                Box::new(m20260310_000004_create_scan_targets::Migration),
                Box::new(m20260310_000005_create_engagements::Migration),
                Box::new(m20260310_000006_create_engagement_offers::Migration),
                Box::new(m20260310_000007_create_pentester_assignments::Migration),
                Box::new(m20260310_000008_create_scan_jobs::Migration),
                Box::new(m20260310_000009_create_findings::Migration),
                Box::new(m20260310_000010_create_reports::Migration),
                Box::new(m20260310_000011_create_invoices::Migration),
            ])
            .collect()
    }
}
```

---

## Controller / Route Structure

### Public Routes (no auth required)

| Method | Path | Controller | Description |
|--------|------|------------|-------------|
| GET | `/` | `home::index` | Landing page (guest) or dashboard (authenticated) |
| GET | `/services` | `service::list` | Service catalog (public) |
| GET | `/services/:slug` | `service::show` | Service detail + pricing tiers (public) |
| GET | `/pricing` | `home::pricing` | Pricing comparison page (public) |

### Client Routes (org-scoped, require login + org membership)

**Dashboard:**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/dashboard` | Viewer | Org dashboard with scan summary, engagement status |

**Subscriptions:**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/subscriptions` | Viewer | List org subscriptions |
| GET | `/subscriptions/:pid` | Viewer | Subscription detail |
| POST | `/subscriptions` | Admin | Create subscription (Stripe checkout) |
| POST | `/subscriptions/:pid/cancel` | Admin | Cancel subscription |

**Scan Targets:**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/targets` | Viewer | List scan targets |
| GET | `/targets/new` | Member | New target form |
| POST | `/targets` | Member | Add target |
| GET | `/targets/:pid` | Viewer | Target detail + verification status |
| POST | `/targets/:pid/verify` | Member | Initiate verification |
| DELETE | `/targets/:pid` | Member | Remove target |

**Scan Jobs:**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/scans` | Viewer | List scan jobs |
| POST | `/scans` | Member | Launch a scan |
| GET | `/scans/:pid` | Viewer | Scan results + findings |

**Engagements (Client View):**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/engagements` | Viewer | List org's engagements |
| GET | `/engagements/new` | Member | Scope submission form |
| POST | `/engagements` | Member | Submit pentest request |
| GET | `/engagements/:pid` | Viewer | Engagement detail (scope, offer, findings, timeline) |
| POST | `/engagements/:pid/respond` | Member | Accept or negotiate offer |

**Findings (Client View -- read-only for engagement findings, status updates for scan findings):**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/findings` | Viewer | All findings across scans + engagements |
| GET | `/findings/:pid` | Viewer | Finding detail (full report view) |
| POST | `/findings/:pid/status` | Member | Update finding status (scan findings only) |

**Reports:**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/reports` | Viewer | List reports |
| GET | `/reports/:pid` | Viewer | Report detail |
| GET | `/reports/:pid/download` | Viewer | Download PDF |

**Invoices:**
| Method | Path | Role | Description |
|--------|------|------|-------------|
| GET | `/invoices` | Admin | List invoices |
| GET | `/invoices/:pid` | Admin | Invoice detail |
| GET | `/invoices/:pid/download` | Admin | Download PDF |

### Admin Routes (require platform admin)

All admin routes are prefixed with `/admin` and require `is_platform_admin` check.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/admin/engagements` | PlatformAdmin | List all engagement requests across all orgs |
| GET | `/admin/engagements/:pid` | PlatformAdmin | Review engagement scope |
| GET | `/admin/engagements/:pid/offer` | PlatformAdmin | Create offer form |
| POST | `/admin/engagements/:pid/offer` | PlatformAdmin | Submit offer |
| POST | `/admin/engagements/:pid/status` | PlatformAdmin | Update engagement status |
| GET | `/admin/engagements/:pid/assign` | PlatformAdmin | Assign pentester form |
| POST | `/admin/engagements/:pid/assign` | PlatformAdmin | Assign pentester |
| POST | `/admin/engagements/:pid/unassign/:user_pid` | PlatformAdmin | Remove pentester |
| GET | `/admin/engagements/:pid/report` | PlatformAdmin | Generate/review report |
| POST | `/admin/engagements/:pid/report` | PlatformAdmin | Create and deliver report |
| GET | `/admin/services` | PlatformAdmin | Service catalog management |
| POST | `/admin/services` | PlatformAdmin | Create service |
| POST | `/admin/services/:pid` | PlatformAdmin | Update service |
| GET | `/admin/pricing` | PlatformAdmin | Pricing tier management |
| POST | `/admin/pricing` | PlatformAdmin | Create pricing tier |
| POST | `/admin/pricing/:pid` | PlatformAdmin | Update pricing tier |

### Pentester Routes (require pentester assignment)

All pentester routes are prefixed with `/pentester` and scoped to assigned engagements only.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/pentester/engagements` | AssignedPentester | List my assigned engagements |
| GET | `/pentester/engagements/:pid` | AssignedPentester | View engagement scope |
| GET | `/pentester/engagements/:pid/findings` | AssignedPentester | List findings for engagement |
| GET | `/pentester/engagements/:pid/findings/new` | AssignedPentester | Add finding form |
| POST | `/pentester/engagements/:pid/findings` | AssignedPentester | Create finding |
| GET | `/pentester/engagements/:pid/findings/:fid/edit` | AssignedPentester | Edit finding form |
| POST | `/pentester/engagements/:pid/findings/:fid` | AssignedPentester | Update finding |
| DELETE | `/pentester/engagements/:pid/findings/:fid` | AssignedPentester | Delete finding (draft only) |

### JSON API Routes (for API-key access, Pro+ tiers)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/targets` | List targets |
| POST | `/api/v1/scans` | Launch scan |
| GET | `/api/v1/scans/:pid` | Scan status + results |
| GET | `/api/v1/scans/:pid/findings` | Findings for a scan |
| GET | `/api/v1/reports/:pid` | Report metadata |
| GET | `/api/v1/reports/:pid/download` | Download report |

---

## src/app.rs

```rust
use crate::{
    controllers, initializers,
    models::_entities::{
        engagement_offers, engagements, findings, invoices, org_invites, org_members,
        organizations, pentester_assignments, pricing_tiers, reports, scan_jobs, scan_targets,
        services, subscriptions, users,
    },
};

impl Hooks for App {
    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes()
            // Core routes
            .add_route(controllers::org::routes())
            .add_route(controllers::org::invite_routes())
            .add_route(controllers::oidc::routes())
            // Client routes
            .add_route(controllers::home::routes())
            .add_route(controllers::service::routes())
            .add_route(controllers::subscription::routes())
            .add_route(controllers::scan_target::routes())
            .add_route(controllers::scan_job::routes())
            .add_route(controllers::finding::routes())
            .add_route(controllers::engagement::routes())
            .add_route(controllers::report::routes())
            .add_route(controllers::invoice::routes())
            // Admin routes
            .add_route(controllers::admin::routes())
            // Pentester routes
            .add_route(controllers::pentester::routes())
            // API routes
            .add_route(controllers::api::routes())
    }

    async fn truncate(ctx: &AppContext) -> Result<()> {
        // Children first (reverse FK dependency order)
        truncate_table(&ctx.db, findings::Entity).await?;
        truncate_table(&ctx.db, reports::Entity).await?;
        truncate_table(&ctx.db, pentester_assignments::Entity).await?;
        truncate_table(&ctx.db, engagement_offers::Entity).await?;
        truncate_table(&ctx.db, scan_jobs::Entity).await?;
        truncate_table(&ctx.db, invoices::Entity).await?;
        truncate_table(&ctx.db, engagements::Entity).await?;
        truncate_table(&ctx.db, subscriptions::Entity).await?;
        truncate_table(&ctx.db, scan_targets::Entity).await?;
        truncate_table(&ctx.db, pricing_tiers::Entity).await?;
        truncate_table(&ctx.db, services::Entity).await?;
        // Core tables
        truncate_table(&ctx.db, org_invites::Entity).await?;
        truncate_table(&ctx.db, org_members::Entity).await?;
        truncate_table(&ctx.db, organizations::Entity).await?;
        truncate_table(&ctx.db, users::Entity).await?;
        Ok(())
    }
}
```

---

## Module Wiring

### src/lib.rs

```rust
pub mod app;
pub mod controllers;
pub mod initializers;
pub mod mailers;
pub mod models;
pub mod views;
pub mod workers;

pub use fracture_core::{require_role, require_user};
```

### src/controllers/mod.rs

```rust
pub mod fallback;
pub mod home;
pub mod service;
pub mod subscription;
pub mod scan_target;
pub mod scan_job;
pub mod finding;
pub mod engagement;
pub mod report;
pub mod invoice;
pub mod admin;
pub mod pentester;
pub mod api;

pub use fracture_core::controllers::{middleware, oidc, oidc_state, org};
```

### src/models/mod.rs

```rust
pub mod _entities;
pub mod auth;                   // is_platform_admin, is_assigned_pentester
pub mod services;
pub mod pricing_tiers;
pub mod subscriptions;
pub mod scan_targets;
pub mod engagements;
pub mod engagement_offers;
pub mod pentester_assignments;
pub mod scan_jobs;
pub mod findings;
pub mod reports;
pub mod invoices;

pub use fracture_core::models::{org_invites, org_members, organizations, users};
```

### src/models/_entities/mod.rs

```rust
pub mod prelude;
pub mod services;
pub mod pricing_tiers;
pub mod subscriptions;
pub mod scan_targets;
pub mod engagements;
pub mod engagement_offers;
pub mod pentester_assignments;
pub mod scan_jobs;
pub mod findings;
pub mod reports;
pub mod invoices;

pub use fracture_core::models::_entities::{org_invites, org_members, organizations, users};
```

### src/models/_entities/prelude.rs

```rust
pub use super::services::Entity as Services;
pub use super::pricing_tiers::Entity as PricingTiers;
pub use super::subscriptions::Entity as Subscriptions;
pub use super::scan_targets::Entity as ScanTargets;
pub use super::engagements::Entity as Engagements;
pub use super::engagement_offers::Entity as EngagementOffers;
pub use super::pentester_assignments::Entity as PentesterAssignments;
pub use super::scan_jobs::Entity as ScanJobs;
pub use super::findings::Entity as Findings;
pub use super::reports::Entity as Reports;
pub use super::invoices::Entity as Invoices;

pub use fracture_core::models::_entities::prelude::{OrgInvites, OrgMembers, Organizations, Users};
```

### src/views/mod.rs

```rust
pub mod home;
pub mod service;
pub mod subscription;
pub mod scan_target;
pub mod scan_job;
pub mod finding;
pub mod engagement;
pub mod report;
pub mod invoice;
pub mod admin;
pub mod pentester;

pub use fracture_core::views::{base_context, org};
```

---

## Cargo.toml

```toml
[workspace]
members = [".", "migration"]

[package]
name = "gethacked"
description = "EU-focused pentesting and security services portal"
version = "0.1.0"
edition = "2021"
license = "AGPL-3.0-or-later"
publish = false
default-run = "gethacked-cli"

[workspace.dependencies]
loco-rs = { version = "0.16" }
sea-orm = { version = "1.1", features = [
  "sqlx-sqlite",
  "sqlx-postgres",
  "runtime-tokio-rustls",
  "macros",
] }

[dependencies]
loco-rs = { workspace = true }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1" }
tokio = { version = "1.50", default-features = false, features = [
  "rt-multi-thread",
] }
async-trait = { version = "0.1" }
axum = { version = "0.8" }
tracing = { version = "0.1" }
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
migration = { path = "migration" }
fracture-core = { path = "../fracture-cms/fracture-core" }
sea-orm = { workspace = true }
chrono = { version = "0.4" }
uuid = { version = "1.22", features = ["v4"] }
fluent-templates = { version = "0.13", features = ["tera"] }
unic-langid = { version = "0.9" }
axum-extra = { version = "0.10", features = ["form", "cookie"] }
slug = { version = "0.1" }

[[bin]]
name = "gethacked-cli"
path = "src/bin/main.rs"
required-features = []

[dev-dependencies]
loco-rs = { workspace = true, features = ["testing"] }
serial_test = { version = "3.4" }
rstest = { version = "0.26" }
insta = { version = "1.46", features = ["redactions", "yaml", "filters"] }
```

### migration/Cargo.toml

```toml
[package]
name = "migration"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
sea-orm-migration = { version = "1.1" }
async-trait = { version = "0.1" }
fracture-core-migration = { path = "../../fracture-cms/fracture-core/migration" }
```

---

## Security Architecture

### Principle: Defense in Depth

This is a security company's app. Every layer must enforce access control independently.

### 1. IDOR Prevention (Critical)

Every database query that returns user-visible data MUST be scoped. There are three scoping patterns:

**Pattern A: Org-scoped resources (client data)**
```rust
// CORRECT: always filter by org_id from the authenticated org context
let item = Model::find_by_pid_and_org(&ctx.db, &pid, org_ctx.org.id)
    .await
    .ok_or_else(|| Error::NotFound)?;

// WRONG: never look up by pid alone
// let item = Model::find_by_pid(&ctx.db, &pid); // DO NOT DO THIS
```

**Pattern B: Pentester-scoped resources**
```rust
// CORRECT: verify pentester assignment before showing engagement data
let engagement = engagements::Model::find_by_pid(&ctx.db, &pid)
    .await
    .ok_or_else(|| Error::NotFound)?;
if !auth::is_assigned_pentester(&ctx.db, user.id, engagement.id).await {
    return Err(Error::Unauthorized);
}
```

**Pattern C: Admin-scoped resources**
```rust
// CORRECT: verify platform admin status
require_platform_admin!(&ctx.db, &user);
// Admin can then access cross-org data
```

### 2. Role Enforcement

Every controller handler MUST check authorization before accessing data:

| Route Prefix | Required Check |
|-------------|----------------|
| `/targets`, `/scans`, `/findings`, `/reports` (client) | `require_user!` + `require_role!(org_ctx, OrgRole::*)` + org-scoped query |
| `/engagements` (client) | `require_user!` + `require_role!(org_ctx, OrgRole::*)` + org-scoped query |
| `/admin/*` | `require_user!` + `require_platform_admin!` |
| `/pentester/*` | `require_user!` + `is_assigned_pentester()` per engagement |
| `/api/v1/*` | API key auth + org-scoped query |

### 3. Data Isolation Rules

| Actor | Can Access | Cannot Access |
|-------|-----------|---------------|
| Client (Viewer) | Own org's scans, findings, reports, engagement status | Other orgs, admin notes, pentester identity |
| Client (Member) | + request engagements, manage targets, respond to offers | Same restrictions |
| Client (Admin) | + subscriptions, invoices, invite members | Same restrictions |
| Pentester | Assigned engagement scope + findings they create | Client org data, other engagements, targets, scans, invoices |
| Platform Admin | All engagements, create offers, assign pentesters | Client passwords, API keys (these are `#[serde(skip_serializing)]` in fracture-core) |

### 4. Additional Security Measures

**Input validation:**
- All user input validated and sanitized before storage
- Markdown rendered with a strict allowlist (no raw HTML)
- IP ranges and domains validated for format
- Severity values validated against allowed set (extreme/high/elevated/moderate/low)

**Output security:**
- `admin_notes` field on engagements is NEVER included in client views or API responses
- Pentester identity is not exposed to clients (findings show "gethacked assessor")
- `#[serde(skip_serializing)]` on sensitive fields

**Rate limiting:**
- Scan launch endpoints rate-limited per subscription tier
- Engagement request limited to prevent spam

**Audit trail:**
- All tables have `created_at`/`updated_at` timestamps
- `created_by_user_id` on findings tracks who added each finding
- `assigned_by_user_id` on assignments tracks who assigned each pentester
- Offer history preserved (never deleted, superseded offers remain)

### 5. Cross-Tenant Query Safety

The `find_by_pid_and_org(pid, org_id)` pattern from fracture-cms is the primary defense against cross-tenant data access. This pattern MUST be used for ALL org-scoped resources. The `org_id` comes from the authenticated `org_ctx`, not from user input.

For pentester queries, the defense is the `pentester_assignments` join:
```rust
// Find all engagements assigned to this pentester
Entity::find()
    .inner_join(pentester_assignments::Entity)
    .filter(pentester_assignments::Column::UserId.eq(user_id))
    .all(db)
    .await
```

For admin queries that cross org boundaries, the defense is the `is_platform_admin` check, which verifies membership in the `gethacked-admin` org.

---

## Model Logic Patterns

Each model file in `src/models/<name>.rs` follows the fracture-cms pattern:

```rust
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Order;
use sea_orm::QueryOrder;

pub use super::_entities::<name>::{ActiveModel, Column, Entity, Model};
pub type <Name> = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert {
            this.pid = sea_orm::ActiveValue::Set(Uuid::new_v4());
        } else if this.updated_at.is_unchanged() {
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
        }
        Ok(this)
    }
}

impl Model {
    // Org-scoped entities get find_by_org + find_by_pid_and_org:
    pub async fn find_by_org(db: &DatabaseConnection, org_id: i32) -> Vec<Self> { ... }
    pub async fn find_by_pid_and_org(db: &DatabaseConnection, pid: &str, org_id: i32) -> Option<Self> { ... }
    // Global entities (services, pricing_tiers) get find_active, find_by_slug, etc.
}
```

### Key Query Methods Per Model

**services:**
- `find_active()` -- all active services ordered by sort_order
- `find_by_slug(slug)` -- lookup by URL slug

**pricing_tiers:**
- `find_by_service(service_id)` -- tiers for a service, ordered by sort_order
- `find_active_by_service(service_id)` -- active tiers only

**subscriptions:**
- `find_by_org(org_id)` -- org's subscriptions
- `find_active_by_org(org_id)` -- active subs only (check tier limits)

**scan_targets:**
- `find_by_org(org_id)` -- all targets for org
- `find_verified_by_org(org_id)` -- only verified targets
- `find_by_pid_and_org(pid, org_id)`

**scan_jobs:**
- `find_by_org(org_id)` -- all jobs
- `find_by_target(target_id, org_id)` -- jobs for a specific target (org-scoped)
- `find_by_pid_and_org(pid, org_id)`
- `count_this_month(org_id)` -- for quota enforcement

**engagements:**
- `find_by_org(org_id)` -- client's engagements
- `find_by_pid_and_org(pid, org_id)` -- client access
- `find_by_status(status)` -- admin: filter by workflow status (cross-org)
- `find_all_pending()` -- admin: all requested engagements (cross-org)
- `find_by_pentester(user_id)` -- pentester: my assignments (via join)

**engagement_offers:**
- `find_by_engagement(engagement_id)` -- all offers for an engagement
- `find_latest_by_engagement(engagement_id)` -- most recent offer

**pentester_assignments:**
- `find_by_engagement(engagement_id)` -- all pentesters assigned
- `find_by_user(user_id)` -- all engagements for a pentester
- `is_assigned(user_id, engagement_id)` -- check assignment

**findings:**
- `find_by_job(job_id, org_id)` -- findings for a specific scan (org-scoped)
- `find_by_engagement(engagement_id)` -- findings for a pentest engagement
- `find_by_org(org_id)` -- all findings for an org
- `find_by_severity(org_id, severity)` -- filter by severity
- `find_by_pid_and_org(pid, org_id)` -- client access
- `find_by_pid_and_engagement(pid, engagement_id)` -- pentester access

**reports:**
- `find_by_org(org_id)`
- `find_by_pid_and_org(pid, org_id)`
- `find_by_engagement(engagement_id)`

**invoices:**
- `find_by_org(org_id)`
- `find_by_pid_and_org(pid, org_id)`

---

## Controller Patterns

### Client Controller Pattern (org-scoped)

Every org-scoped controller follows this exact pattern from fracture-cms:

```rust
use super::middleware;
use crate::models::_entities::<entity>::{ActiveModel, Model};
use crate::models::org_members::OrgRole;
use crate::models::organizations as org_model;
use crate::{require_role, require_user, views};

pub async fn list(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    let org_ctx = middleware::get_org_context_or_default(&jar, &ctx.db, &user)
        .await
        .ok_or_else(|| Error::NotFound)?;
    require_role!(org_ctx, OrgRole::Viewer);
    let items = Model::find_by_org(&ctx.db, org_ctx.org.id).await;
    let user_orgs = org_model::Model::find_orgs_for_user(&ctx.db, user.id).await;
    views::<entity>::list(&v, &user, &org_ctx, &user_orgs, &items)
}
```

### Admin Controller Pattern

```rust
use crate::models::auth;

pub async fn list_engagements(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    require_platform_admin!(&ctx.db, &user);
    // Admin can see engagements across all orgs
    let items = engagements::Model::find_all_pending(&ctx.db).await;
    views::admin::engagement::list(&v, &user, &items)
}
```

### Pentester Controller Pattern

```rust
pub async fn list_engagements(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    // Only show engagements this pentester is assigned to
    let items = engagements::Model::find_by_pentester(&ctx.db, user.id).await;
    views::pentester::engagement::list(&v, &user, &items)
}

pub async fn add_finding(
    Path(engagement_pid): Path<String>,
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Form(params): Form<FindingParams>,
) -> Result<Response> {
    let user = middleware::get_current_user(&jar, &ctx).await;
    let user = require_user!(user);
    // Look up engagement and verify pentester assignment
    let engagement = engagements::Model::find_by_pid(&ctx.db, &engagement_pid)
        .await
        .ok_or_else(|| Error::NotFound)?;
    if !auth::is_assigned_pentester(&ctx.db, user.id, engagement.id).await {
        return Ok(/* 403 Forbidden */);
    }
    // Create finding scoped to this engagement
    let mut item: findings::ActiveModel = Default::default();
    params.update(&mut item);
    item.engagement_id = Set(Some(engagement.id));
    item.org_id = Set(engagement.org_id);
    item.created_by_user_id = Set(Some(user.id));
    item.status = Set("draft".to_string());
    item.insert(&ctx.db).await?;
    // ...
}
```

---

## RBAC Role Mapping

### Client Org Roles (fracture-core standard)

| Action | Minimum Role |
|--------|-------------|
| View services/pricing (public) | None |
| View dashboard, targets, scans, findings, reports | Viewer |
| Add targets, launch scans, request engagements, respond to offers | Member |
| Manage subscriptions, view invoices, invite members | Admin |
| Org settings, delete org | Owner |

### Platform Admin (gethacked-admin org membership)

| Action | Required |
|--------|----------|
| View all engagement requests | PlatformAdmin |
| Create offers, set pricing | PlatformAdmin |
| Assign/remove pentesters | PlatformAdmin |
| Update engagement status | PlatformAdmin |
| Generate and deliver reports | PlatformAdmin |
| Manage service catalog, pricing tiers | PlatformAdmin |

### Pentester (pentester_assignments table)

| Action | Required |
|--------|----------|
| View assigned engagement scope | AssignedPentester |
| Add findings to assigned engagement | AssignedPentester |
| Edit/delete own draft findings | AssignedPentester |
| View other pentesters' findings on same engagement | AssignedPentester |

---

## Integration with fracture-core Multi-Tenancy

- Every org-scoped table has an `org_id` FK to `organizations`
- Every client query is filtered by `org_ctx.org.id` -- no cross-org data leakage
- The `org_pid` cookie selects the active org, resolved by `get_org_context_or_default`
- Users can belong to multiple orgs (e.g., a consultant managing multiple clients)
- Personal orgs get a free Starter tier automatically
- Platform admin org (`gethacked-admin`) is a regular org with a well-known slug
- Pentester access is orthogonal to org membership -- pentesters do NOT become org members

---

## Background Workers

### ScanRunnerWorker

Processes queued scan jobs asynchronously:

1. Picks up jobs with status `queued`
2. Updates status to `running`
3. Executes the scan (nmap, nuclei, etc. via subprocess)
4. Parses results into `findings` rows (with `job_id` set, `engagement_id` null)
5. Updates job status to `completed` with `result_summary`
6. Optionally generates a report

This uses Loco's `BackgroundWorker` + `Queue` pattern (same as `DownloadWorker` in fracture-cms).

---

## Page Flow

### Guest Flow
1. Landing page (`/`) -- hero, EU value props, service overview
2. Service catalog (`/services`) -- browse services
3. Service detail (`/services/vulnerability-scanning`) -- description + pricing
4. Pricing page (`/pricing`) -- comparison table across tiers
5. CTA -> OIDC login -> auto-create personal org -> dashboard

### Client Flow
1. Dashboard (`/`) -- active subscription, recent scans, engagement status, finding summary
2. Add targets (`/targets/new`) -> verify ownership
3. Launch scan (`/scans` POST) -> view results (`/scans/:pid`)
4. Review findings (`/findings`) -> update status
5. Download reports (`/reports/:pid/download`)
6. Request pentest (`/engagements/new`) -> fill scope form
7. View offer (`/engagements/:pid`) -> accept or negotiate
8. Receive report when engagement delivered
9. Manage subscription (`/subscriptions`)
10. View invoices (`/invoices`)

### Admin Flow
1. Review incoming requests (`/admin/engagements`)
2. Review scope -> create offer (`/admin/engagements/:pid/offer`)
3. Wait for client acceptance
4. Assign pentester(s) (`/admin/engagements/:pid/assign`)
5. Monitor progress
6. Review findings before delivery (`/admin/engagements/:pid/report`)
7. Generate report and mark delivered

### Pentester Flow
1. View assigned engagements (`/pentester/engagements`)
2. Read engagement scope (`/pentester/engagements/:pid`)
3. Add findings as testing progresses (`/pentester/engagements/:pid/findings/new`)
4. Each finding includes: title, business description, technical description, impact, recommendation, severity, category, evidence, affected asset
5. Edit findings as needed
6. Mark findings as confirmed (draft -> confirmed)

---

## EU Compliance Considerations

- All data stored in EU-hosted infrastructure (configurable via deployment)
- GDPR data processing: user consent at signup, data export/deletion via org settings
- NIS2 awareness: compliance gap analysis as a service offering
- EUR-only pricing, Stripe configured for EU payment methods
- Cookie consent handled via security headers initializer / frontend banner
- Audit trail: all tables have `created_at`/`updated_at` timestamps, user attribution on findings

---

## Future Extensibility (Out of Scope, Designed For)

### Local Network Assessment Agent
The data model supports a future downloadable agent for localhost/internal network assessments:
- `scan_targets.target_type` can be extended with `"internal"` or `"agent"`
- `scan_jobs` can track agent-initiated scans via a new `source` field
- Agent would authenticate via the existing API key mechanism and POST findings to `/api/v1/scans/:pid/findings`
- The `scan_targets.verification_method` field can support `"agent_verified"` for internal targets
- No schema changes required -- only new `target_type` and `verification_method` values
