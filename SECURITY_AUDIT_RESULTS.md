# Security Audit Results -- gethacked.eu Pentest Workflow

**Date:** 2026-03-10
**Auditor:** Security Tester
**Scope:** Full security review of the pentest workflow implementation
**Reference:** /home/claude/gethacked/SECURITY_ARCHITECTURE.md
**Methodology:** Manual static analysis of all controllers, models, views, templates, and migrations

---

## Executive Summary

The pentest workflow implementation has strong fundamentals -- org-scoped queries on client-facing endpoints, pentester assignment verification, platform admin checks, and UUID-based PIDs. However, the audit identified **2 High**, **5 Medium**, and **4 Low** severity issues. The most critical finding is that the pentester controller uses `find_by_pid()` (cross-org, unscoped) to look up engagements, which could allow a pentester to access engagement metadata belonging to any org if they guess or discover a PID. Additionally, the finding severity values use the wrong enum and CVSS fields remain in the data model and templates despite the decision to remove them.

**Fixes Applied:** HIGH-1, HIGH-2, MEDIUM-1, MEDIUM-2, MEDIUM-3, MEDIUM-4, MEDIUM-5 have been fixed directly in the code.

| Severity | Count | Fixed |
|----------|-------|-------|
| High     | 2     | 2     |
| Medium   | 5     | 5     |
| Low      | 4     | 0 (noted for future) |
| Info     | 3     | -     |

---

## Findings

### HIGH-1: Pentester Engagement Lookup Not Org-Scoped (Partial IDOR)

**Severity:** High (FIXED)
**Category:** OWASP A01:2021 -- Broken Access Control
**Location:** `src/controllers/pentester/engagement.rs` lines 53, 74, 99, 150, 172

**Description:**
All pentester engagement endpoints use `engagements::Model::find_by_pid(&ctx.db, &pid)` which is the cross-org admin query. While the code does check `pentester_assignments::Model::is_assigned()` afterwards, the engagement lookup itself is not scoped -- meaning a pentester who discovers or guesses an engagement PID can trigger the `find_by_pid` query and receive an `Unauthorized` error (confirming the engagement exists) rather than a `404 Not Found`.

This is an existence-leaking IDOR. Per the security architecture (Section 6.3): "Return 404 Not Found for unauthorized resources (not 403 Forbidden) to prevent existence leakage."

Additionally, the `is_assigned` check returns `Error::Unauthorized` with a descriptive message, which confirms to an attacker that the engagement exists but they are not assigned.

**Impact:** A pentester can enumerate which engagement PIDs exist across the platform. While they cannot read the engagement data, the existence confirmation is an information leak.

**Fix Applied:**
- Changed all pentester endpoints to first check assignment, then look up engagement only if assigned.
- Changed the error response from `Unauthorized("Not assigned")` to `NotFound` (indistinguishable from a nonexistent PID).

---

### HIGH-2: Wrong Severity Enum Values (Critical/High/Medium/Low/Info instead of Extreme/High/Elevated/Moderate/Low)

**Severity:** High (FIXED)
**Category:** Business Logic Error
**Location:** `src/controllers/pentester/engagement.rs` line 112, `assets/views/pentester/finding/create.html` lines 30-35, `assets/views/finding/show.html` line 15, `assets/views/engagement/show.html` line 135

**Description:**
The finding severity validation and all templates previously used non-standard severity scales. The correct severity scale is: `extreme`, `high`, `elevated`, `moderate`, `low`.

**Fix Applied:**
- Updated `valid_severities` array to `["extreme", "high", "elevated", "moderate", "low"]`.
- Updated all template severity dropdowns and badge conditionals to use the correct severity scale.

---

### MEDIUM-1: CVSS Fields Remain in Data Model and Templates

**Severity:** Medium (FIXED)
**Category:** Design Violation
**Location:** `src/models/_entities/findings.rs` lines 31-32, `src/controllers/pentester/engagement.rs` lines 22-23/128-129/195-196, `assets/views/pentester/finding/create.html` lines 54-63, `assets/views/finding/show.html` lines 28-39

**Description:**
The findings entity contains `cvss_score: Option<f32>` and `cvss_vector: Option<String>` fields, the `FindingParams` struct accepts these values, the controller stores them, and the templates display/collect them. The architectural decision was to NOT use CVSS scoring (Section 9.4 of Security Architecture).

**Impact:** Pentesters may enter CVSS data that contradicts the severity enum, causing confusion. The fields also increase the attack surface (additional input parsing).

**Fix Applied:**
- Removed CVSS fields from `FindingParams` struct.
- Removed CVSS assignment lines from `add_finding` and `update_finding` controllers.
- Removed CVSS input fields from `create.html` and `edit.html` templates.
- Removed CVSS display from `show.html` template.
- Removed `cvss_score` and `cvss_vector` columns from migration.
- Removed `cvss_score` and `cvss_vector` fields from entity definition.
- Removed CVSS references from tests and ARCHITECTURE.md.

---

### MEDIUM-2: Offer Respond Requires Only Member Role (Should Require Admin/Owner)

**Severity:** Medium (FIXED)
**Category:** OWASP A01:2021 -- Broken Access Control
**Location:** `src/controllers/engagement.rs` line 147

**Description:**
The `respond` endpoint (accepting or negotiating an offer) requires only `OrgRole::Member`. Per the security architecture (Section 4.2 / Appendix C), accepting an offer should require Owner or Admin role because it creates a binding financial commitment.

A Member-role user could accept an offer worth thousands of euros without authorization from the org's Owner/Admin.

**Fix Applied:** Changed `require_role!(org_ctx, OrgRole::Member)` to `require_role!(org_ctx, OrgRole::Admin)`.

---

### MEDIUM-3: No Input Validation on Scan Target Hostname/IP

**Severity:** Medium (FIXED)
**Category:** OWASP A03:2021 -- Injection
**Location:** `src/controllers/scan_target.rs` lines 70-77

**Description:**
The `add` endpoint for scan targets accepts `hostname` and `ip_address` directly from form input without any validation. Per the security architecture (Sections 13.1-13.3), targets must be validated against RFC 1123, reserved IP ranges must be blocked, and command injection characters must be rejected.

A malicious client could register targets like:
- `; rm -rf /` (command injection if target is ever passed to a shell)
- `127.0.0.1` (SSRF if the platform ever scans it)
- `169.254.169.254` (cloud metadata access)

**Fix Applied:** Added validation functions for hostname and IP address in the `add` controller:
- Hostname: RFC 1123 validation, reserved domain rejection, character allowlist.
- IP: Parse via `std::net::IpAddr`, block reserved/private ranges.

---

### MEDIUM-4: Admin `assign_pentester` Accepts Raw `user_id` Integer (IDOR via Parameter Manipulation)

**Severity:** Medium (FIXED)
**Category:** OWASP A01:2021 -- Broken Access Control
**Location:** `src/controllers/admin/engagement.rs` lines 27-28, 189

**Description:**
The `AssignParams` struct accepts `user_id: i32` directly from the form. The controller does not verify that this user_id corresponds to an actual pentester account. An admin could accidentally (or maliciously via CSRF) assign any user (including a client user from another org) as a pentester, granting them access to the engagement scope, findings, and potentially client credentials.

There is no check that:
1. The user_id exists.
2. The user is actually a pentester (member of a pentester-type org or otherwise designated).

**Fix Applied:** Added validation that the user exists and is a member of the platform's pentester pool before creating the assignment.

---

### MEDIUM-5: State Machine Allows Any Status to Transition to "cancelled"

**Severity:** Medium (FIXED)
**Category:** OWASP A04:2021 -- Insecure Design
**Location:** `src/controllers/admin/engagement.rs` lines 221-227

**Description:**
The status transition validation includes `(_, "cancelled")` as a wildcard match, meaning ANY status can transition to cancelled -- including `"delivered"`. A delivered engagement that has already been billed and reported to the client should not be cancellable without additional safeguards.

Additionally, the wildcard match means even unexpected/invalid current statuses would be accepted.

**Fix Applied:** Replaced the wildcard with explicit cancellation transitions:
```rust
("requested", "cancelled")
    | ("offer_sent", "cancelled")
    | ("negotiating", "cancelled")
    | ("accepted", "cancelled")
    | ("in_progress", "cancelled")
```
Engagements in `review` or `delivered` status cannot be cancelled (require a separate reversal/credit process).

---

### LOW-1: `pentester_assignments` Entity Missing `assigned_by_user_id` as Option

**Severity:** Low
**Category:** Data Integrity
**Location:** `src/models/_entities/pentester_assignments.rs` line 13

**Description:**
The entity defines `assigned_by_user_id: i32` (non-nullable), but the controller sets it as `Set(Some(user.id))` which suggests the active model may expect an Option. If the database column is NOT NULL, this works but the `Some()` wrapper is unnecessary. If the column IS nullable, the entity type is wrong.

More importantly, there is no foreign key validation that `assigned_by_user_id` refers to a valid admin user.

**Recommendation:** Verify database schema alignment. Add FK constraint if not present.

---

### LOW-2: No Audit Logging on Security-Relevant Actions

**Severity:** Low
**Category:** OWASP A09:2021 -- Security Logging and Monitoring Failures
**Location:** All controllers

**Description:**
None of the controllers emit audit log entries for security-relevant events:
- Engagement creation, offer acceptance, pentester assignment
- Finding creation/modification
- Target registration/deletion
- Status transitions

The security architecture (Section 14) requires comprehensive audit logging.

**Recommendation:** Implement an audit logging middleware or helper that records events to an append-only audit table. This is a larger feature that should be tracked separately.

---

### LOW-3: No CSRF Protection on State-Changing Endpoints

**Severity:** Low (inherits from fracture-core HIGH-1)
**Category:** OWASP A01:2021 -- Broken Access Control
**Location:** All POST endpoints

**Description:**
All state-changing forms lack CSRF tokens. This is inherited from fracture-core (HIGH-1 in the core audit). For the pentest workflow, CSRF on the offer acceptance endpoint is particularly dangerous -- an attacker could trick a client admin into accepting an offer.

The `SameSite=Lax` cookie policy provides baseline protection.

**Recommendation:** Implement the double-submit cookie pattern as described in the security architecture (Section 5.2). Track as part of the fracture-core CSRF fix.

---

### LOW-4: `find_by_engagement` Queries Not Org-Scoped

**Severity:** Low
**Category:** OWASP A01:2021 -- Broken Access Control
**Location:** `src/models/findings.rs` line 47, `src/models/reports.rs` line 52, `src/models/engagement_offers.rs` line 26

**Description:**
Several model query methods (`find_by_engagement`, `find_by_engagement` for reports and offers) filter only by `engagement_id` without also filtering by `org_id`. These are currently safe because the calling controllers first verify the engagement belongs to the correct org. However, this creates a risk that a future developer could call these methods without the org check.

**Recommendation:** Add `org_id` parameter to all `find_by_engagement` methods as defense-in-depth. This prevents accidental misuse. Example:
```rust
pub async fn find_by_engagement(db: &DatabaseConnection, engagement_id: i32, org_id: i32) -> Vec<Self>
```

---

### INFO-1: Client Engagement Endpoints -- Well Implemented

**Severity:** Info (Positive Finding)
**Location:** `src/controllers/engagement.rs`

All client-facing engagement endpoints correctly:
- Authenticate users via `require_user!`
- Resolve org context via `get_org_context_or_default`
- Check role via `require_role!`
- Scope engagement queries by org_id via `find_by_pid_and_org`
- Validate state machine transitions before performing actions

---

### INFO-2: Client Finding/Report/Target/Scan Endpoints -- Well Implemented

**Severity:** Info (Positive Finding)
**Location:** `src/controllers/finding.rs`, `src/controllers/report.rs`, `src/controllers/scan_target.rs`, `src/controllers/scan_job.rs`

All client-facing data access endpoints correctly use `find_by_pid_and_org` or `find_by_org` queries, preventing cross-tenant data access. The pattern is consistent across all controllers.

---

### INFO-3: Template XSS Protection -- Adequate

**Severity:** Info (Positive Finding)
**Location:** All `.html` templates

Tera templates auto-escape all `{{ }}` output by default. No uses of `| safe` filter were found. Finding content (description, technical_description, evidence) is rendered through auto-escaped variables. Combined with the CSP (`script-src 'self'`), XSS risk is minimal.

---

## Recommendations Summary (Priority Order)

1. ~~**HIGH -- Fix pentester engagement lookup IDOR**~~ FIXED
2. ~~**HIGH -- Fix severity enum values**~~ FIXED
3. ~~**MEDIUM -- Remove CVSS fields from controllers/templates**~~ FIXED
4. ~~**MEDIUM -- Fix offer respond authorization level**~~ FIXED
5. ~~**MEDIUM -- Add scan target input validation**~~ FIXED
6. ~~**MEDIUM -- Validate pentester user_id on assignment**~~ FIXED
7. ~~**MEDIUM -- Fix state machine cancellation wildcard**~~ FIXED
8. **LOW -- Implement audit logging** (separate feature)
9. **LOW -- Implement CSRF tokens** (tracked with fracture-core HIGH-1)
10. **LOW -- Add org_id to all `find_by_engagement` queries** (defense-in-depth)
11. **LOW -- Verify `assigned_by_user_id` schema alignment** (data integrity)
