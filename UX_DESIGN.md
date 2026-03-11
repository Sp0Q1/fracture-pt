# GetHacked.eu -- UX Design & Visual Direction

## 1. Brand Identity

### Tagline
**"Offensive Security, European Standards."**

### Brand Personality
- Professional and trustworthy (corporate security buyer confidence)
- Technically competent (speaks the language of CISOs and DevSecOps)
- EU-proud (subtle EU stars motif, GDPR-first messaging, data sovereignty)
- Premium but accessible (not enterprise-only; SMEs welcome)

### Logo Concept
- Shield icon with 12-star EU circle motif integrated into the shield outline
- Wordmark: "get**hacked**" -- lowercase, monospace-inspired font, "hacked" in bold accent color
- Favicon: Shield with single star

---

## 2. Color Scheme

Derived from fracture-cms design system (dark theme, OAT CSS framework) but rebranded for security.

### Primary Palette

| Role               | Color     | Hex       | Usage                                      |
|--------------------|-----------|-----------|--------------------------------------------|
| Background         | Near-black| `#0b0d13` | Page background (matches fracture dark)    |
| Surface            | Dark gray | `#13151c` | Cards, panels, elevated surfaces           |
| Primary            | EU Blue   | `#003399` | EU brand color, primary buttons, links     |
| Primary Light      | Bright Blue| `#2563eb`| Hover states, active elements              |
| Accent             | Cyber Green| `#22c55e`| Success states, "secure" indicators, CTAs  |
| Accent Secondary   | Amber     | `#f59e0b` | Warnings, attention, vulnerability ratings |
| Danger             | Red       | `#ef4444` | Extreme findings, destructive actions      |
| Text Primary       | White     | `#f0f0f5` | Headlines, body text                       |
| Text Secondary     | Gray      | `#9ca3af` | Captions, labels, muted text               |
| Border             | Subtle    | `#1e2130` | Card borders, dividers                     |

### Gradient
- Hero gradient: `linear-gradient(135deg, #003399, #2563eb)` (EU blue spectrum)
- Accent gradient: `linear-gradient(135deg, #22c55e, #2563eb)` (green-to-blue, security feel)
- Card top border: 3px gradient accent (inherited from fracture-cms `.card-accent`)

### Dark Mode Only
The site ships dark-mode only. Security professionals expect it. It reinforces the "hacker" brand without being edgy.

---

## 3. Typography

| Element       | Font                          | Weight | Size           |
|---------------|-------------------------------|--------|----------------|
| Headings      | `Inter` (system-ui fallback)  | 700-800| 2.75rem hero, scaling down |
| Body          | `Inter` (system-ui fallback)  | 400    | 1rem (16px)    |
| Code/Terminal | `JetBrains Mono` / monospace  | 400    | 0.875rem       |
| Nav           | System sans-serif             | 500    | 0.9rem         |

---

## 4. Page Hierarchy & Navigation

### Sitemap

```
/                           Landing page (public)
/services                   Services overview (public)
/services/pentest           Manual pentesting detail
/services/scan              Automated scanning detail
/services/red-teaming       Red teaming & adversary simulation detail
/services/source-review     Source code review detail
/pricing                    Pricing tiers (public)
/about                      About us / Team (public)
/contact                    Contact / Request quote (public)
/blog                       Blog / Resources (public, future)
/legal/privacy              Privacy policy (public)
/legal/terms                Terms of service (public)
/legal/imprint              Impressum (required in EU)

--- Client Dashboard (role: client org Owner/Member) ---
/dashboard                  Client dashboard home
/dashboard/scans            Scan history & results
/dashboard/scans/:id        Individual scan report
/dashboard/pentests         Pentest requests & engagements
/dashboard/pentests/new     New pentest request wizard (scope definition)
/dashboard/pentests/:id     Engagement detail (scope, status, findings, report)
/dashboard/pentests/:id/offer  View & accept/negotiate offer
/dashboard/pentests/:id/findings  View findings for engagement
/dashboard/pentests/:id/report   Download compiled report
/dashboard/reports          All downloadable reports
/dashboard/billing          Subscription & invoices
/dashboard/settings         Account & org settings
/dashboard/api-keys         API key management

--- Pentester Dashboard (role: pentester, scoped to assigned engagements) ---
/pentester                  Pentester home (assigned engagements only)
/pentester/engagements      List of assigned engagements
/pentester/engagements/:id  Engagement workspace
/pentester/engagements/:id/findings        List findings for engagement
/pentester/engagements/:id/findings/new    Add new finding
/pentester/engagements/:id/findings/:fid   Edit finding
/pentester/engagements/:id/scope           View engagement scope (read-only)

--- Admin Dashboard (role: platform admin) ---
/admin                      Admin overview (all requests, engagements, users)
/admin/requests             Incoming pentest requests (pending review)
/admin/requests/:id         Review request & create offer
/admin/engagements          All engagements (active, completed, cancelled)
/admin/engagements/:id      Manage engagement (assign pentesters, status)
/admin/engagements/:id/offer   Create/edit offer for client
/admin/users                User management
/admin/pentesters           Pentester roster & assignments
/admin/reports              All generated reports
/admin/billing              Platform billing overview
```

### Top Navigation (Public)

```
[Shield Logo] GetHacked     Services v    Pricing    About    Contact    [Sign In]  [Get Started]
```

- "Services" has a dropdown: Pentesting, Scanning, Red Teaming
- "Get Started" is a primary CTA button (accent green)
- "Sign In" is a ghost/outline button
- Navigation is sticky with backdrop blur (inherited from fracture-cms `nav[data-topnav]`)

### Top Navigation (Client Dashboard)

```
[Shield Logo] GetHacked     [Org Switcher]     Dashboard    Scans    Pentests    Reports    [User Avatar v]
```

- Org switcher badge inherited from fracture-cms (`.org-badge`)
- User avatar dropdown: Settings, Billing, API Keys, Sign Out
- Client sees ONLY their own org's data; no cross-org navigation possible

### Top Navigation (Pentester Dashboard)

```
[Shield Logo] GetHacked     Engagements    [User Avatar v]
```

- No org switcher (pentesters see only assigned engagements, not client orgs)
- Minimal nav -- pentesters should not see client billing, settings, or other engagements
- User avatar dropdown: Profile, Sign Out

### Top Navigation (Admin Dashboard)

```
[Shield Logo] GetHacked     Requests    Engagements    Pentesters    Users    [User Avatar v]
```

- Full platform visibility
- User avatar dropdown: Settings, Sign Out

---

## 5. Landing Page Wireframe

### Section 1: Hero

```
+------------------------------------------------------------------------+
|  [Nav bar with backdrop blur]                                          |
+------------------------------------------------------------------------+
|                                                                        |
|  Radial gradient background (blue tones, subtle grid pattern overlay)  |
|                                                                        |
|           Know Your Weaknesses Before They Do.                         |
|     Professional penetration testing and automated security            |
|     scanning for European businesses. GDPR-compliant.                  |
|     EU-hosted. Results you can act on.                                 |
|                                                                        |
|         [Start Free Scan]  [Request Pentest Quote]                     |
|                                                                        |
|          EU Flag Stars        ISO 27001       GDPR Ready               |
|                                                                        |
+------------------------------------------------------------------------+
```

- **Headline**: Large, bold, white. The word "Weaknesses" could use the accent gradient.
- **Subtitle**: Secondary text color, max-width 540px, centered.
- **CTA buttons**: "Start Free Scan" = primary green. "Request Pentest Quote" = outline.
- **Trust signals row**: Small badges below CTAs (EU stars icon, ISO 27001, GDPR-compliant, "Data hosted in EU").
- **Background**: Radial gradients similar to fracture-cms `.hero::before` but in blue spectrum instead of green.

### Section 2: Social Proof Bar

```
+------------------------------------------------------------------------+
|  Trusted by 200+ European companies                                    |
|  [Logo 1]  [Logo 2]  [Logo 3]  [Logo 4]  [Logo 5]  [Logo 6]          |
+------------------------------------------------------------------------+
```

- Grayscale client logos on hover become colored
- Subtle separator line above and below

### Section 3: Services Overview (3-column grid)

```
+------------------------+------------------------+------------------------+
| [Shield Icon]          | [Radar Icon]           | [Target Icon]          |
| Penetration Testing    | Automated Scanning     | Red Teaming            |
|                        |                        |                        |
| Expert-led manual      | Continuous vulnerability| Full-scope adversary   |
| testing by certified   | scanning with instant  | simulation. Social     |
| security researchers.  | alerts and reports.    | engineering + exploit. |
|                        |                        |                        |
| [Learn More ->]        | [Learn More ->]        | [Learn More ->]        |
+------------------------+------------------------+------------------------+
```

- Uses fracture-cms `.card-accent` style (gradient top border)
- `.feature-icon` style for icon containers
- Each card links to `/services/<type>`
- Responsive: stack to single column on mobile

### Section 4: Why GetHacked? (Alternating feature rows)

```
+------------------------------------------------------------------------+
|  [Terminal mockup image]     |  EU-Hosted Infrastructure               |
|                              |  All scans run on EU-based servers.     |
|                              |  Your data never leaves the EU.         |
|                              |  Full GDPR compliance by design.       |
+------------------------------------------------------------------------+
|  Actionable Reports          |  [Report preview image]                 |
|  Not just a list of CVEs.    |                                         |
|  Prioritized findings with   |                                         |
|  business impact analysis.   |                                         |
+------------------------------------------------------------------------+
```

- Two alternating rows (image left/right)
- Clean, spacious layout
- Trust-building content

### Section 5: Pricing Preview

```
+------------------------------------------------------------------------+
|              Simple, Transparent Pricing                                |
|     No hidden fees. No per-scan surprises. Cancel anytime.             |
|                                                                        |
|  +------------+  +----------------+  +-------------+  +-------------+  |
|  | Starter    |  | Professional   |  | Business    |  | Enterprise  |  |
|  | EUR 99/yr  |  | EUR 249/yr     |  | EUR 499/yr  |  | Custom      |  |
|  |            |  | MOST POPULAR   |  |             |  |             |  |
|  | 10 scans/m |  | 50 scans/mo    |  | Unlimited   |  | Dedicated   |  |
|  | Basic rpts |  | + Scheduling   |  | + Priority  |  | + SLA       |  |
|  |            |  | + API access   |  | + Src review|  | + On-prem   |  |
|  |[Get Started]| |[Get Started]   |  |[Get Started]|  |[Contact Us] |  |
|  +------------+  +----------------+  +-------------+  +-------------+  |
|                                                                        |
|              [See Full Comparison ->]                                   |
+------------------------------------------------------------------------+
```

- Professional tier highlighted (accent border, "Most Popular" badge)
- EUR currency symbol, annual pricing with monthly equivalent shown smaller
- "See Full Comparison" links to /pricing
- Cards use `.card-accent` pattern

### Section 6: Testimonials / Case Studies

```
+------------------------------------------------------------------------+
|  "GetHacked identified 3 critical vulnerabilities our internal         |
|   team missed. Their EU-based approach means our pentest data          |
|   never leaves EU jurisdiction."                                       |
|                                                                        |
|   -- Jan Mueller, CISO, [Company], Germany                             |
+------------------------------------------------------------------------+
```

- Carousel or 2-3 static quotes
- Company name, role, country flag emoji
- Subtle quote marks in background (decorative)

### Section 7: CTA Banner

```
+------------------------------------------------------------------------+
|  Gradient background (blue to green)                                   |
|                                                                        |
|     Ready to Find Your Vulnerabilities?                                |
|     Start with a free scan or talk to our team.                        |
|                                                                        |
|     [Start Free Scan]     [Book a Demo]                                |
+------------------------------------------------------------------------+
```

### Section 8: Footer

```
+------------------------------------------------------------------------+
| GetHacked.eu                | Services       | Company    | Legal      |
| Offensive Security,         | Pentesting     | About      | Privacy    |
| European Standards.         | Scanning       | Blog       | Terms      |
|                             | Red Teaming    | Careers    | Imprint    |
| [EU Flag] Data hosted in EU |                | Contact    | Cookie Pol.|
|                             |                |            |            |
| [LinkedIn] [GitHub] [X]    |                |            |            |
+------------------------------------------------------------------------+
|  (c) 2026 GetHacked.eu -- Registered in [EU Country]                  |
|  VAT: EU123456789                                                      |
+------------------------------------------------------------------------+
```

- 4-column grid layout (matches fracture-cms `.footer-grid` pattern but expanded)
- EU flag icon and data residency statement
- Social links
- VAT number (required for EU B2B)
- Impressum link (required in DE/AT)

---

## 6. Pricing Page (`/pricing`)

### Layout

```
+------------------------------------------------------------------------+
|  Toggle: [Monthly] / [Annual - Save 20%]                              |
+------------------------------------------------------------------------+

+------------+  +----------------+  +-------------+  +------------------+
| STARTER    |  | PROFESSIONAL   |  | BUSINESS    |  | ENTERPRISE       |
|            |  | ** POPULAR **  |  |             |  |                  |
| EUR 99/yr  |  | EUR 249/yr     |  | EUR 499/yr  |  | Custom pricing   |
| ~EUR 9/mo  |  | ~EUR 21/mo     |  | ~EUR 42/mo  |  |                  |
+------------+  +----------------+  +-------------+  +------------------+
| Features:  |  | Features:      |  | Features:   |  | Everything in    |
| * 10 scans |  | * 50 scans/mo  |  | * Unlimited |  | Business, plus:  |
|   per month|  | * Scheduled    |  |   scans     |  |                  |
| * 3 targets|  |   scans        |  | * 25 targets|  | * Dedicated team |
| * Basic    |  | * 10 targets   |  | * Source    |  | * Custom SLA     |
|   reports  |  | * API access   |  |   code rev  |  | * On-prem option |
| * Email    |  | * PDF exports  |  | * Priority  |  | * SSO / SAML     |
|   alerts   |  | * Webhooks     |  |   support   |  | * Dedicated      |
|            |  | * Email +      |  | * Phone     |  |   account mgr    |
|            |  |   Slack alerts |  |   support   |  | * Custom         |
|            |  |                |  | * Retesting |  |   integrations   |
+------------+  +----------------+  +-------------+  +------------------+
|[Start Free]|  |[Start Free]    |  |[Start Free] |  | [Contact Sales]  |
+------------+  +----------------+  +-------------+  +------------------+

+------------------------------------------------------------------------+
| Full Feature Comparison Table                                          |
+------------------------------------------------------------------------+
| Feature              | Starter | Pro    | Business | Enterprise        |
|----------------------|---------|--------|----------|-------------------|
| Monthly scans        | 10      | 50     | Unlimited| Unlimited         |
| Targets              | 3       | 10     | 25       | Unlimited         |
| Scheduled scans      | --      | Yes    | Yes      | Yes               |
| API access           | --      | Yes    | Yes      | Yes               |
| PDF reports          | --      | Yes    | Yes      | Yes               |
| Source code review   | --      | --     | Yes      | Yes               |
| Dark web monitoring  | --      | --     | --       | Yes               |
| Priority support     | --      | --     | Yes      | Yes               |
| Phone support        | --      | --     | Yes      | Yes               |
| Retesting            | --      | --     | Yes      | Yes               |
| On-premises option   | --      | --     | --       | Yes               |
| Custom SLA           | --      | --     | --       | Yes               |
| SAML/SSO             | --      | --     | --       | Yes               |
| Dedicated team       | --      | --     | --       | Yes               |
+------------------------------------------------------------------------+
```

### Pricing Page Notes
- Monthly/annual toggle (annual is default, shows savings)
- All prices in EUR, inclusive of VAT note for B2C, "excl. VAT" for B2B toggle
- "Professional" tier has accent border + "Most Popular" badge
- Enterprise card has dark gradient background to stand out
- FAQ section below comparison table (common billing, cancellation, GDPR questions)

---

## 7. Service Detail Pages

### Structure (repeated for each service)

```
/services/pentest
/services/scan
/services/red-teaming
/services/source-review
```

Each page follows the same template:

```
+------------------------------------------------------------------------+
| [Breadcrumb: Services > Penetration Testing]                           |
+------------------------------------------------------------------------+
| Hero Section:                                                          |
|   H1: Professional Penetration Testing                                 |
|   Subtitle: Expert-led security assessments by certified EU hackers    |
|   [Request Quote]  [View Sample Report]                                |
+------------------------------------------------------------------------+
| What's Included (3-col feature cards)                                  |
|   - Web Application Testing                                            |
|   - Network Infrastructure Testing                                     |
|   - API Security Assessment                                            |
|   - Social Engineering (optional add-on)                               |
+------------------------------------------------------------------------+
| Our Process (numbered steps, horizontal)                               |
|   1. Scoping Call  ->  2. Testing  ->  3. Report  ->  4. Remediation   |
+------------------------------------------------------------------------+
| Certifications                                                         |
|   OSCP, OSCE, CEH, CREST badges                                       |
+------------------------------------------------------------------------+
| Sample Report Preview (gated download)                                 |
+------------------------------------------------------------------------+
| CTA: Ready to test your defenses? [Request Quote]                      |
+------------------------------------------------------------------------+
```

### Breadcrumb
Uses fracture-cms `.breadcrumb` component.

---

## 8. Dashboards (Role-Based Views)

### 8a. Client Dashboard (`/dashboard`)

#### Dashboard Home

```
+------------------------------------------------------------------------+
| [Sidebar]              | Dashboard Home                                |
|                        |                                               |
| Dashboard              | Welcome back, {name}                          |
| Scans                  | {org_name}                                    |
| Pentests               |                                               |
| Reports                | +----------+ +----------+ +-----------+       |
| ---                    | | 12       | | 3        | | 1         |       |
| Billing                | | Scans    | | Extreme  | | Active    |       |
| Settings               | | This Mo. | | Findings | | Pentest   |       |
| API Keys               | +----------+ +----------+ +-----------+       |
|                        |                                               |
|                        | Active Pentests                               |
|                        | +------------------------------------------+ |
|                        | | Engagement  | Status     | Findings | Due  | |
|                        | |-------------|------------|----------|------| |
|                        | | Web App Q1  | In Progress| 5        | Apr 1| |
|                        | | API Review  | Offer Sent | --       | --   | |
|                        | +------------------------------------------+ |
|                        |                                               |
|                        | Recent Scan Results                           |
|                        | +------------------------------------------+ |
|                        | | Target    | Status  | Findings | Date     | |
|                        | |-----------|---------|----------|----------| |
|                        | | app.co.de | Done    | 5 (2 Hi) | Mar 9   | |
|                        | | api.co.de | Running | --       | Mar 10  | |
|                        | +------------------------------------------+ |
|                        |                                               |
|                        | Quick Actions                                 |
|                        | [+ New Scan]  [+ Request Pentest]             |
+------------------------------------------------------------------------+
```

#### Design Notes
- Uses fracture-cms sidebar layout (`[data-sidebar-layout]`)
- Stat cards use `.stat-card` pattern with severity-based colors
- Tables use fracture-cms table styles with `.clickable-rows`
- Org switcher in top nav for multi-tenant support
- **Client sees ONLY their own org's pentests, scans, and findings**

#### Scan Results Page (`/dashboard/scans/:id`)

```
+------------------------------------------------------------------------+
| [Breadcrumb: Dashboard > Scans > app.example.com - Mar 10]            |
+------------------------------------------------------------------------+
| Scan Summary Card                                                      |
| Target: app.example.com    Status: Completed    Duration: 12m          |
| Score: 72/100              Grade: B                                    |
+------------------------------------------------------------------------+
| Findings by Severity                                                   |
| [====] Extreme: 2  [====] High: 3  [====] Elevated: 8  [====] Low: 12 |
+------------------------------------------------------------------------+
| Findings List (expandable details)                                     |
| +----------------------------------------------------------------+    |
| | EXTREME  | SQL Injection in /api/users                         |    |
| |          | Parameter: id  |  [View Details v]                    |    |
| +----------------------------------------------------------------+    |
| | HIGH     | Missing CSP Header                                  |    |
| |          | All pages affected  |  [View Details v]               |    |
| +----------------------------------------------------------------+    |
+------------------------------------------------------------------------+
| [Download PDF]  [Export JSON]  [Rescan]                                 |
+------------------------------------------------------------------------+
```

- Severity badges use color coding: Extreme=red, High=orange, Elevated=amber, Moderate=blue, Low=gray
- Expandable finding details use `<details>/<summary>` (OAT component)
- Grade badge (A-F) with color scale
- Progress bars for severity distribution

#### Client Pentest Engagement View (`/dashboard/pentests/:id`)

```
+------------------------------------------------------------------------+
| [Breadcrumb: Dashboard > Pentests > Web App Assessment Q1 2026]        |
+------------------------------------------------------------------------+
| Engagement Status Bar                                                  |
| [Requested]-->[Offer Sent]-->[Accepted]-->[In Progress]-->[Completed]  |
|     done         done          done         CURRENT                    |
+------------------------------------------------------------------------+
| Engagement Details Card                                                |
| +--------------------------------------------------------------+      |
| | Scope: Web Application, API                                  |      |
| | Targets: app.example.com, api.example.com                    |      |
| | Window: March 15 -- March 22, 2026                           |      |
| | Methodology: OWASP, PTES                                     |      |
| | Assigned Team: 2 pentesters                                  |      |
| | Reference: GH-2026-0042                                      |      |
| +--------------------------------------------------------------+      |
+------------------------------------------------------------------------+
| Findings (visible once testing begins)                                 |
| +----------------------------------------------------------------+    |
| | EXTREME  | SQL Injection in /api/users                         |    |
| |          | Description: An attacker can extract all user        |    |
| |          | records by manipulating the id parameter...           |    |
| |          | Impact: Full database compromise                     |    |
| |          | Recommendation: Use parameterized queries             |    |
| |          | Category: Injection                                  |    |
| |          | [View Technical Details v]                            |    |
| +----------------------------------------------------------------+    |
| | HIGH     | Broken Authentication on Admin Panel                 |    |
| |          | ...                                                   |    |
| +----------------------------------------------------------------+    |
+------------------------------------------------------------------------+
| [Download Report (PDF)]  [Download Findings (JSON)]                    |
+------------------------------------------------------------------------+
```

- Findings shown as expandable cards
- Client sees: Description, Impact, Recommendation, Severity, Category
- Client can optionally expand "Technical Details" (reproduction steps)
- Report download available once engagement status = Completed
- **NO editing capability** -- client is read-only on findings

#### Client Offer View (`/dashboard/pentests/:id/offer`)

```
+------------------------------------------------------------------------+
| Offer for: Web App Assessment Q1 2026                                  |
+------------------------------------------------------------------------+
| +--------------------------------------------------------------+      |
| | Scope: Web Application + API pentesting                      |      |
| | Duration: 5 business days                                    |      |
| | Team: 2 senior pentesters                                    |      |
| | Deliverables:                                                |      |
| |   - Executive summary                                        |      |
| |   - Detailed technical findings                              |      |
| |   - Remediation roadmap                                      |      |
| |   - 1 free retest within 30 days                             |      |
| | Timeline: March 15 -- March 22, 2026                         |      |
| +--------------------------------------------------------------+      |
| | Price: EUR 7,500 (excl. VAT)                                 |      |
| +--------------------------------------------------------------+      |
|                                                                        |
| [Accept Offer]     [Request Changes]     [Decline]                     |
|                                                                        |
| "Request Changes" opens a text field for negotiation notes             |
+------------------------------------------------------------------------+
```

---

### 8b. Pentester Dashboard (`/pentester`)

Pentesters have a completely separate, minimal dashboard. They see ONLY
engagements they are assigned to -- never client org data, billing, or
other engagements.

#### Pentester Home

```
+------------------------------------------------------------------------+
| My Engagements                                                         |
|                                                                        |
| +----------+ +----------+                                              |
| | 2        | | 7        |                                              |
| | Active   | | Findings |                                              |
| | Eng.     | | This Wk  |                                              |
| +----------+ +----------+                                              |
|                                                                        |
| Active Engagements                                                     |
| +------------------------------------------------------------------+  |
| | Client Org | Engagement      | Status      | Findings | Due      |  |
| |------------|-----------------|-------------|----------|----------|  |
| | Acme Corp  | Web App Q1      | In Progress | 5        | Mar 22   |  |
| | Beta GmbH  | API Assessment  | In Progress | 2        | Apr 5    |  |
| +------------------------------------------------------------------+  |
|                                                                        |
| Completed Engagements                                                  |
| +------------------------------------------------------------------+  |
| | Client Org | Engagement      | Findings | Completed              |  |
| |------------|-----------------|----------|------------------------|  |
| | Gamma SA   | Network Audit   | 12       | Feb 28, 2026           |  |
| +------------------------------------------------------------------+  |
+------------------------------------------------------------------------+
```

- No sidebar -- simple list layout (pentesters don't need complex nav)
- Shows only assigned engagements
- Client org name shown for context, but no link to client data

#### Pentester Engagement Workspace (`/pentester/engagements/:id`)

```
+------------------------------------------------------------------------+
| [Breadcrumb: Engagements > Acme Corp -- Web App Q1]                    |
+------------------------------------------------------------------------+
| Scope (Read-Only)                                                      |
| +--------------------------------------------------------------+      |
| | Targets: app.acme.com, api.acme.com                          |      |
| | Type: Web Application, API                                   |      |
| | Window: March 15 -- March 22, 2026                           |      |
| | Exclusions: /admin (out of scope)                             |      |
| | Emergency Contact: security@acme.com, +49 123 456 7890       |      |
| +--------------------------------------------------------------+      |
+------------------------------------------------------------------------+
| Findings                                         [+ Add Finding]       |
| +----------------------------------------------------------------+    |
| | EXTREME  | SQL Injection in /api/users          [Edit] [Delete]|    |
| |          | Category: Injection                                  |    |
| +----------------------------------------------------------------+    |
| | HIGH     | Broken Auth on Admin Panel           [Edit] [Delete]|    |
| |          | Category: Broken Authentication                      |    |
| +----------------------------------------------------------------+    |
| | ELEVATED | Missing Rate Limiting                [Edit] [Delete]|    |
| |          | Category: Misconfiguration                           |    |
| +----------------------------------------------------------------+    |
+------------------------------------------------------------------------+
| [Mark Engagement Complete]                                             |
+------------------------------------------------------------------------+
```

- Pentester can ADD, EDIT, DELETE findings for their assigned engagement
- Scope is READ-ONLY (defined by client, approved by admin)
- "Mark Engagement Complete" triggers report generation for client

#### Add/Edit Finding Form (`/pentester/engagements/:id/findings/new`)

```
+------------------------------------------------------------------------+
| Add Finding                                                            |
+------------------------------------------------------------------------+
| Title *                                                                |
| [________________________________]                                     |
|                                                                        |
| Category *                                                             |
| [Select: Injection | Broken Auth | XSS | IDOR | SSRF |               |
|  Misconfiguration | Cryptographic Failure | Security Logging |         |
|  Insecure Design | Vulnerable Components | Other ]                    |
|                                                                        |
| Severity *                                                             |
| [Select: Extreme | High | Elevated | Moderate | Low ]                 |
|                                                                        |
| Description * (business-level, shown to client)                        |
| [textarea -- markdown supported]                                       |
| Hint: Explain what the vulnerability is in terms a non-technical       |
| stakeholder can understand.                                            |
|                                                                        |
| Technical Description * (reproduction steps, shown to client on expand)|
| [textarea -- markdown supported, code blocks]                          |
| Hint: Step-by-step reproduction. Include URLs, parameters, payloads.   |
|                                                                        |
| Impact * (what could happen if exploited)                              |
| [textarea -- markdown supported]                                       |
| Hint: Describe the business impact: data breach, financial loss, etc.  |
|                                                                        |
| Recommendation * (how to fix)                                          |
| [textarea -- markdown supported, code blocks]                          |
| Hint: Specific remediation steps. Include code examples where useful.  |
|                                                                        |
| Evidence / Screenshots (optional)                                      |
| [file upload -- images, max 10MB each]                                 |
|                                                                        |
| Affected URL/Endpoint (optional)                                       |
| [________________________________]  [+ Add More]                       |
|                                                                        |
|                    [Cancel]  [Save Finding]                             |
+------------------------------------------------------------------------+
```

- All required fields marked with *
- Markdown rendering for descriptions (preview tab)
- Severity uses pentest scale: Extreme / High / Elevated / Moderate / Low
- Category dropdown aligned with OWASP Top 10 2021
- File upload for evidence screenshots

---

### 8c. Admin Dashboard (`/admin`)

Admin has full platform visibility. This is the operational control panel.

#### Admin Home

```
+------------------------------------------------------------------------+
| [Sidebar]              | Platform Overview                              |
|                        |                                               |
| Overview               | +----------+ +----------+ +-----------+       |
| Requests               | | 3        | | 5        | | 42        |       |
| Engagements            | | Pending  | | Active   | | Total     |       |
| Pentesters             | | Requests | | Eng.     | | Clients   |       |
| Users                  | +----------+ +----------+ +-----------+       |
| Reports                |                                               |
| Billing                | Pending Requests (needs action)               |
|                        | +------------------------------------------+ |
|                        | | Client   | Type      | Submitted | Action | |
|                        | |----------|-----------|-----------|--------| |
|                        | | Acme Corp| Web+API   | Mar 8     | [Review]| |
|                        | | Beta GmbH| Network   | Mar 9     | [Review]| |
|                        | | Gamma SA | Cloud     | Mar 10    | [Review]| |
|                        | +------------------------------------------+ |
|                        |                                               |
|                        | Active Engagements                            |
|                        | +------------------------------------------+ |
|                        | | Client   | Eng.     | Pentesters | Status | |
|                        | |----------|----------|------------|--------| |
|                        | | Delta Ltd| API Test | @jan, @eva | 60%    | |
|                        | +------------------------------------------+ |
+------------------------------------------------------------------------+
```

#### Admin Request Review (`/admin/requests/:id`)

```
+------------------------------------------------------------------------+
| [Breadcrumb: Requests > Acme Corp -- Web App Request]                  |
+------------------------------------------------------------------------+
| Client Scope Request                                                   |
| +--------------------------------------------------------------+      |
| | Client: Acme Corp (org: acme-corp-pid)                       |      |
| | Type: Web Application, API                                   |      |
| | Targets: app.acme.com, api.acme.com                          |      |
| | Window: March 15 -- March 22, 2026                           |      |
| | Exclusions: /admin section                                   |      |
| | Methodology: OWASP, PTES                                     |      |
| | Environment: Staging                                         |      |
| | Budget: EUR 5,000 -- 10,000                                  |      |
| | Contact: security@acme.com                                   |      |
| | Notes: "Focus on API authentication flows"                   |      |
| +--------------------------------------------------------------+      |
+------------------------------------------------------------------------+
| Create Offer                                                           |
| +--------------------------------------------------------------+      |
| | Price (EUR, excl. VAT) *    | Duration (business days) *     |      |
| | [  7500  ]                  | [  5  ]                        |      |
| |                                                              |      |
| | Timeline *                                                   |      |
| | [Start: Mar 15] -- [End: Mar 22]                             |      |
| |                                                              |      |
| | Team Size *                                                  |      |
| | [  2  ] pentesters                                           |      |
| |                                                              |      |
| | Deliverables *                                               |      |
| | [x] Executive summary                                        |      |
| | [x] Detailed technical findings                              |      |
| | [x] Remediation roadmap                                      |      |
| | [x] Free retest (within 30 days)                             |      |
| | [ ] Attack surface diagram                                   |      |
| |                                                              |      |
| | Internal Notes (not visible to client)                       |      |
| | [textarea]                                                   |      |
| +--------------------------------------------------------------+      |
|                                                                        |
|            [Decline Request]     [Send Offer to Client]                |
+------------------------------------------------------------------------+
```

#### Admin Engagement Management (`/admin/engagements/:id`)

```
+------------------------------------------------------------------------+
| [Breadcrumb: Engagements > Acme Corp -- Web App Q1]                    |
+------------------------------------------------------------------------+
| Status: [Requested] > [Offer Sent] > [Accepted] > [In Progress] > ... |
+------------------------------------------------------------------------+
| Engagement Details                                                     |
| Client: Acme Corp    Price: EUR 7,500    Window: Mar 15-22             |
+------------------------------------------------------------------------+
| Assign Pentesters                                                      |
| +--------------------------------------------------------------+      |
| | Currently Assigned:                                          |      |
| | - @jan.kowalski (OSCP, OSCE) [Remove]                       |      |
| | - @eva.schneider (CEH, CREST) [Remove]                      |      |
| |                                                              |      |
| | Add Pentester: [Select from roster v]  [Assign]              |      |
| +--------------------------------------------------------------+      |
+------------------------------------------------------------------------+
| Findings Summary                                                       |
| Total: 7 | Extreme: 1 | High: 2 | Elevated: 3 | Low: 1               |
+------------------------------------------------------------------------+
| [View All Findings]  [Generate Report]  [Mark Complete]                |
+------------------------------------------------------------------------+
```

- Admin can assign/remove pentesters
- Admin can view findings but typically does not edit them
- Admin can force-generate report or mark engagement complete
- Internal notes visible to admin only

---

## 9. Pentest Workflow (End-to-End, In-App)

This is the core business workflow. Every step happens within the app --
no email-only steps, no off-platform negotiations.

### Engagement Lifecycle States

```
[Requested] --> [Offer Sent] --> [Accepted] --> [In Progress] --> [Completed]
                     |                |
                     v                v
              [Offer Declined]  [Cancelled]
                     |
                     v
              [Negotiating] --> [Offer Revised] --> [Accepted]
```

### Phase 1: Scope Definition (Client)

Client navigates to `/dashboard/pentests/new` (or from public `/contact` page
which redirects to sign-in first, then to the wizard).

#### Step 1 of 4: Test Type & Targets

```
+------------------------------------------------------------------------+
| Request a Penetration Test                                             |
|                                                                        |
| Step 1 of 4: Scope                    [1]---[2]---[3]---[4]           |
|                                                                        |
| What would you like tested? *                                          |
| [ ] Web Application                                                    |
| [ ] API / Microservices                                                |
| [ ] Network Infrastructure                                             |
| [ ] Mobile Application                                                 |
| [ ] Cloud Environment (AWS/Azure/GCP)                                  |
|                                                                        |
| Target URL(s), domains, or IP ranges: *                                |
| [________________________________]  [+ Add More]                       |
|                                                                        |
| Exclusions (systems/paths NOT to test):                                |
| [________________________________]  [+ Add More]                       |
|                                                                        |
| Preferred testing window: *                                            |
| [Date picker: Start] -- [Date picker: End]                             |
|                                                                        |
|                              [Next: Details ->]                        |
+------------------------------------------------------------------------+
```

#### Step 2 of 4: Environment & Requirements

```
+------------------------------------------------------------------------+
| Step 2 of 4: Details                   [1]--[2]---[3]---[4]           |
|                                                                        |
| Environment type: *                                                    |
| ( ) Production  ( ) Staging  ( ) Development                           |
|                                                                        |
| Testing methodology:                                                   |
| [ ] OWASP  [ ] PTES  [ ] OSSTMM  [ ] Custom: [____]                  |
|                                                                        |
| Do you have existing security documentation?                           |
| ( ) Yes, I'll share it  ( ) No                                        |
|                                                                        |
| Areas of particular concern:                                           |
| [textarea]                                                             |
|                                                                        |
| Budget range (optional):                                               |
| [Select: EUR 2,000-5,000 | 5,000-10,000 | 10,000-25,000 | 25,000+]   |
|                                                                        |
|                    [<- Back]  [Next: Contact ->]                       |
+------------------------------------------------------------------------+
```

#### Step 3 of 4: Emergency Contact

```
+------------------------------------------------------------------------+
| Step 3 of 4: Contact Info              [1]--[2]--[3]---[4]            |
|                                                                        |
| Emergency contact during testing: *                                    |
| Name:  [________________________________]                              |
| Email: [________________________________]                              |
| Phone: [________________________________]                              |
|                                                                        |
| How should we communicate findings during testing?                     |
| ( ) In-app only (findings appear in dashboard)                         |
| ( ) In-app + email notification for Extreme/High                       |
| ( ) In-app + phone call for Extreme                                    |
|                                                                        |
|                    [<- Back]  [Next: Review ->]                        |
+------------------------------------------------------------------------+
```

#### Step 4 of 4: Review & Submit

```
+------------------------------------------------------------------------+
| Step 4 of 4: Review                    [1]--[2]--[3]--[4]             |
|                                                                        |
| Summary of your request:                                               |
| +--------------------------------------------------------------+      |
| | Type: Web Application, API                                   |      |
| | Targets: app.example.com, api.example.com                    |      |
| | Exclusions: /admin, 10.0.0.0/8                               |      |
| | Window: March 15 -- March 22, 2026                           |      |
| | Environment: Staging                                         |      |
| | Methodology: OWASP, PTES                                     |      |
| | Budget: EUR 5,000 - 10,000                                   |      |
| | Contact: Jan Mueller, jan@acme.com, +49 123 456 7890         |      |
| | Notifications: In-app + email for Extreme/High               |      |
| +--------------------------------------------------------------+      |
|                                                                        |
| [x] I confirm these targets are owned by my organization *             |
| [x] I agree to the Terms of Engagement *                               |
|                                                                        |
|                    [<- Back]  [Submit Request]                          |
+------------------------------------------------------------------------+
```

### Phase 2: Offer / Quote (Admin)

After submission:
1. Request appears in admin dashboard at `/admin/requests` with status "Pending"
2. Admin reviews scope details at `/admin/requests/:id`
3. Admin creates offer with pricing, timeline, deliverables, team size
4. Admin clicks "Send Offer to Client"
5. Status changes to "Offer Sent"
6. Client receives in-app notification + email

(See Section 8c for admin offer creation wireframe.)

### Phase 3: Acceptance / Negotiation (Client)

Client views offer at `/dashboard/pentests/:id/offer`:
- **Accept** -- status moves to "Accepted", admin is notified, engagement is created
- **Request Changes** -- client writes negotiation notes, status moves to "Negotiating"
  - Admin can revise offer and resend
  - Back-and-forth happens in-app via a simple comment thread on the offer
- **Decline** -- status moves to "Declined", engagement is not created

(See Section 8a for client offer view wireframe.)

### Phase 4: Pentest Execution (Pentesters)

After client accepts:
1. Admin assigns pentesters at `/admin/engagements/:id`
2. Assigned pentesters see the engagement in their dashboard at `/pentester/engagements/:id`
3. Pentesters view scope (read-only) and add findings as they test
4. Each finding has structured fields (see Section 8b for finding form):
   - **Title** -- Short name
   - **Category** -- OWASP Top 10 aligned (Injection, Broken Auth, XSS, IDOR, etc.)
   - **Severity** -- Extreme / High / Elevated / Moderate / Low
   - **Description** -- Business-level explanation
   - **Technical Description** -- Reproduction steps, payloads, technical details
   - **Impact** -- What could happen if exploited
   - **Recommendation** -- Specific remediation steps
   - **Evidence** -- Screenshots, request/response captures
   - **Affected Endpoints** -- URLs/IPs affected
5. Client sees findings appear in real-time in their dashboard
6. Client gets notifications for Extreme/High findings (per their preference)

### Phase 5: Report Delivery

When pentester marks engagement complete (or admin does):
1. System compiles all findings into a structured report
2. Report is available at `/dashboard/pentests/:id/report`
3. Client can download as PDF or JSON
4. Report includes:
   - Executive summary (auto-generated from findings)
   - Severity distribution chart
   - Each finding with full details
   - Remediation roadmap (prioritized by severity)
   - Attack chain diagrams (if applicable)
5. Engagement status moves to "Completed"

### Engagement Status Visibility by Role

| Status        | Client Sees          | Pentester Sees       | Admin Sees           |
|---------------|----------------------|----------------------|----------------------|
| Requested     | "Under Review"       | --                   | Full request         |
| Offer Sent    | Offer details        | --                   | Offer + request      |
| Negotiating   | Comment thread       | --                   | Comment thread       |
| Accepted      | "Scheduling..."      | --                   | Ready to assign      |
| In Progress   | Scope + findings     | Scope + findings     | Everything           |
| Completed     | Findings + report    | Findings (read-only) | Everything           |
| Declined      | "Declined"           | --                   | Declined + reason    |
| Cancelled     | "Cancelled"          | --                   | Cancelled + reason   |

---

## 10. Role Model & Security UX Constraints

### Role Hierarchy

GetHacked extends fracture-core's RBAC (Owner > Admin > Member > Viewer)
with platform-level role concepts:

| Platform Role | Fracture-Core Mapping        | Access Scope                          |
|---------------|------------------------------|---------------------------------------|
| **Client**    | Org Owner or Org Member      | Own org's pentests, scans, reports, billing |
| **Pentester** | Special role (not org-based)  | ONLY assigned engagements, nothing else |
| **Admin**     | Platform-level superuser     | All requests, engagements, users, billing |

### Least Privilege Rules (Enforced in UX and Backend)

#### Client (Org Owner/Member)
- CAN: Create pentest requests, view/accept/negotiate offers, view findings, download reports, manage scans, manage billing, manage org settings
- CANNOT: View other orgs' data, edit findings, assign pentesters, access admin panel, view other clients
- Org Viewer: Can view findings and reports but cannot create requests or manage billing
- Org scoping: Every query includes `WHERE org_id = :current_org_id`

#### Pentester
- CAN: View scope of assigned engagements (read-only), add/edit/delete findings for assigned engagements, mark engagement as testing-complete
- CANNOT: View client org details (billing, settings, other pentests), view unassigned engagements, modify scope, access admin panel, view any scan data
- Assignment scoping: Every query includes `WHERE engagement_id IN (SELECT engagement_id FROM pentester_assignments WHERE pentester_id = :current_user_id)`

#### Admin
- CAN: View all requests, create/edit offers, assign pentesters, manage engagements, view all findings, manage users, view billing
- CANNOT: N/A (full access), but destructive actions (delete engagement, delete findings) require confirmation dialogs
- Admin actions are audit-logged

### Security UX Patterns

1. **No direct object references in URLs that aren't verified server-side.**
   All `:id` parameters in routes are public IDs (UUIDs/pids), never sequential integers.
   Backend always verifies the authenticated user has access to the requested resource.

2. **No hidden UI as security.**
   Hiding a button is not access control. Even if the "Delete" button is hidden for
   a viewer, the DELETE endpoint must reject the request independently.

3. **Role-appropriate navigation.**
   Each role sees a completely different navigation structure (see Section 4).
   There are no "grayed out" admin links visible to clients.

4. **Confirmation for destructive actions.**
   Delete finding, cancel engagement, decline offer all require explicit confirmation
   via `confirm()` dialog or OAT `<dialog>` modal.

5. **Audit trail visibility.**
   Clients see a timeline of status changes on their engagement (who changed what, when).
   They do NOT see internal admin notes or pentester assignment details beyond "2 pentesters assigned."

6. **Session security.**
   Inherited from fracture-cms: OIDC tokens, 12-minute refresh cycle, session expiry
   page on failure. No local password storage.

---

## 11. EU Branding Elements

### EU Stars Motif
- Subtle 12-star circle used as:
  - Background watermark on hero section (10% opacity)
  - Part of the shield logo
  - Decorative element on pricing cards
  - Footer badge: "EU-hosted infrastructure"

### Trust Badges (placed strategically)
- "Data hosted in EU" with EU flag icon
- "GDPR Compliant" (about our own operations)
- "ISO 27001 Certified" (when achieved)
- "OSCP Certified Team"
- "100% EU-Hosted"

### Language / Localization
- Primary language: English (international business language)
- Future: German (de-DE), French (fr-FR), Dutch (nl-NL)
- i18n system inherited from fracture-cms (Fluent `.ftl` files)
- Currency always EUR with locale-appropriate formatting

### Legal Pages (EU-required)
- `/legal/imprint` -- Impressum (mandatory in DE, AT, recommended EU-wide)
- `/legal/privacy` -- GDPR-compliant privacy policy
- `/legal/terms` -- Terms of service
- Cookie consent banner (GDPR/ePrivacy Directive)
  - Only essential cookies by default
  - Analytics requires opt-in
  - Uses simple banner, not dark patterns

---

## 12. Component Library (Building on OAT + Fracture-CMS)

### Existing Components (inherited from fracture-cms)
- `nav[data-topnav]` -- Sticky nav with blur
- `.card` / `.card-accent` -- Content cards with gradient borders
- `.stat-card` -- Dashboard stat counters
- `.feature-icon` -- Icon containers
- `.badge` -- Status badges (success, warning, danger, outline)
- `.breadcrumb` -- Navigation breadcrumbs
- `.form-card` / `[data-field]` -- Form containers
- `.empty-state` -- Empty state illustrations
- `[data-sidebar-layout]` -- Dashboard sidebar layout
- `ot-dropdown` -- Popover dropdowns
- `.org-badge` -- Org switcher (reuse for workspace/company)
- `details/summary` -- Accordion (OAT built-in)
- `dialog` -- Modals (OAT built-in)
- `.toast` -- Notifications (OAT built-in)

### New Components Needed
- **Pricing Card** -- Extends `.card-accent` with tier highlight, price display, feature list
- **Severity Badge** -- Color-coded vulnerability severity (Extreme/High/Elevated/Moderate/Low)
- **Progress Ring** -- Circular security score visualization
- **Step Indicator** -- Multi-step form progress (4-step wizard for pentest request)
- **Scan Status** -- Animated dot + label (Queued/Running/Complete/Failed)
- **Finding Card** -- Expandable vulnerability finding with severity, category, full structured fields
- **Grade Badge** -- Letter grade (A-F) with color scale
- **Cookie Consent Banner** -- GDPR-compliant, bottom-fixed
- **Testimonial Card** -- Quote with attribution
- **Logo Cloud** -- Client logo display row
- **Feature Row** -- Alternating image/text layout for landing page
- **Engagement Status Pipeline** -- Horizontal progress bar showing lifecycle states
- **Offer Card** -- Structured offer display with Accept/Negotiate/Decline actions
- **Negotiation Thread** -- Simple comment thread for offer negotiation
- **Role-Based Nav** -- Three separate nav configurations (client, pentester, admin)
- **Audit Timeline** -- Chronological list of engagement status changes
- **Finding Form** -- Structured multi-field form with markdown preview for pentesters
- **Evidence Upload** -- File upload component with image preview (for finding screenshots)
- **Provisioning Interstitial** -- Progress page shown during org setup (instant in MVP, async in future isolation model)

---

## 13. Responsive Breakpoints

| Breakpoint | Width    | Layout Changes                                    |
|------------|----------|---------------------------------------------------|
| Desktop    | >1024px  | Full layout, sidebar visible, 4-col pricing       |
| Tablet     | 768-1024 | 2-col pricing, sidebar collapsed, stacked features |
| Mobile     | <768px   | Single column, hamburger menu, stacked everything  |

Inherited from OAT CSS: `@media (max-width: 768px)` with `--grid-cols: 4` (effectively single column).

---

## 14. Key User Journeys

### Journey A: New Visitor -> Paid Subscriber (Automated Scanning)
1. Lands on `/` (organic search or ad)
2. Reads hero, scrolls to services
3. Clicks "Start Free Scan" -> redirected to sign up (OIDC)
4. After signup, sees provisioning interstitial (instant in MVP, see Section 21)
5. Lands on dashboard, runs first free scan
6. Sees results, wants more -> clicks upgrade
7. Selects Professional tier -> payment (Stripe)
8. Now a paying subscriber with dashboard access

### Journey B: Enterprise Prospect -> Pentest Client (Full Lifecycle)
1. Lands on `/services/pentest` (from Google or referral)
2. Reads methodology, certifications
3. Clicks "Request Pentest" -> sign in / sign up (provisioning interstitial on first login)
4. Fills 4-step scope wizard (targets, details, contact, review)
5. Submits request; sees confirmation with reference number
6. Admin reviews request, creates offer (pricing, timeline, deliverables)
7. Client receives notification, views offer in dashboard
8. Client accepts offer (or negotiates via in-app thread)
9. Admin assigns pentesters to the engagement
10. Pentesters begin testing, add findings in real-time
11. Client sees findings appearing in dashboard, gets notifications for Extreme/High
12. Pentester marks testing complete
13. Client downloads compiled report (PDF) from dashboard

### Journey C: Pentester Workflow
1. Pentester logs in, sees assigned engagements
2. Opens engagement workspace, reviews scope (read-only)
3. Begins testing, adds findings one by one via structured form
4. For each finding: title, category, severity, description, technical details, impact, recommendation, evidence
5. Marks engagement complete when testing finished
6. Moves on to next assigned engagement

### Journey D: Admin Operations
1. Admin logs in, sees pending requests on overview
2. Reviews incoming pentest request (scope, targets, budget)
3. Creates offer with pricing and timeline
4. Sends offer to client
5. After client accepts, assigns pentesters from roster
6. Monitors engagement progress (finding count, severity distribution)
7. Reviews final findings, generates report
8. Marks engagement complete

### Journey E: Existing Client -> Download Report
1. Logs into `/dashboard`
2. Views completed pentest engagement
3. Navigates to Reports
4. Downloads detailed pentest report (PDF)
5. Shares with stakeholders

---

## 15. Accessibility Standards

- WCAG 2.1 AA compliance minimum
- Skip-to-content link (inherited from fracture-cms)
- Keyboard navigation for all interactive elements
- ARIA labels on all icon-only buttons
- Color contrast ratios: 4.5:1 minimum for text
- Focus visible outlines (`:focus-visible` in OAT)
- Screen reader friendly: semantic HTML, proper heading hierarchy
- Reduced motion: respect `prefers-reduced-motion`

---

## 16. Performance Targets

- First Contentful Paint: <1.5s
- Largest Contentful Paint: <2.5s
- Total page weight: <500KB (excluding images)
- No JavaScript frameworks required for public pages (server-rendered Tera templates)
- JS only for interactivity (dashboard charts, scan progress, form wizards)
- Critical CSS inlined for above-the-fold content

---

## 17. Integration Points with Fracture-CMS

The gethacked app extends fracture-cms as a library dependency:

| Fracture Feature       | GetHacked Usage                              |
|------------------------|----------------------------------------------|
| OIDC Authentication    | Client sign-in/sign-up                       |
| Organizations + RBAC   | Client companies, team member access         |
| Org-scoped data        | Scans, engagements, reports per-org          |
| Base template system   | Extended `base.html` with GetHacked branding |
| OAT CSS framework      | All styling built on OAT + custom theme      |
| i18n (Fluent)          | EN-US primary, DE-DE and FR-FR planned       |
| Session management     | Token refresh, authenticated routes          |

### Template Inheritance

```
fracture-cms/base.html            (OAT + nav + footer)
    |
    +-- gethacked/base.html        (rebranded nav, GetHacked footer, EU badges)
         |
         +-- gethacked/public/     (landing, services, pricing -- no auth required)
         |
         +-- gethacked/dashboard/  (CLIENT area: sidebar with Scans, Pentests, Reports, etc.)
         |    +-- dashboard/pentests/new.html    (4-step scope wizard)
         |    +-- dashboard/pentests/show.html   (engagement view + findings)
         |    +-- dashboard/pentests/offer.html  (accept/negotiate/decline)
         |    +-- dashboard/scans/...
         |
         +-- gethacked/pentester/  (PENTESTER area: minimal nav, engagement workspace)
         |    +-- pentester/engagements/show.html  (scope + findings list)
         |    +-- pentester/findings/form.html     (add/edit finding)
         |
         +-- gethacked/admin/     (ADMIN area: sidebar with Requests, Engagements, Users, etc.)
              +-- admin/requests/show.html    (review + create offer)
              +-- admin/engagements/show.html (assign pentesters, manage)
```

Each dashboard type uses a different `base_dashboard.html` / `base_pentester.html` /
`base_admin.html` that extends `base.html` but with role-appropriate navigation.
The backend verifies role on every route -- template-level navigation hiding is
cosmetic only, never a security boundary.

---

## 18. CSS Variable Overrides

GetHacked overrides fracture-cms CSS variables for rebranding:

```css
:root[data-theme="dark"] {
    --primary: #2563eb;          /* EU blue instead of green */
    --primary-foreground: #fff;
    --ring: #2563eb;
}

.gradient-text {
    background: linear-gradient(135deg, #2563eb, #22c55e);
    /* Blue-to-green instead of green-to-indigo */
}

.card-accent {
    border-image: linear-gradient(90deg, #2563eb, #22c55e) 1;
}

.hero::before {
    background:
        radial-gradient(ellipse at 20% 50%, rgba(37, 99, 235, 0.15) 0%, transparent 50%),
        radial-gradient(ellipse at 80% 50%, rgba(34, 197, 94, 0.12) 0%, transparent 50%);
}
```

---

## 19. Implementation Priority

### Phase 1: MVP Launch (Core Pentest Workflow + Public Pages)
1. Landing page with hero, services, pricing preview, footer
2. Pricing page with tier comparison
3. Sign-up / Sign-in flow (OIDC via fracture-cms) with provisioning interstitial (no-op in MVP, hook point for future isolation -- see Section 21)
4. **Pentest request wizard (4-step scope definition)**
5. **Admin request review & offer creation**
6. **Client offer acceptance/negotiation**
7. **Pentester dashboard with engagement workspace & finding form**
8. **Client engagement view with findings display**
9. **Report compilation and PDF download**
10. Role-based navigation (client, pentester, admin)
11. Cookie consent banner
12. Legal pages (privacy, terms, imprint)

### Phase 2: Automated Scanning + Growth
1. Basic dashboard with automated scan list
2. Scan execution and results display
3. Detailed technical report generation
4. API key management
5. Webhook/Slack notifications
6. Admin user management and pentester roster

### Phase 3: Scale
1. Blog / resource center
2. Multi-language support (DE, FR, NL)
3. Enterprise features (SSO/SAML, on-prem)
4. Advanced dashboard analytics and trend charts
5. Client logo social proof section
6. Testimonial carousel
7. Downloadable agent for localhost/local network assessments (future -- see Section 20)

---

## 20. Future Extensibility: Local Assessment Agent

Out of scope for current implementation, but the data model and UX should
be designed to accommodate this future feature.

### Concept
A downloadable agent (CLI tool or lightweight daemon) that clients install
on their local machine or network to enable:
- Internal network scanning (not reachable from the internet)
- Localhost application testing
- On-premise infrastructure assessment

### UX Implications (Design For Now, Build Later)
- Dashboard should have a section for "Agents" (hidden/disabled until feature ships)
- Engagement scope form should eventually include "Internal / Agent-Based" as a test type
- Findings from agent-based scans should flow into the same finding model
- Agent communicates back to the platform via authenticated API (uses API key from dashboard)
- Data model note: the `finding` and `engagement` models should not assume
  internet-reachable targets. Target fields should accept RFC 1918 addresses and
  "localhost" entries without validation errors.

### Data Model Considerations
- `target` field: VARCHAR, no URL validation (can be IP, hostname, CIDR range, "localhost")
- `scan_source` enum: `platform` | `agent` (extensible)
- Agent table (future): `id`, `org_id`, `name`, `api_key_id`, `last_seen`, `version`, `status`

---

## 21. Future Extensibility: Per-Client Isolated Environments

Out of scope for MVP, but the UX and architecture must not prevent migration
to full per-client isolation. The long-term goal is that each client org gets
its own isolated deployment (separate DB, container, or Kubernetes namespace)
provisioned automatically on account creation.

### Current Model (MVP)
Shared multi-tenant: all clients in one database, all data scoped by `org_id`.
Fracture-core handles org-scoped queries. This is the starting point.

### Future Model
When a new client creates an account:
1. OIDC signup completes (fracture-core)
2. Org is created (fracture-core)
3. **Provisioning is triggered automatically** -- no manual setup
4. Client sees a brief "Setting up your environment..." interstitial
5. Once provisioned, client lands on their dashboard

### UX Implications (Design For Now, Build Later)

#### Onboarding Flow with Provisioning Step

```
+------------------------------------------------------------------------+
| Welcome to GetHacked!                                                  |
|                                                                        |
|   [Spinner]  Setting up your secure environment...                     |
|                                                                        |
|   This takes a few seconds. We're creating your isolated workspace     |
|   with dedicated infrastructure.                                       |
|                                                                        |
|   [=============================>          ] 70%                        |
|                                                                        |
|   * Creating your workspace          [done]                            |
|   * Provisioning secure environment  [in progress]                     |
|   * Configuring access controls      [pending]                         |
+------------------------------------------------------------------------+
```

- This page is a simple polling/SSE-driven interstitial
- In the shared-tenant MVP, this step completes instantly (org creation is synchronous)
- In the isolated model, this step waits for the provisioning backend (container/namespace spin-up)
- The UX is the same either way -- the interstitial is shown briefly even in MVP
  to train the user expectation and avoid a jarring UX change later
- If provisioning fails, show a clear error with "Contact support" CTA

#### Dashboard Status Indicator

```
+------------------------------------------------------------------------+
| Environment: [green dot] Active    Region: EU-West    Isolation: Shared |
+------------------------------------------------------------------------+
```

- In settings page, show the client's environment status
- MVP: always "Shared" (or omit this entirely)
- Future: shows "Dedicated" with region, uptime, resource usage
- This is informational only -- clients cannot change isolation level (admin does)

#### Admin Provisioning View (Future)

```
+------------------------------------------------------------------------+
| Admin > Environments                                                    |
+------------------------------------------------------------------------+
| | Client Org   | Type      | Status  | Region   | Created   |          |
| |--------------|-----------|---------|----------|-----------|          |
| | Acme Corp    | Dedicated | Active  | EU-West  | Mar 10    |          |
| | Beta GmbH    | Dedicated | Active  | EU-West  | Mar 8     |          |
| | Gamma SA     | Shared    | Active  | EU-West  | Feb 15    |          |
+------------------------------------------------------------------------+
```

- Admin can see all client environments, their type, status
- Future: admin can migrate a client from shared to dedicated

### Design Constraints (Enforce Now)

These constraints ensure we can migrate to per-client isolation without
rearchitecting the UX:

1. **No cross-org references in the UI.** A client must never see data, IDs,
   or references belonging to another org. This is already enforced by Section 10
   (Role Model), but it also means: no global leaderboards, no "other customers
   also scanned..." features, no shared vulnerability databases across orgs.

2. **Org-scoped everything.** Every URL, every API endpoint, every template
   variable must be scoped to the current org. When we move to isolated
   deployments, the org context becomes implicit (you are on your instance),
   but the code should already work that way.

3. **Stateless session handling.** OIDC tokens must not rely on server-side
   session storage tied to a specific instance. Token validation must work
   against the IdP directly (fracture-core already does this).

4. **No shared filesystem state.** Uploaded evidence, generated reports, and
   other files must be stored in a location that can be per-org (e.g., object
   storage with org-prefixed keys like `s3://reports/{org_id}/...`), not in
   a shared filesystem path.

5. **Configuration as data.** Per-org settings (notification preferences, API
   keys, branding customization if ever offered) must be stored in the database,
   not in config files, so they travel with the org's data during migration.

6. **Onboarding hook point.** The signup flow must have a clear hook point
   where provisioning can be injected. In MVP this is a no-op. In future it
   calls a provisioning service. The UX (interstitial page) is the same.

### Migration Path (UX Perspective)

| Phase          | Isolation      | Onboarding UX                              |
|----------------|----------------|--------------------------------------------|
| MVP            | Shared DB      | Instant org creation, brief interstitial    |
| Phase 2        | DB-per-tenant  | Interstitial waits for DB provisioning (~5s)|
| Phase 3        | Full isolation  | Interstitial waits for container/ns (~30s)  |

The interstitial page and polling mechanism are the same across all phases.
Only the backend provisioning time changes. The client experience is seamless.
