# Security Architecture -- gethacked.eu

**Date:** 2026-03-10 (updated)
**Author:** Security Tester
**Status:** Draft v2
**Scope:** Complete security architecture for the gethacked.eu pentesting portal built on fracture-cms/fracture-core

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Threat Model](#2-threat-model)
3. [Pentest Workflow Security](#3-pentest-workflow-security)
4. [Role Model and Least Privilege](#4-role-model-and-least-privilege)
5. [Authentication and Session Management](#5-authentication-and-session-management)
6. [Multi-Tenant Data Isolation and IDOR Prevention](#6-multi-tenant-data-isolation-and-idor-prevention)
7. [Engagement-Scoped Access (Pentester Isolation)](#7-engagement-scoped-access-pentester-isolation)
8. [Scope Definition and Target Authorization](#8-scope-definition-and-target-authorization)
9. [Finding and Report Security](#9-finding-and-report-security)
10. [Encryption at Rest](#10-encryption-at-rest)
11. [API Key Management](#11-api-key-management)
12. [Rate Limiting Strategy](#12-rate-limiting-strategy)
13. [Input Validation](#13-input-validation)
14. [Audit Logging](#14-audit-logging)
15. [GDPR Compliance](#15-gdpr-compliance)
16. [NIS2 Compliance](#16-nis2-compliance)
17. [Secure File Handling](#17-secure-file-handling)
18. [Incident Response](#18-incident-response)
19. [Security Headers and Transport Security](#19-security-headers-and-transport-security)
20. [Dependency and Supply Chain Security](#20-dependency-and-supply-chain-security)
21. [Security Testing Requirements](#21-security-testing-requirements)
22. [Future: Local Agent Security](#22-future-local-agent-security)
23. [Future: Per-Client Isolated Environments](#23-future-per-client-isolated-environments)
24. [Open Items from fracture-core Audit](#24-open-items-from-fracture-core-audit)

---

## 1. Executive Summary

gethacked.eu is an EU-focused pentesting and security services portal built on fracture-core. The portal manages the full pentest lifecycle: scope definition, offer/quote, client acceptance, pentester execution with structured findings, and report delivery.

As a security company's own portal, the security bar must be exemplary -- any compromise would be a devastating reputational event. This is not a generic SaaS application; it handles clients' vulnerability data, which is among the most sensitive data that exists.

The portal has three distinct actor types -- **Clients**, **Pentesters**, and **Administrators** -- each requiring strictly different permissions. The architecture enforces least privilege at every layer: role-based, org-scoped, and engagement-scoped access controls.

### Design Principles

- **Least privilege everywhere:** Every account type gets only the permissions it needs. No exceptions.
- **Zero IDORs:** All resource access verified against authenticated user's org AND role AND engagement assignment. Every query is scoped.
- **Defense in depth:** No single control is the sole line of defense.
- **Zero trust for scan targets:** Every target must have proven ownership before any testing.
- **Encryption by default:** All sensitive data encrypted at rest and in transit.
- **Audit everything:** Every security-relevant action is logged immutably.
- **Data minimization:** Collect and retain only what is necessary.
- **Assume breach:** Design for detection and containment, not just prevention.

---

## 2. Threat Model

### Assets

| Asset | Sensitivity | Impact of Compromise |
|-------|------------|---------------------|
| Pentest findings (vulnerabilities) | Critical | Client exposure, attack roadmap for adversaries |
| Pentest reports (compiled) | Critical | Full security posture exposed |
| Engagement details (scope, offers) | High | Reveals client infrastructure and commercial terms |
| Client organization data | High | GDPR breach, loss of trust |
| Target lists (IPs/domains/ranges) | High | Reveals client infrastructure map |
| Pentester notes and working data | High | Incomplete findings, methodology exposure |
| API keys | High | Unauthorized access, data exfiltration |
| User session tokens | High | Account takeover |
| Offer/pricing data | Medium | Competitive intelligence |
| Audit logs | Medium | Cover tracks for attackers |

### Threat Actors

1. **External attackers** -- Attempting to access client vulnerability data or abuse the portal.
2. **Malicious clients** -- Attempting to access other clients' data, escalate privileges, or request testing of targets they do not own.
3. **Rogue pentesters** -- Attempting to access engagements they are not assigned to, or exfiltrate client data beyond their authorized scope.
4. **Insider threats** -- Compromised admin accounts or disgruntled employees.
5. **Competitors** -- Corporate espionage targeting client lists and methodologies.
6. **Nation-state actors** -- Targeting vulnerability data of critical infrastructure clients.

### Attack Surfaces

- Web application (OWASP Top 10 -- all categories)
- Pentest workflow state transitions (unauthorized state changes)
- Cross-engagement data leakage (pentester A accessing pentester B's engagement)
- Cross-tenant data leakage (client A accessing client B's findings)
- IDOR on every resource (engagements, findings, reports, offers, targets)
- API endpoints (authentication bypass, injection, parameter manipulation)
- File uploads/downloads (malicious files, path traversal)
- OIDC integration (token manipulation, redirect attacks)
- Supply chain (compromised dependencies)
- Future: local agent communication channel

---

## 3. Pentest Workflow Security

The pentest workflow is the core feature of the application. Every state transition must be authorized and audited.

### 3.1 Workflow States and Authorized Transitions

```
SCOPE_DRAFT --> SCOPE_SUBMITTED --> OFFER_CREATED --> OFFER_SENT
     |               |                    |               |
     v               v                    v               v
 [Client]        [Client]            [Admin]          [Admin]
                                                         |
                                                         v
                                          OFFER_ACCEPTED / OFFER_NEGOTIATING
                                               |                |
                                               v                v
                                           [Client]         [Client]
                                               |
                                               v
                                       ENGAGEMENT_ACTIVE
                                               |
                                               v
                                          [Admin assigns pentesters]
                                               |
                                               v
                                       PENTEST_IN_PROGRESS
                                               |
                                               v
                                         [Pentesters add findings]
                                               |
                                               v
                                       REPORT_DRAFT --> REPORT_DELIVERED
                                            |               |
                                            v               v
                                        [Admin]         [Admin]
```

### 3.2 State Transition Authorization Matrix

| Transition | Who Can Trigger | Security Controls |
|-----------|----------------|-------------------|
| Create scope draft | Client (Owner/Admin/Member of their org) | Org membership check |
| Submit scope | Client who created the draft OR org Owner/Admin | Org membership + ownership check |
| Create offer | Platform Admin only | Admin role check; audit logged |
| Send offer to client | Platform Admin only | Admin role check |
| Accept offer | Client org Owner/Admin only | Org membership + role check; creates binding engagement |
| Request negotiation | Client org Owner/Admin only | Org membership + role check |
| Activate engagement | Platform Admin only | Validates offer was accepted |
| Assign pentester | Platform Admin only | Validates pentester user exists; creates engagement_assignment |
| Add finding | Assigned pentester only | Engagement assignment check (see Section 7) |
| Edit finding | Assigned pentester who created it, OR platform Admin | Creator check OR admin role |
| Create report draft | Platform Admin or assigned pentester with report scope | Engagement assignment check |
| Deliver report | Platform Admin only | Marks engagement as delivered; triggers client notification |

### 3.3 Workflow Invariants (enforced in code)

1. **No backward transitions** except OFFER_SENT to OFFER_NEGOTIATING (negotiation loop).
2. **Offer amount cannot be modified** after OFFER_ACCEPTED. Create a new offer for changes.
3. **Findings cannot be added** to an engagement that is not in PENTEST_IN_PROGRESS status.
4. **Reports cannot be delivered** if there are zero findings (likely an error).
5. **Engagement cannot be deleted** once in ENGAGEMENT_ACTIVE or later -- only archived.
6. **All state transitions are atomic** and logged to the audit trail with before/after state.

### 3.4 Pentest Request (Scope) Security

The scope definition form captures sensitive infrastructure information:

- **Target systems** (IPs, domains, URLs, internal hostnames)
- **IP ranges** (CIDR blocks)
- **Testing windows** (when testing is permitted -- dates and times)
- **Exclusions** (systems/IPs that must NOT be tested)
- **Contact info** (emergency contacts during testing)
- **Testing type** (black-box, grey-box, white-box)
- **Credentials** (if grey-box or white-box -- handled separately, see 3.5)

All scope data is:
- Encrypted at rest (Section 10)
- Scoped to the client's org (never visible to other clients)
- Visible only to platform admins who need to create the offer
- Visible to assigned pentesters only after engagement is activated

### 3.5 Credential Handling for Authenticated Tests

If the client provides credentials for grey-box or white-box testing:

- Credentials are stored in a separate encrypted vault (not in the main scope record).
- Accessible ONLY to assigned pentesters for the specific engagement.
- Encrypted with a per-engagement key (not the org master key).
- Automatically deleted 7 days after engagement completion.
- Never included in reports or findings.
- Never logged (not even hashed forms).
- Access to the credential vault is logged separately.

---

## 4. Role Model and Least Privilege

### 4.1 Mapping to fracture-core RBAC

fracture-core provides four roles: Owner > Admin > Member > Viewer. gethacked extends this with the concept of **account types** that determine what the roles mean in context.

**Account type is determined by the organization type:**

| Org Type | Description | Available Roles | How Created |
|---------|-------------|----------------|-------------|
| `client` | Client organization requesting pentests | Owner, Admin, Member, Viewer | Self-registration or admin creation |
| `pentester` | Individual pentester or pentesting team | Owner, Member | Admin creates; Owner = lead pentester |
| `platform` | gethacked platform administration | Owner, Admin | Bootstrap only; no self-registration |

### 4.2 Permission Matrix by Account Type

**Client org permissions:**

| Role | Request Pentest | View Own Org Engagements | Accept Offer | View Findings | Download Reports | Manage Org Members | Manage Targets |
|------|----------------|------------------------|-------------|--------------|-----------------|-------------------|----------------|
| Owner | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Admin | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Member | Yes | Yes | No | Yes | Yes | No | No |
| Viewer | No | Yes (read-only) | No | Yes (read-only) | Yes | No | No |

**Pentester permissions:**

| Capability | Access Rule |
|-----------|------------|
| View engagement details | Only engagements they are assigned to |
| View scope/targets | Only for assigned engagements |
| Add/edit findings | Only for assigned engagements in PENTEST_IN_PROGRESS |
| View findings | Only findings in their assigned engagements |
| View other clients' data | NEVER -- strict isolation |
| View unassigned engagements | NEVER |
| Create offers | NEVER |
| Manage users | NEVER |

**Platform admin permissions:**

| Capability | Access Rule |
|-----------|------------|
| View all engagements | Yes (needed to create offers, assign pentesters) |
| Create/send offers | Yes |
| Assign pentesters | Yes |
| Manage all users/orgs | Yes |
| View findings | Yes (needed for report QA) |
| Modify findings | No (only the creating pentester can modify) |
| Delete client data | Via GDPR deletion pipeline only (Section 15.3) |

### 4.3 Least Privilege Enforcement Rules

1. **No default access:** New users have no access to any engagement until explicitly granted.
2. **No implicit promotion:** Having access to one engagement does not grant access to other engagements for the same client.
3. **Pentester isolation:** Pentesters see ONLY their assigned engagements. They cannot list or search for other engagements.
4. **Client isolation:** Clients see ONLY their own org's engagements. They cannot discover other clients exist.
5. **Admin accountability:** All admin actions are logged with the acting admin's identity.
6. **Scope limitation:** API keys are always scoped to a specific org and cannot cross org boundaries.

---

## 5. Authentication and Session Management

### 5.1 Primary Authentication: OIDC via Zitadel

Inherited from fracture-core. The existing implementation is solid:

- PKCE (S256) on all authorization code flows
- Random CSRF state tokens with 5-minute TTL and one-time use
- Nonce validation on ID tokens
- Backchannel logout support with JWKS signature verification
- HTTP-only, Secure, SameSite=Lax cookies for JWT and ID token
- Session invalidation via `session_invalidated_at` column

**Required enhancements for gethacked:**

- **MFA enforcement:** Configure Zitadel to require MFA (TOTP or WebAuthn) for all portal users. Verify the `amr` (Authentication Methods References) claim in the ID token and reject logins that did not use a second factor.
- **Session binding:** Bind JWT sessions to the originating IP range (e.g., /24 for IPv4). If the client IP shifts outside the range, require re-authentication.
- **Session duration:** JWT expiration set to 30 minutes. Client-side refresh at 20 minutes (66% of expiration window).
- **Concurrent session limit:** Maximum 3 concurrent sessions per user. Alert on new session creation.

### 5.2 CSRF Protection (fracture-core gap: HIGH-1)

Implement the **double-submit cookie pattern**:

1. On login, set a `csrf_token` cookie (NOT HttpOnly, SameSite=Strict) containing a cryptographically random token.
2. All state-changing requests (POST, PUT, DELETE) must include the token as a header (`X-CSRF-Token`) or form field.
3. Server validates that the cookie value matches the header/field value.
4. Rotate the token on each session refresh.

This is critical for the pentest workflow -- CSRF on state transitions (accepting offers, submitting findings) would be catastrophic.

### 5.3 Authentication Context Propagation

Every authenticated request must carry:
1. **User identity** (from JWT)
2. **Org context** (from `org_pid` cookie, validated via membership)
3. **Account type** (derived from org type: client/pentester/platform)
4. **Role** (from org_members table)

This context is used by ALL authorization checks. It must be resolved in middleware, never re-derived in individual controllers.

---

## 6. Multi-Tenant Data Isolation and IDOR Prevention

### 6.1 Foundation: fracture-core Org Model

fracture-core provides strong org-scoped isolation:

- All queries filtered by `org_id` via SeaORM parameterized queries (no raw SQL)
- Organization membership verified on every request via `get_org_context_or_default()`
- UUIDv4 PIDs prevent enumeration (122 bits of entropy)

### 6.2 IDOR Prevention Strategy

IDOR (Insecure Direct Object Reference) is the single most critical vulnerability class for this application. A pentesting company with IDORs in their own portal would be a career-ending embarrassment.

**Rule: Every resource access must be verified against THREE dimensions:**

1. **Authentication:** Is the user logged in?
2. **Organization scope:** Does the resource belong to an org the user has access to?
3. **Permission level:** Does the user's role permit this action on this resource?

For pentesters, add a fourth dimension:
4. **Engagement assignment:** Is the pentester assigned to the engagement this resource belongs to?

**Implementation pattern (mandatory for ALL controllers):**

```rust
// CORRECT: Every query scoped by org_id AND verified
pub async fn view_finding(
    Path((engagement_pid, finding_pid)): Path<(String, String)>,
    // ...
) -> Result<Response> {
    let user = require_user!(user);
    let org_ctx = require_org_context!(jar, db, user);

    // 1. Find engagement scoped to the user's org
    let engagement = Engagement::find_by_pid_and_org(db, &engagement_pid, org_ctx.org.id)
        .await
        .ok_or(Error::NotFound)?;

    // 2. If pentester, verify assignment
    if org_ctx.account_type == AccountType::Pentester {
        require_engagement_assignment!(db, engagement.id, user.id);
    }

    // 3. Find finding scoped to the engagement
    let finding = Finding::find_by_pid_and_engagement(db, &finding_pid, engagement.id)
        .await
        .ok_or(Error::NotFound)?;

    // Now safe to render
}

// WRONG: Never do this
pub async fn view_finding_INSECURE(Path(finding_pid): Path<String>) -> Result<Response> {
    let finding = Finding::find_by_pid(db, &finding_pid).await?; // NO ORG SCOPE!
}
```

**Database-level defense (PostgreSQL RLS):**

```sql
ALTER TABLE findings ENABLE ROW LEVEL SECURITY;

-- Client access: only findings for engagements in their org
CREATE POLICY client_isolation ON findings
    USING (engagement_id IN (
        SELECT id FROM engagements
        WHERE client_org_id = current_setting('app.current_org_id')::int
    ));

-- Pentester access: only findings for their assigned engagements
CREATE POLICY pentester_isolation ON findings
    USING (engagement_id IN (
        SELECT engagement_id FROM engagement_assignments
        WHERE pentester_user_id = current_setting('app.current_user_id')::int
    ));
```

Set `app.current_org_id` and `app.current_user_id` at the beginning of each database transaction via `SET LOCAL`.

### 6.3 Isolation Migration Readiness

The current architecture uses shared multi-tenancy (single database, org-scoped queries). The long-term goal is per-client isolated environments (see Section 23). The current design must not block this migration.

**Design rules to preserve migration path:**

- **No cross-org JOINs:** Never write queries that JOIN data across organizations. Each org's data must be independently extractable.
- **Org-scoped file storage:** Use per-org prefixes in object storage (e.g., `s3://reports/{org_pid}/...`). This maps directly to per-tenant buckets later.
- **Configuration via org context:** All org-specific settings (encryption keys, retention policies, feature flags) must be stored in the org record or a related config table, not in application-level config files. This allows per-tenant configuration in isolated deployments.
- **No global sequences for tenant data:** Use UUIDs (already in place via fracture-core PIDs), not auto-incrementing integers that would conflict across isolated databases.
- **Stateless application tier:** No in-memory state tied to a specific org (except short-lived request context). This allows routing requests to different backends per tenant.
- **Database connection abstraction:** The database connection resolver should accept an org context. Currently this returns the shared connection; in the future it can route to a per-tenant database. Design the abstraction now even if the implementation is trivial.

**What would change at each isolation level:**

| Level | Database | Application | File Storage | Encryption Keys |
|-------|----------|-------------|-------------|----------------|
| Current: Shared multi-tenant | Single DB, RLS policies | Single deployment | Shared bucket, org prefixes | Per-org keys in shared KMS |
| Phase 2: DB-per-tenant | Separate DB per client org | Single deployment, connection routing | Per-org buckets | Per-org keys in shared KMS |
| Phase 3: Full isolation | Separate DB per client | Separate container/namespace per client | Per-org buckets with separate IAM | Per-org keys, potentially separate KMS instances |

**Security implications of isolation levels:**

- **Shared (current):** Blast radius of a SQL injection or application bug is all tenants. Mitigated by RLS, but RLS is defense-in-depth, not primary defense.
- **DB-per-tenant:** SQL injection blast radius reduced to single tenant. Application-level bugs could still route to wrong DB if connection resolver has a flaw.
- **Full isolation:** Blast radius of most vulnerabilities limited to single tenant. Only shared infrastructure (load balancer, orchestrator, KMS) is cross-tenant attack surface.

### 6.4 Cross-Tenant Data Leak Prevention

- Never include org-identifying information in error messages.
- Return `404 Not Found` for unauthorized resources (not `403 Forbidden`) to prevent existence leakage.
- Use separate S3 prefixes with IAM policies per org for report/file storage.
- Ensure database indexes do not leak information about other tenants' data volume.
- All list endpoints return ONLY resources scoped to the current org.

### 6.5 IDOR Test Cases (mandatory before launch)

The following must be tested for every endpoint:

1. **Cross-org access:** User from org A tries to access resources from org B by guessing/enumerating PIDs.
2. **Cross-engagement access (pentester):** Pentester assigned to engagement X tries to access findings from engagement Y.
3. **Role escalation:** Viewer tries to perform Member actions; Member tries Admin actions.
4. **Account type escalation:** Client user tries to access admin-only endpoints; pentester tries to create offers.
5. **Parameter manipulation:** Changing `org_pid` or `engagement_pid` in the URL while keeping the same session.
6. **Batch operations:** If any batch endpoint exists, verify EVERY item in the batch is authorized.

---

## 7. Engagement-Scoped Access (Pentester Isolation)

This is the most complex access control requirement in the system. Pentesters must access client data (scope, targets, credentials) but ONLY for their assigned engagements.

### 7.1 Data Model

```
engagement_assignments
    - id (PK)
    - engagement_id (FK -> engagements)
    - pentester_user_id (FK -> users)
    - assigned_by (FK -> users, must be platform admin)
    - assigned_at (timestamp)
    - revoked_at (timestamp, nullable)
    - scopes: ["findings:write", "scope:read", "credentials:read"]
```

### 7.2 Access Rules

| Resource | Client Org | Assigned Pentester | Unassigned Pentester | Platform Admin |
|----------|-----------|-------------------|---------------------|----------------|
| Engagement metadata | Yes (own org) | Yes (assigned) | NO | Yes |
| Scope details | Yes (own org) | Yes (assigned) | NO | Yes |
| Target list | Yes (own org) | Yes (assigned) | NO | Yes |
| Test credentials | NO (hidden after submission) | Yes (assigned, if credentialed test) | NO | NO |
| Findings | Yes (own org, read-only) | Yes (assigned, read-write) | NO | Yes (read-only) |
| Reports | Yes (own org) | Yes (assigned, read-only after delivery) | NO | Yes |
| Offers/pricing | Yes (own org) | NO | NO | Yes |

### 7.3 Assignment Security

- Only platform admins can assign pentesters to engagements.
- Assignment is logged with the admin's identity.
- Assignments can be revoked (soft-delete with `revoked_at` timestamp). Revoked pentesters lose access immediately.
- A pentester can be assigned to multiple engagements, but each assignment is independent.
- Assignment does NOT grant access to the client's org -- only to the specific engagement's data.

### 7.4 Pentester Data Boundary

Pentesters must NEVER be able to:
- List client organizations
- See client billing/payment information
- Access engagements they are not assigned to
- See which other pentesters are assigned to the same engagement (unless explicitly needed for collaboration)
- Modify engagement state (only add findings within their assigned engagement)
- Download/export all findings at once (export is per-engagement, rate-limited)

### 7.5 Macro: `require_engagement_assignment!`

```rust
/// Verifies the current user is assigned to the given engagement.
/// Returns 404 (not 403) if not assigned, to prevent existence leakage.
#[macro_export]
macro_rules! require_engagement_assignment {
    ($db:expr, $engagement_id:expr, $user_id:expr) => {
        let assignment = engagement_assignments::Model::find_active_assignment(
            $db, $engagement_id, $user_id
        ).await;
        if assignment.is_none() {
            return Ok(/* 404 Not Found */);
        }
    };
}
```

---

## 8. Scope Definition and Target Authorization

### 8.1 Scope Submission Security

The scope definition form captures sensitive infrastructure information:

- **Target systems** (IPs, domains, URLs)
- **IP ranges** (CIDR blocks)
- **Testing windows** (dates/times)
- **Exclusions** (systems that must NOT be tested)
- **Contact info** (emergency contacts)
- **Testing type** (black-box, grey-box, white-box)

All inputs are validated per Section 13 (Input Validation). Additionally:

- **Testing windows** must be in the future and within reasonable bounds (not more than 1 year out).
- **Exclusions** are validated as valid IPs/domains/CIDRs.
- **Contact info** is validated as valid phone numbers/emails.
- **Free-text fields** (notes, special instructions) are sanitized for XSS but preserved for content.

### 8.2 Target Ownership Verification

Before any pentest can be executed, the client must prove ownership of the declared targets. This is a legal requirement -- unauthorized testing is a criminal offense under EU Directive 2013/40/EU.

**For domains:**

1. **DNS TXT record:** Client adds `_gethacked-verify=<token>` as a TXT record.
2. **HTTP well-known file:** Client places token at `https://<domain>/.well-known/gethacked-verify.txt`.
3. **HTML meta tag:** Client adds `<meta name="gethacked-verify" content="<token>">`.

**For IP addresses:**

1. **Reverse DNS + domain verification:** PTR record pointing to a verified domain.
2. **WHOIS/RDAP confirmation:** Manual review required.
3. **Hosted file verification:** Token served via HTTP at the IP.

**For IP ranges (CIDR):**

1. Verify at least 2 non-adjacent IPs within the range.
2. Ranges larger than /24 require manual admin approval.

### 8.3 Verification Token Properties

- 32 bytes of cryptographic randomness (256 bits), hex-encoded.
- Unique per target registration attempt.
- Expires after 7 days if not verified.
- One-time use; stored hashed (SHA-256) in database.
- Plaintext shown to client only once.

### 8.4 Target Status Model

```
PENDING     -- registered, awaiting verification
VERIFIED    -- ownership confirmed, pentesting allowed
EXPIRED     -- verification expired (>90 days), re-verification needed
REVOKED     -- manually revoked by client or admin
SUSPENDED   -- flagged for abuse, under review
```

Only `VERIFIED` targets can be included in an engagement scope. The system must verify target status at engagement activation time, not just at scope submission.

### 8.5 Abuse Prevention

- Rate-limit target registration to 10 per org per hour.
- Blocklist for well-known infrastructure (major cloud control planes, government domains).
- Maintain a global blocklist of targets that must never be tested.
- Log all verification attempts including failures.
- Targets for the same domain/IP cannot be registered by multiple orgs without admin approval.

---

## 9. Finding and Report Security

### 9.1 Finding Data Model Security

Each finding has structured fields:

| Field | Type | Security Considerations |
|-------|------|------------------------|
| Description | Rich text | XSS sanitization; stored encrypted |
| Technical Description | Rich text | XSS sanitization; stored encrypted; may contain reproduction steps |
| Impact | Rich text | XSS sanitization; stored encrypted |
| Recommendation | Rich text | XSS sanitization; stored encrypted |
| Severity | Enum (Extreme/High/Elevated/Moderate/Low) | Validate against the five allowed values; reject invalid severities |
| Category | Enum | Validate against allowlist (Injection, Broken Auth, XSS, IDOR, Misconfiguration, etc.) |
| Evidence/Screenshots | File attachments | Secure file handling (Section 17); image-only validation |
| Engagement ID | FK | Immutable after creation; enforced at DB level |
| Created By | FK (pentester user) | Immutable; audit trail |

### 9.2 Finding Access Control

- **Creation:** Only pentesters assigned to the engagement, only while engagement is in PENTEST_IN_PROGRESS.
- **Modification:** Only the creating pentester OR a platform admin (for QA/editorial fixes).
- **Viewing (client):** Only after REPORT_DRAFT or REPORT_DELIVERED status, and only for the client's own org.
- **Viewing (pentester):** Only for assigned engagements.
- **Deletion:** Soft-delete only, by the creating pentester or admin. Deletion is logged. Content is retained for audit purposes (marked as deleted, not destroyed).

### 9.3 Finding Content Sanitization

Findings may contain user-supplied HTML/Markdown with technical content (code snippets, HTTP requests, etc.). This is a high-risk XSS vector.

- Use a strict allowlist-based HTML sanitizer (e.g., `ammonia` crate in Rust).
- Allow: `<p>`, `<br>`, `<strong>`, `<em>`, `<code>`, `<pre>`, `<ul>`, `<ol>`, `<li>`, `<table>`, `<tr>`, `<td>`, `<th>`, `<h1>`-`<h6>`, `<blockquote>`, `<a href>` (with URL validation).
- Strip: `<script>`, `<iframe>`, `<object>`, `<embed>`, `<form>`, `<input>`, `<style>`, event handlers (`onclick`, `onerror`, etc.), `javascript:` URLs, `data:` URLs.
- Sanitize on input (before storage) AND on output (before rendering).
- Store the original input alongside the sanitized version for audit purposes.

### 9.4 Severity Validation

Findings use a simple five-level severity scale standard in pentest reporting:

- **Extreme** -- Immediate risk of full system compromise or mass data breach.
- **High** -- Significant risk, exploitable with moderate effort.
- **Elevated** -- Notable risk, requires specific conditions to exploit.
- **Moderate** -- Limited risk, exploitation requires significant effort or unlikely conditions.
- **Low** -- Minor risk, limited impact, best-practice recommendations.

Validation rules:
- Severity is a server-side enum. Reject any value not in the five allowed levels.
- Severity cannot be modified after report delivery without creating a new finding revision (audit trail).
- Default severity is not set -- pentesters must explicitly choose a level for each finding.

### 9.5 Report Compilation Security

- Reports are compiled from findings server-side. The client/pentester does not upload a report file.
- Report generation runs in a sandboxed process with no network access.
- Generated PDFs are scanned for embedded scripts before storage.
- Reports are encrypted before storage (Section 10).
- Each report is watermarked with the requesting user's identity and a timestamp (to trace leaks).

### 9.6 Report Sharing

Reports can be shared with external parties (auditors, regulators):

- **Share links:** Time-limited URL with 32-byte cryptographic random token. Does not require authentication.
- **Link properties:**
  - Expiration: default 7 days, maximum 30 days.
  - Access count limit: configurable.
  - IP restriction: optional.
  - Password protection: optional (Argon2id hashed).
- **Audit:** Every access logged (IP, User-Agent, timestamp).
- **Revocation:** Instant, by any Admin/Owner in the client org.

### 9.7 Report Redaction

- Support redacted variants that hide specific findings.
- Redacted reports are generated as separate encrypted files, not derived on-the-fly.
- Redaction choices are logged.

---

## 10. Encryption at Rest

### 10.1 Encryption Architecture

**Application-level envelope encryption:**

```
Per-org master key (stored in external KMS: AWS KMS / HashiCorp Vault)
    |
    v
Per-record data encryption key (DEK) -- randomly generated
    |
    v
DEK encrypts content (AES-256-GCM)
    |
    v
DEK encrypted by org master key
    |
    v
Encrypted DEK stored alongside encrypted content
```

For engagement credentials (grey-box/white-box tests), use a **per-engagement key** instead of the org master key, so credentials can be destroyed independently.

### 10.2 Key Management

- **KMS:** HashiCorp Vault or AWS KMS. Master keys never leave the HSM.
- **Hierarchy:** Root key -> Org master key -> Data encryption key (DEK).
- **Rotation:** Org master keys rotated annually. Old keys retained for decryption only.
- **Access logging:** All KMS operations logged with requestor identity.

### 10.3 What Gets Encrypted

| Data | Encryption | Justification |
|------|-----------|---------------|
| Finding content (all text fields) | AES-256-GCM per record | Contains vulnerability details |
| Report PDF/HTML files | AES-256-GCM per file | Contains vulnerability details |
| Scope details | AES-256-GCM per record | Contains infrastructure details |
| Test credentials | AES-256-GCM per-engagement key | Most sensitive data; independent deletion |
| Offer details | AES-256-GCM per record | Commercial sensitivity |
| Target metadata | Not encrypted (indexed) | Needed for filtering/search |
| API keys | Hashed (Argon2id) | Never need to decrypt |

### 10.4 In-Transit Encryption

- External traffic: TLS 1.3 (TLS 1.2 permitted with restricted ciphers).
- Internal service-to-service: mTLS.
- Database connections: TLS required (`sslmode=verify-full`).

---

## 11. API Key Management

### 11.1 Key Format

- 32 bytes of cryptographic randomness, Base62-encoded (43 characters).
- Prefix: `ghk_live_` (production) or `ghk_test_` (sandbox).
- Full key shown once at creation. Only prefix (first 8 chars after `ghk_*_`) stored in plaintext for lookup.
- Full key hashed with Argon2id (memory=64MB, iterations=3, parallelism=1).

### 11.2 Key Scoping

```json
{
  "key_id": "ghk_live_a1b2c3d4...",
  "org_id": 42,
  "created_by": 17,
  "scopes": ["engagements:read", "findings:read", "reports:read"],
  "allowed_ips": ["203.0.113.0/24"],
  "expires_at": "2026-09-10T00:00:00Z"
}
```

**Available scopes (client API keys):**
- `engagements:read` -- View engagement status and details
- `findings:read` -- View findings for completed engagements
- `reports:read` -- Download reports
- `targets:read` -- List verified targets
- `targets:write` -- Register new targets

**Pentester API keys** are NOT supported in v1 (browser-only access for pentesters to maintain tighter session control).

Keys CANNOT exceed the creating user's role permissions.

### 11.3 Key Lifecycle

- Maximum lifetime: 1 year. Default: 90 days.
- Warning 14 days before expiration.
- Immediate revocation supported.
- Maximum 10 active keys per user, 50 per org.

### 11.4 Key Authentication Flow

1. Client sends `Authorization: Bearer ghk_live_a1b2c3d4...`.
2. Server extracts prefix, looks up key record.
3. Server hashes full key with Argon2id and compares.
4. Validates: not expired, not revoked, client IP in `allowed_ips`.
5. Authorizes against key scopes and org context.

API key auth and OIDC session auth are mutually exclusive per request.

---

## 12. Rate Limiting Strategy

### 12.1 Rate Limit Tiers

| Endpoint Category | Per IP | Per API Key | Per Org | Window |
|-------------------|--------|-------------|---------|--------|
| OIDC auth | 20/min | N/A | N/A | Sliding |
| Scope submission | 5/min | N/A | 10/hour | Fixed |
| Finding creation | 30/min | N/A | 100/hour | Fixed |
| Report download | 30/min | 60/min | N/A | Sliding |
| Target registration | 10/min | 10/min | 30/hour | Fixed |
| General API | 120/min | 300/min | N/A | Sliding |
| Share link access | 30/min/link | N/A | N/A | Sliding |

### 12.2 Implementation

- `governor` crate with Redis backend for distributed rate limiting.
- Axum middleware layers, most restrictive first.
- Response: `429 Too Many Requests` with `Retry-After` header.
- Headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`.

### 12.3 OIDC State Store Cap

Add 10,000 entry maximum to `OidcStateStore`. Return `503` when full.

---

## 13. Input Validation

### 13.1 Domain Validation

```
1. Parse as valid hostname per RFC 1123.
2. Reject: characters outside [a-zA-Z0-9.-], label >63 chars, total >253 chars,
   bare TLDs, reserved domains (localhost, *.local, *.internal, *.test, *.example),
   resolves to private/reserved IP.
3. Normalize: lowercase, strip trailing dot.
```

### 13.2 IP Address Validation

```
1. Parse via Rust std::net::IpAddr.
2. Reject reserved ranges (see 13.3).
3. CIDR: reject ranges larger than /24 (IPv4) or /48 (IPv6) without manual approval.
4. Store canonical form.
```

### 13.3 Blocked IP Ranges (SSRF Prevention)

Reject targets resolving to:
- `127.0.0.0/8`, `::1/128` (loopback)
- `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` (RFC 1918)
- `169.254.0.0/16`, `fe80::/10` (link-local)
- `fc00::/7` (unique local)
- `100.64.0.0/10` (carrier-grade NAT)
- `0.0.0.0/8` (current network)
- `169.254.169.254` (cloud metadata)

**Note:** For the future local agent feature (Section 22), private IP ranges will be permitted ONLY for agent-mediated scans, never for direct platform-initiated scans.

### 13.4 DNS Resolution Safety

1. Check ALL resolved IPs against blocklist.
2. Use application's own resolver (prevent DNS rebinding).
3. Pin resolved IP for scan duration (prevent TOCTOU).
4. 5-second resolution timeout.

### 13.5 Finding Content Validation

- Severity: validate against the five allowed enum values (Extreme/High/Elevated/Moderate/Low).
- Categories: validate against defined enum (reject unknown categories).
- Rich text: sanitize with allowlist (Section 9.3).
- File attachments: validate MIME type and magic bytes (Section 17).

### 13.6 Command Injection Prevention

- NEVER interpolate user input into shell commands.
- Use `Command::new()` with explicit argument separation.
- Character allowlist for target strings: `[a-zA-Z0-9.:-/]`.
- Scan workers in isolated containers.

---

## 14. Audit Logging

### 14.1 Events to Log

**Authentication events:**
- Login success/failure (user, IP, method, MFA used)
- Logout, session refresh, API key auth success/failure
- Backchannel logout received

**Authorization events:**
- Access denied (user, resource, required role, actual role)
- Role change, member added/removed
- **Engagement assignment created/revoked** (admin, pentester, engagement)

**Pentest workflow events:**
- Scope created/submitted (user, org, engagement)
- Offer created/sent/accepted/negotiated (admin, client org, engagement, amount)
- Engagement activated (admin, engagement)
- Pentester assigned/unassigned (admin, pentester, engagement)
- **Finding created/modified/deleted** (pentester, engagement, finding PID, severity)
- Report generated/delivered (admin, engagement, report PID)
- **Credentials accessed** (pentester, engagement, timestamp)

**Data access events:**
- Report viewed/downloaded (user, org, report ID, format)
- Report shared/share link accessed
- Findings viewed (user, engagement)

**API key events:**
- Key created/revoked/expired

**Administrative events:**
- Org created/deleted, settings changed
- Data export/deletion requested

### 14.2 Log Format

Structured JSON with: `timestamp`, `event_type`, `severity`, `actor` (user_id, user_pid, auth_method, ip, user_agent, account_type), `org` (org_id, org_pid), `resource` (type, id), `details`, `request_id`.

### 14.3 Log Storage and Retention

- Append-only table. No DELETE/UPDATE permissions for application DB user.
- Ship to external SIEM in near-real-time.
- Retention: 2 years minimum (NIS2), 5 years for financial contexts.
- No sensitive data in logs (use references/IDs).
- Hash-chain for tamper detection.

### 14.4 Alerting

Trigger alerts on:
- Multiple failed auth attempts (>5 in 10 minutes per IP)
- API key used from new IP outside `allowed_ips`
- **Cross-org data access attempt** (should never happen; indicates IDOR bug)
- **Cross-engagement access attempt by pentester** (indicates assignment bypass)
- Privilege escalation attempt
- Rate limit exceeded persistently
- Anomalous data download volume
- **Credential vault access outside testing window**
- **Finding modification after report delivery**

---

## 15. GDPR Compliance

### 15.1 Data Classification

| Data Category | GDPR Classification | Lawful Basis | Retention |
|--------------|-------------------:|-------------|-----------|
| User account (name, email) | Personal data | Contract (Art. 6(1)(b)) | Duration of account + 30 days |
| Engagement scope/findings | May contain personal data | Legitimate interest (Art. 6(1)(f)) + DPA | 1 year after engagement completion, then deleted |
| Test credentials | Personal data / secrets | Contract + DPA | 7 days after engagement completion |
| Audit logs (user IDs, IPs) | Personal data (pseudonymized) | Legal obligation (Art. 6(1)(c)) | 2 years minimum |
| Payment data | Personal data | Contract (Art. 6(1)(b)) | Per tax law (7 years) |

### 15.2 Data Processing Agreement (DPA)

Every client MUST sign a DPA before the first engagement. The DPA covers:

- **Parties:** gethacked.eu as processor, client as controller
- **Subject matter:** Manual penetration testing, vulnerability assessment, report generation
- **Personal data types:** IP addresses, domain names, email addresses, system configurations, vulnerability details, potentially any data discovered during testing
- **Sub-processors:** Cloud providers, explicitly listed with notification of changes
- **Breach notification:** Within 36 hours (stricter than GDPR's 72 hours)
- **Audit rights:** Client may audit with 30 days' notice
- **Deletion:** All client data deleted within 30 days of contract termination with certificate of destruction
- **Pentester obligations:** Pentesters are bound by the same confidentiality and data handling requirements

### 15.3 Right to Erasure (Art. 17)

1. Client requests deletion via portal or support.
2. Data immediately marked "pending deletion" and becomes inaccessible.
3. Within 30 days: delete all engagements, findings, reports, targets, credentials, API keys. Anonymize audit logs. Remove user accounts if requested.
4. Certificate of destruction with SHA-256 hashes of deleted records.
5. Retain certificate and anonymized audit entries for legal retention period.

### 15.4 Right to Data Portability (Art. 20)

- Export all engagement data, findings, reports in machine-readable JSON.
- Owner/Admin only. Rate-limited to 1 export per org per day.
- Export encrypted with user-chosen password.

### 15.5 Privacy by Design (Art. 25)

- New accounts have minimal permissions (Viewer).
- All sensitive data encrypted at rest by default.
- Data retention enforced automatically.
- No third-party tracking.

### 15.6 DPIA

Required before launch due to high-risk processing of vulnerability data. Reviewed annually.

---

## 16. NIS2 Compliance

### 16.1 Applicability

gethacked.eu is likely an "important entity" under NIS2 as a managed security service provider. Clients in scope will require gethacked to meet NIS2 standards.

### 16.2 Security Measures (Art. 21)

| NIS2 Requirement | Implementation |
|-----------------|----------------|
| Risk analysis | This document; annual risk assessment |
| Incident handling | Section 18 |
| Business continuity | Encrypted daily backups; multi-AZ; recovery procedures |
| Supply chain security | Section 20; sub-processor assessments |
| Secure development | Secure SDLC; code review; dependency scanning |
| Vulnerability disclosure | security.txt; bug bounty |
| Risk assessment | Annual pentest of own platform |
| Cryptography | Section 10; TLS 1.3; AES-256-GCM |
| MFA | Section 5.1 |

### 16.3 Incident Notification (Art. 23)

- Early warning: within 24 hours
- Incident notification: within 72 hours
- Final report: within 1 month

### 16.4 Supply Chain

- Register of all sub-processors with security certifications.
- 24-hour incident notification from sub-processors.
- Annual security assessment. Contractual audit rights.

---

## 17. Secure File Handling

### 17.1 Report Files

- Generated server-side in sandboxed process.
- Scanned for embedded scripts before storage.
- Encrypted before storage.
- Watermarked with requesting user identity.

### 17.2 Report Download

- Pre-signed URLs, 5-minute expiration.
- `Content-Disposition: attachment` (force download).
- Explicit `Content-Type` (application/pdf, application/zip).
- Decrypted in memory, streamed to client; never written to disk unencrypted.

### 17.3 Evidence Uploads (Finding Attachments)

Pentesters may upload screenshots and evidence files with findings:

- Maximum file size: 50 MB per file, 200 MB per engagement.
- Allowed types: PNG, JPG, GIF, PDF, TXT, JSON, XML. Validate by magic bytes.
- Generate UUIDv4 filename; never use client-provided name.
- Scan with ClamAV before processing.
- Strip all metadata (EXIF).
- Store in separate bucket from application code, no execute permissions.

### 17.4 Storage

- S3-compatible object storage with SSE-KMS.
- Versioning disabled (right to erasure).
- Access logging enabled.
- No public access. Application service account only.
- Per-org prefixes with IAM policies.

---

## 18. Incident Response

### 18.1 Classification

| Severity | Definition | Response Time | Examples |
|----------|-----------|---------------|---------|
| P1 | Active data breach, client findings exposed | Immediate (1 hour) | DB compromise, IDOR exploitation |
| P2 | Security control failure, potential breach | Within 4 hours | Auth bypass, privilege escalation |
| P3 | Security anomaly, no confirmed impact | Within 24 hours | Unusual patterns, failed brute force |
| P4 | Minor issue, no operational impact | Within 72 hours | Non-critical vuln, policy violation |

### 18.2 Response Plan

1. **Detection:** Automated alerting + monitoring + external reports (security.txt, bug bounty)
2. **Triage:** On-call engineer assesses within 30 minutes
3. **Containment:** Isolate systems, revoke sessions/API keys, block attacker IPs
4. **Eradication:** Root cause analysis, patch vulnerabilities
5. **Recovery:** Restore from backups if needed, re-enable with enhanced monitoring
6. **Notification:** Internal (immediately), clients (36 hours), authorities (24-72 hours per NIS2/GDPR)
7. **Post-mortem:** Within 5 business days, sanitized version to affected clients

### 18.3 Forensic Readiness

- Append-only tamper-evident audit logs.
- Database transaction logs retained 30 days.
- Network flow logs retained 30 days.
- System snapshots for forensic analysis.

---

## 19. Security Headers and Transport Security

### 19.1 Inherited from fracture-core

- `Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; form-action 'self'; base-uri 'self'; frame-ancestors 'none'`
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `Referrer-Policy: strict-origin-when-cross-origin`
- `X-Permitted-Cross-Domain-Policies: none`
- `Strict-Transport-Security: max-age=63072000; includeSubDomains`

### 19.2 Additional Headers

- `Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=()`
- `Cross-Origin-Opener-Policy: same-origin`
- `Cross-Origin-Embedder-Policy: require-corp`
- `Cross-Origin-Resource-Policy: same-origin`

### 19.3 TLS

- Minimum TLS 1.2, prefer TLS 1.3.
- ECDHE-only key exchange. AEAD ciphers only.
- OCSP stapling. CT log monitoring.
- HSTS preload list candidate.

---

## 20. Dependency and Supply Chain Security

### 20.1 Rust Dependencies

- `cargo audit` in CI on every commit. Block merges with known vulnerabilities.
- Pin versions in `Cargo.lock`. Review updates before merging.
- `cargo deny` for license compliance and unmaintained crate blocking.

### 20.2 Container Security

- Distroless/scratch images. Trivy/Grype scanning.
- Non-root, read-only filesystem. No shell in production.
- cosign/Sigstore image signing.

### 20.3 Infrastructure

- IaC (Terraform/OpenTofu) with state encryption.
- Secrets via Vault/AWS Secrets Manager (never env vars or config files).
- Network segmentation: web, app, database, scan worker tiers.

---

## 21. Security Testing Requirements

As a security company, the portal must undergo the most rigorous testing possible.

### 21.1 Mandatory IDOR Testing

For EVERY endpoint that accepts a resource identifier (PID), test:

| Test Case | Method | Expected Result |
|-----------|--------|----------------|
| Cross-org engagement access | Client A's session, Client B's engagement PID | 404 |
| Cross-org finding access | Client A's session, finding PID from Client B's engagement | 404 |
| Cross-org report download | Client A's session, Client B's report PID | 404 |
| Pentester unassigned engagement | Pentester's session, unassigned engagement PID | 404 |
| Pentester cross-engagement finding | Pentester's session, finding from different engagement | 404 |
| Client accessing offer internals | Client's session, admin-only offer endpoints | 403 or 404 |
| Viewer performing writes | Viewer's session, POST to create finding | 403 |
| Member accepting offer | Member's session (not Owner/Admin), accept offer endpoint | 403 |
| Parameter substitution | Valid session, swap org_pid/engagement_pid in URL | 404 |

### 21.2 Privilege Escalation Testing

| Test Case | Method | Expected Result |
|-----------|--------|----------------|
| Client creating offer | Client session, POST to create offer | 403 |
| Pentester modifying engagement state | Pentester session, state transition endpoint | 403 |
| Member role self-promotion | Member session, role change to Admin/Owner | 403 |
| Viewer submitting scope | Viewer session, POST scope | 403 |
| API key scope escalation | Key with `findings:read`, attempt `findings:write` | 403 |
| Client org viewing pentester list | Client session, pentester listing endpoint | 403/404 |

### 21.3 OWASP Top 10 Testing

| Category | Specific Tests for gethacked |
|----------|------------------------------|
| A01: Broken Access Control | All IDOR and privilege escalation tests above; workflow state transition authorization |
| A02: Cryptographic Failures | Verify encryption at rest for findings/reports; verify TLS config; verify password/key hashing |
| A03: Injection | SQL injection on all query parameters; XSS in finding content fields; command injection in target inputs |
| A04: Insecure Design | Workflow bypass (skip offer acceptance); rate limit bypass; business logic flaws |
| A05: Security Misconfiguration | Security headers present; debug endpoints disabled; default credentials removed; error messages sanitized |
| A06: Vulnerable Components | `cargo audit` clean; container image scan clean; no known CVEs in dependencies |
| A07: Auth Failures | Session fixation; JWT manipulation; API key brute force; MFA bypass; concurrent session handling |
| A08: Data Integrity Failures | Finding content integrity (no unauthorized modification); report tampering detection; audit log integrity |
| A09: Logging Failures | All security events logged; logs not containing sensitive data; alerting functional |
| A10: SSRF | Target input -> internal IP access; DNS rebinding; cloud metadata access |

### 21.4 Additional Security Tests

- **Cross-tenant data leakage:** Verify no data from org A appears in any response to org B users, including error messages, timing differences, and response sizes.
- **Engagement boundary enforcement:** Verify pentesters cannot access data outside their assigned engagements through any endpoint, including list/search/filter endpoints.
- **Workflow state integrity:** Verify engagement state machine cannot be bypassed (e.g., creating findings before engagement is active, delivering reports before findings exist).
- **Credential isolation:** Verify test credentials are accessible ONLY to assigned pentesters, not to client users, admins viewing findings, or report generation.
- **Concurrent access:** Verify race conditions cannot bypass authorization checks (e.g., TOCTOU on engagement assignment revocation).
- **Data deletion completeness:** After GDPR deletion, verify no data remnants exist in database, file storage, caches, or logs (beyond anonymized audit entries).

---

## 22. Future: Local Agent Security

**Out of scope for v1, but the architecture must be designed to accommodate this.**

A downloadable agent will allow clients to run assessments on localhost/internal networks. Security considerations for when this is implemented:

### 22.1 Agent Communication

- Agent communicates with platform via authenticated WebSocket or mTLS gRPC channel.
- Agent authenticates using a short-lived, engagement-scoped token (not the user's session token or API key).
- Token is generated when the engagement is activated and expires when the engagement is completed.
- All agent traffic is encrypted (TLS 1.3).

### 22.2 Agent Trust Model

- The agent runs on the client's infrastructure, so it is **partially trusted** -- it can see the client's internal network, but it should not be able to access other clients' data on the platform.
- Agent-submitted scan results are treated as **untrusted input** -- validated and sanitized before storage.
- The agent cannot initiate scans against targets not declared in the engagement scope.

### 22.3 Private IP Handling

- When the agent is mediating, private IP ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16) are permitted as scan targets.
- The platform NEVER connects to private IPs directly -- all internal scanning is agent-mediated.
- The blocklist in Section 13.3 applies only to platform-initiated connections. Agent-reported results from private IPs are accepted but validated.

### 22.4 Data Model Extensibility

- The `engagements` table should include an `agent_enabled` boolean.
- The `scan_targets` table should include a `requires_agent` boolean for internal targets.
- Finding sources should be tracked: `source` field with values `manual` (pentester), `agent`, `platform_scan`.

---

## 23. Future: Per-Client Isolated Environments

**Out of scope for v1, but the architecture must be designed to support this.**

The long-term goal is complete isolation per client -- when a new client creates an account, a new isolated environment spins up automatically. This section covers the security implications and requirements for each isolation level.

### 23.1 Isolation Levels (Migration Path)

**Level 1 (Current): Shared multi-tenant**
- Single database, single application deployment.
- Isolation via org-scoped queries + PostgreSQL RLS.
- Single encryption key namespace in KMS.
- Shared object storage with per-org prefixes.

**Level 2 (Future): Database-per-tenant**
- Separate PostgreSQL database per client org.
- Single application deployment with connection routing.
- Provisioned automatically on org creation.
- Each tenant DB has its own credentials (no shared superuser across tenants).

**Level 3 (Future): Full isolated deployment**
- Separate container/namespace per client org (Kubernetes namespace or Podman container).
- Separate database per client.
- Separate object storage bucket with dedicated IAM role.
- Potentially separate KMS key scope.
- Auto-provisioned via orchestration on account creation.

### 23.2 Security Benefits by Level

| Threat | Shared (L1) | DB-per-tenant (L2) | Full Isolation (L3) |
|--------|------------|--------------------|--------------------|
| SQL injection blast radius | All tenants | Single tenant | Single tenant |
| Application bug cross-tenant leak | Possible (mitigated by RLS) | Possible (connection routing bug) | Eliminated (separate processes) |
| Memory-based side channels | Possible (shared process) | Possible (shared process) | Eliminated |
| Noisy neighbor (resource exhaustion) | Possible | Partially mitigated (separate DB) | Eliminated (resource quotas per namespace) |
| Compromised application credential | All tenants exposed | All DBs if shared app credential | Single tenant only |
| Backup/restore isolation | Must filter by org | Per-tenant backup/restore | Per-tenant backup/restore |
| GDPR deletion confidence | Must verify all tables | Drop database | Destroy namespace + storage |

### 23.3 Auto-Provisioning Security Requirements

When auto-provisioning is implemented:

- **Provisioning must be idempotent:** Re-running provisioning for an existing org must not create duplicate resources or reset existing data.
- **Provisioning credentials:** The provisioning service needs elevated permissions (create databases, namespaces). These credentials must be tightly scoped and rotated regularly. The provisioning service must NOT use the same credentials as the application runtime.
- **Provisioning audit trail:** Every provisioning event (create, scale, destroy) must be logged immutably.
- **Tenant destruction:** When a client org is deleted (GDPR Art. 17), the entire isolated environment must be destroyed. For DB-per-tenant: drop the database. For full isolation: destroy the namespace/container, database, storage bucket, and KMS key scope.
- **Secrets per tenant:** Each provisioned environment must have unique database credentials, unique API signing keys, and unique encryption keys. Never share secrets across tenant environments.
- **Network isolation:** In full isolation (L3), each tenant namespace must have network policies preventing cross-namespace traffic. Only the platform admin services and the load balancer/ingress can reach tenant namespaces.
- **Resource limits:** Each tenant environment must have CPU/memory/storage quotas to prevent a single tenant from consuming all cluster resources.

### 23.4 Design Rules for Current Code (Do Not Block Migration)

These rules are enforced NOW in the shared model to ensure the migration path remains viable:

1. **No cross-org JOINs** in any query (see Section 6.3).
2. **UUIDs for all identifiers** (already in place via fracture-core PIDs).
3. **Per-org object storage prefixes** (maps to per-tenant buckets later).
4. **Database connection abstraction** that accepts org context (trivial now, routes to per-tenant DB later).
5. **Org-scoped configuration** stored in database, not application config files.
6. **Stateless application tier** with no org-specific in-memory state beyond request scope.
7. **No hardcoded single-database assumptions** in migration scripts -- migrations must be runnable per-tenant.

---

## 24. Open Items from fracture-core Audit

| Finding | Status | Priority |
|---------|--------|----------|
| HIGH-1: No CSRF tokens | Open -- implement double-submit cookie (Section 5.2) | Must fix before launch |
| MEDIUM-2: No rate limiting | Open -- implement with governor (Section 12) | Must fix before launch |
| MEDIUM-3: Unbounded OIDC state store | Open -- add 10K entry cap (Section 12.3) | Must fix before launch |
| LOW-3: Session refresh interval | Open -- align to 66% of JWT expiration (Section 5.1) | Should fix before launch |

All other findings (HIGH-2, MEDIUM-1, MEDIUM-4, MEDIUM-5, MEDIUM-6, LOW-1, LOW-4) already fixed.

---

## Appendix A: Security Checklist for Development

Before any feature is merged, verify:

- [ ] All database queries filter by `org_id` (IDOR prevention)
- [ ] Pentester endpoints also check `engagement_assignment` (engagement isolation)
- [ ] All endpoints check authentication (`require_user!`)
- [ ] All endpoints check authorization (`require_role!` with appropriate minimum role)
- [ ] Account type is verified (client vs pentester vs admin)
- [ ] All user inputs are validated (Section 13)
- [ ] Rich text content is sanitized (Section 9.3)
- [ ] No raw SQL; all queries via SeaORM query builder
- [ ] No shell command interpolation with user input
- [ ] Sensitive data is encrypted before storage
- [ ] Audit log entries are created for security-relevant actions
- [ ] Rate limiting is applied to new endpoints
- [ ] Error messages do not leak cross-tenant or cross-engagement information
- [ ] 404 returned for unauthorized resources (not 403)
- [ ] Workflow state transitions are validated
- [ ] New dependencies reviewed with `cargo audit` and `cargo deny`

## Appendix B: Compliance Matrix

| Requirement | GDPR Article | NIS2 Article | Section |
|------------|-------------|-------------|---------|
| Encryption of personal data | Art. 32(1)(a) | Art. 21(2)(e) | 10 |
| Access control | Art. 32(1)(b) | Art. 21(2)(i) | 4, 6, 7 |
| Data integrity | Art. 32(1)(b) | Art. 21(2)(e) | 14 |
| Availability and resilience | Art. 32(1)(b) | Art. 21(2)(c) | 18 |
| Regular testing | Art. 32(1)(d) | Art. 21(2)(f) | 21 |
| Breach notification | Art. 33, 34 | Art. 23 | 15, 16, 18 |
| Data minimization | Art. 5(1)(c) | - | 15 |
| Right to erasure | Art. 17 | - | 15.3 |
| Data portability | Art. 20 | - | 15.4 |
| Privacy by design | Art. 25 | - | 15.5 |
| DPIA | Art. 35 | - | 15.6 |
| DPA | Art. 28 | - | 15.2 |
| Supply chain security | - | Art. 21(2)(d) | 20 |
| Incident handling | Art. 33 | Art. 21(2)(b), 23 | 18 |
| MFA | - | Art. 21(2)(j) | 5.1 |
| Audit and logging | Art. 5(2) | Art. 21(2)(a) | 14 |

## Appendix C: Role-Resource Access Matrix (Complete)

| Resource | Client Owner | Client Admin | Client Member | Client Viewer | Assigned Pentester | Unassigned Pentester | Platform Admin |
|----------|-------------|-------------|--------------|--------------|-------------------|---------------------|----------------|
| Create scope | Yes | Yes | Yes | No | No | No | No |
| Submit scope | Yes | Yes | No | No | No | No | No |
| View own org engagements | Yes | Yes | Yes | Yes (read) | No | No | Yes |
| View engagement details | Yes (own org) | Yes (own org) | Yes (own org) | Yes (own org) | Yes (assigned) | No | Yes |
| Accept/negotiate offer | Yes | Yes | No | No | No | No | No |
| Create offer | No | No | No | No | No | No | Yes |
| Assign pentester | No | No | No | No | No | No | Yes |
| View scope/targets | Yes (own org) | Yes (own org) | Yes (own org) | Yes (own org) | Yes (assigned) | No | Yes |
| Access test credentials | No | No | No | No | Yes (assigned) | No | No |
| Add finding | No | No | No | No | Yes (assigned, active) | No | No |
| Edit finding | No | No | No | No | Yes (creator only) | No | Yes (QA) |
| View findings | Yes (own org) | Yes (own org) | Yes (own org) | Yes (own org, read) | Yes (assigned) | No | Yes |
| Download report | Yes (own org) | Yes (own org) | Yes (own org) | Yes (own org) | Yes (assigned, read) | No | Yes |
| Share report | Yes | Yes | No | No | No | No | Yes |
| Create API key | Yes | Yes | Yes | No | No | No | Yes |
| Manage org members | Yes | Yes | No | No | No | No | Yes |
| View other clients' data | No | No | No | No | No | No | Yes (as needed) |
