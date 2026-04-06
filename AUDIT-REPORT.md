# Fracture-PT (GetHacked.eu) — Comprehensive Audit Report

**Date**: 2026-04-05
**Application**: fracture-pt v0.3.0 (v0.5.1 release)
**Stack**: Rust / Loco (Axum) / SeaORM / SQLite / Tera templates / OIDC (Zitadel)
**Codebase**: 101 Rust source files, 54 HTML templates, 635 crate dependencies
**Tested on**: Local dev instance at http://localhost:5150

---

## 1. Executive Summary

Fracture-PT is a well-architected Rust application with strong security foundations: strict CSP headers, Tera auto-escaping (no XSS), SeaORM query builder (no SQL injection), UUID-based public identifiers (no IDOR enumeration), and proper OIDC delegation. The codebase is clean, idiomatic, and follows Loco framework conventions well.

However, the audit uncovered **critical gaps** that must be addressed before production deployment at scale:

1. **No CSRF protection on any form** — the single most impactful security finding
2. **No rate limiting anywhere** — free scan endpoint is weaponizable as a recon proxy
3. **No user data deletion mechanism** — potential GDPR exposure for EU deployment
4. **Dashboard loads all findings into memory** — O(n) memory scaling on every page load
5. **SQLite without WAL mode** — writes block all readers, limiting to ~5-10 concurrent users with writes

The positive security posture (org-scoped access, role enforcement, security headers, input validation) provides a solid foundation. The issues are fixable incrementally without architectural changes.

---

## 2. Top 5 Critical Issues (TL;DR)

### 1. SEC-001 — No CSRF Protection (P0)
Every POST form in the application lacks CSRF tokens. All templates have `nosemgrep` comments suppressing the warning. The `GET /orgs/switch/:pid` endpoint changes org context via GET, exploitable via `<img>` tags. **Fix**: Add CSRF middleware + change org switch to POST.

### 2. PERF-001 + PERF-010 — Dashboard Memory Bomb + No WAL (P0)
`compute_severity()` loads ALL findings into memory on every dashboard visit. Combined with SQLite's default journal mode (writes block readers), this creates a scalability ceiling of ~20 concurrent users. **Fix**: `GROUP BY` aggregate query + `PRAGMA journal_mode=WAL`.

### 3. LEGAL-001 + LEGAL-003 — No Data Erasure + False "No Data Stored" Claim (P0)
No mechanism exists to delete user accounts or their data. The free scan page claims "No Data Stored" but scan results ARE persisted in the database indefinitely. **Fix**: Implement account deletion, correct the claim, add retention policies.

### 4. OPS-003 + SEC-004 — No Rate Limiting (P0)
Zero rate limiting on any endpoint. Free scan (`POST /free-scan`) triggers external crt.sh queries and nmap scans with no throttling. Contact form can flood SMTP. **Fix**: Add tower-governor or similar rate limiting middleware.

### 5. QA-001 — Auth Bypass Information Leak (P1)
Axum's `Form()` extractor runs before controller auth checks, returning 422 with internal field names (`target_type`, `hostname`, `ip_address`, etc.) to unauthenticated users on 12+ endpoints. **Fix**: Move auth to Axum middleware layer, or validate auth before form extraction.

---

## 3. Full Findings Table

### Security (13 findings)

| ID | Sev | Component | Summary |
|----|-----|-----------|---------|
| SEC-001 | P0 | All POST forms | No CSRF protection; org switch via GET |
| SEC-002 | P1 | report_builder.rs | XML injection — user content not escaped in generated XML |
| SEC-003 | P1 | report.rs | Report download uses cross-org lookup, path not validated |
| SEC-004 | P1 | free_scan.rs | No rate limiting on unauthenticated scan endpoint |
| SEC-007 | P1 | .env | JWT_SECRET and OIDC credentials on disk |
| SEC-009 | P1 | org.rs (core) | Org switch via GET — exploitable cross-site |
| SEC-005 | P2 | contact.rs | Email header injection risk — no format validation |
| SEC-006 | P2 | Response headers | `x-powered-by: loco.rs` leaks framework info |
| SEC-008 | P2 | OIDC refresh | JWT refresh requires no re-authentication |
| SEC-010 | P2 | engagement.rs | Internal integer IDs used in form (should be PIDs) |
| SEC-011 | P2 | port_scan.rs | SSRF via DNS rebinding — hostname validated but not resolved IP |
| SEC-012 | P2 | asm.rs | DNS resolution results not filtered for private IPs |
| SEC-013 | P2 | admin/engagement.rs | Engagement status transitions too permissive |

### Code Quality (14 findings)

| ID | Sev | Component | Summary |
|----|-----|-----------|---------|
| CODE-001 | P0 | workers/asm_scan.rs, jobs/asm_scan.rs | Duplicated `create_cert_findings` function |
| CODE-002 | P0 | report_builder.rs | XSS/XML injection — 4 fields not escaped (cross-ref SEC-002) |
| CODE-003 | P1 | scan_target.rs, free_scan.rs, port_scan.rs | Input validation triplicated with inconsistent rules |
| CODE-004 | P1 | pentester/engagement.rs, admin/engagement.rs | N+1 org name resolution |
| CODE-005 | P1 | home.rs | All findings loaded for severity counting |
| CODE-006 | P1 | All models | Silent error swallowing via `unwrap_or_default()` |
| CODE-007 | P1 | All find_by_pid methods | Redundant `Condition::any()` UUID pattern in 9 files |
| CODE-008 | P1 | contact.rs, free_scan.rs | No rate limiting or CAPTCHA on public endpoints |
| CODE-009 | P2 | port_scan.rs | Nmap XML parsing via string matching — fragile |
| CODE-010 | P2 | org_settings.rs | Stale tier check — upgrade doesn't take effect for email alerts |
| CODE-011 | P2 | workers/asm_scan.rs, models/scan_jobs.rs | Dead code: legacy worker and scan_jobs model |
| CODE-012 | P2 | Multiple param structs | No input length limits on form fields |
| CODE-013 | P2 | app.rs | `after_routes` seeds unconditionally in all environments |
| CODE-014 | P2 | report.rs | Report download path not validated against expected prefix |

### Performance (22 findings)

| ID | Sev | Component | Summary |
|----|-----|-----------|---------|
| PERF-001 | P0 | home.rs:86 | Dashboard loads ALL findings for severity counting |
| PERF-002 | P0 | home.rs:173 | N+1: findings count per active engagement |
| PERF-003 | P0 | admin/scan_target.rs:23 | N+1: org name per scan target |
| PERF-004 | P0 | admin/engagement.rs:20 | N+1: org name per engagement |
| PERF-005 | P0 | pentester/engagement.rs:90 | N+1: org name per engagement |
| PERF-006 | P1 | home.rs:107 | ASM summary loads all defs + N+1 run lookups |
| PERF-007 | P1 | home.rs:221 | Dashboard queries sequential (should use tokio::join!) |
| PERF-008 | P1 | middleware.rs | 4-5 DB queries per authenticated request |
| PERF-009 | P1 | All listing endpoints | No pagination — `.all(db)` everywhere |
| PERF-010 | P1 | SQLite config | No WAL mode — writes block all readers |
| PERF-011 | P1 | report_builder.rs | ZIP built entirely in memory |
| PERF-012 | P1 | report.rs | File download reads entire file into memory |
| PERF-013 | P1 | report_builder.rs | Blocking `std::fs` I/O in async context |
| PERF-014 | P2 | scan_target.rs | Target detail loads all job definitions + JSON parse loop |
| PERF-015 | P2 | admin/engagement.rs:130 | Admin loads ALL users for pentester dropdown |
| PERF-016 | P2 | engagement.rs:141 | Engagement loads all org targets for dropdown |
| PERF-017 | P2 | All controllers | Duplicate `find_orgs_for_user` per request |
| PERF-018 | P2 | Migrations | Missing index on job_definitions(org_id, job_type) |
| PERF-019 | P2 | Migrations | Missing index on job_runs(job_definition_id, status) |
| PERF-020 | P2 | free_scan.rs | No rate limiting (cross-ref SEC-004, OPS-003) |
| PERF-021 | P2 | scan_target.rs:77 | Tier check loads all targets instead of COUNT |
| PERF-022 | P2 | engagement.rs:244 | Target ownership check loads all targets |

### DevOps / Observability (18 findings)

| ID | Sev | Component | Summary |
|----|-----|-----------|---------|
| OPS-001 | P0 | Containerfile.dev, compose.yaml | Dev container runs as root with network_mode: host |
| OPS-002 | P0 | .env, .env.dev.backup | Real secrets on disk; backup has unrevoked credentials |
| OPS-003 | P0 | All endpoints | No rate limiting anywhere |
| OPS-004 | P1 | Controllers | No health check endpoint |
| OPS-005 | P1 | Config | No CORS configuration |
| OPS-006 | P1 | production.yaml | `auto_migrate: true` in production |
| OPS-007 | P1 | bin/main.rs | No graceful shutdown — jobs left in "running" state |
| OPS-008 | P1 | compose.yaml | Hardcoded Zitadel master key in dev compose |
| OPS-009 | P1 | Containerfile.prod | nmap in production image increases attack surface |
| OPS-010 | P1 | rust.yml | Ignored RUSTSEC-2023-0071 without documentation |
| OPS-011 | P1 | Config | JWT 15-min expiry with no client-side refresh mechanism |
| OPS-012 | P2 | Controllers | No structured JSON error responses for API consumers |
| OPS-013 | P2 | Config, compose | No backup strategy for SQLite database |
| OPS-014 | P2 | All source | Only 25 tracing calls across 9 files — minimal observability |
| OPS-015 | P2 | Compose files | No container resource limits |
| OPS-016 | P2 | Containerfile.dev, compose | Container images use `latest` tags |
| OPS-017 | P2 | Config | No TLS termination documented |
| OPS-018 | P2 | compose.yaml | Hardcoded Postgres passwords in dev compose |

### UX / Accessibility (24 findings)

| ID | Sev | Component | Summary |
|----|-----|-----------|---------|
| UX-001 | P0 | /admin | No admin dashboard or navigation hub |
| UX-002 | P0 | All list views | Clickable table rows not keyboard-accessible (A11Y) |
| UX-003 | P1 | All forms | No CSRF tokens (cross-ref SEC-001) |
| UX-004 | P1 | Delete handlers | No error feedback; inconsistent patterns |
| UX-005 | P1 | subscription, scan_job, admin lists | Internal IDs shown instead of human-readable names |
| UX-006 | P1 | All form pages | No form validation error display mechanism |
| UX-007 | P1 | app.js:52 | Session expiry replaces entire page with plain text |
| UX-008 | P1 | org/members.html | Member removal lacks confirmation; role change may not work |
| UX-009 | P2 | home, pricing templates | Pricing inconsistencies (2hrs vs 4hrs manual testing) |
| UX-010 | P2 | report/show.html | Duplicate action areas |
| UX-011 | P2 | engagement/request.html | No required field indicators |
| UX-012 | P2 | scan_target/create.html | Form doesn't adapt fields to target type |
| UX-013 | P2 | base.html nav | Blog link goes nowhere |
| A11Y-001 | P0 | All `data-href` rows | No keyboard nav — WCAG 2.1.1 fail |
| A11Y-003 | P1 | Org switcher | Missing `role="menu"`, `aria-haspopup` |
| A11Y-004 | P1 | Severity badges | Color-only differentiation (partially mitigated by text) |
| A11Y-006 | P1 | Multiple templates | Card `<header>` used instead of heading elements |
| A11Y-008 | P2 | All templates | Decorative SVGs missing `aria-hidden="true"` |
| A11Y-009 | P2 | scope/wizard.html | Range slider has no accessible label |
| A11Y-011 | P2 | Custom elements | Focus indicators rely on browser defaults |

### QA / Functional (6 findings)

| ID | Sev | Component | Summary |
|----|-----|-----------|---------|
| QA-001 | P1 | All POST endpoints | Form field names leaked before auth check (422 responses) |
| QA-002 | P2 | All routes | Trailing slashes return 404 instead of redirect |
| QA-003 | P2 | /static/ | Directory listing returns 200 with 404 content |
| QA-004 | P2 | /api/contact | Contact form accepts empty required fields |
| QA-005 | P2 | free_scan.rs | `Secure` cookie flag breaks HTTP dev environment |
| QA-006 | P2 | All error paths | Inconsistent error response formats |

### Legal / Privacy (15 findings)

| ID | Sev | Component | Summary |
|----|-----|-----------|---------|
| LEGAL-001 | P0 | User model | No account deletion or data erasure mechanism |
| LEGAL-002 | P0 | All data | No retention policy — indefinite storage |
| LEGAL-003 | P0 | free_scan.rs | Scans arbitrary domains without authorization; "No Data Stored" is false |
| LEGAL-004 | P1 | privacy.html | Privacy policy missing ~15 required elements |
| LEGAL-005 | P1 | Cookie handling | No cookie consent mechanism (may be OK if all essential) |
| LEGAL-006 | P1 | imprint.html | Imprint missing KVK, VAT, registered address |
| LEGAL-007 | P1 | scan_target.rs | Port scans run without verifying domain ownership |
| LEGAL-008 | P1 | terms.html | ToS lacks pentesting-specific disclaimers |
| LEGAL-009 | P2 | Footer | AGPL-3.0 source availability unclear for network users |
| LEGAL-010 | P2 | Contact form | No CSRF protection (cross-ref SEC-001) |
| LEGAL-011 | P2 | scan_alert mailer | Infrastructure data sent unprotected over SMTP |
| LEGAL-012 | P2 | ToS | No DPA template for client organizations |
| LEGAL-013 | P2 | Subscriptions | Stripe not mentioned in privacy policy |
| LEGAL-014 | P2 | OIDC cookies | Full ID token stored in cookie (PII client-side) |
| LEGAL-015 | P2 | free_scan template | "No Data Stored" badge is factually inaccurate |

---

## 4. Threat Model & Attack Surface Map

### Data Flows

```
INTERNET (untrusted)
  |
  |-- [Guest] --POST /free-scan--> [App] --HTTPS--> [crt.sh] (cert transparency)
  |                                  |---DNS--> [System resolver] (subdomain resolution)
  |                                  |---nmap--> [Target host] (port scan)
  |                                  |---DB--> [SQLite] (results persisted)
  |
  |-- [Guest] --POST /api/contact--> [App] --SMTP--> [Mail server]
  |
  |-- [Browser] --OIDC--> [Zitadel IdP] --callback--> [App] --SET-COOKIE jwt,id_token,org_pid
  |
  |-- [Auth User] --CRUD--> [App] --DB--> [SQLite] (org-scoped)
  |                           |---nmap--> [Target] (port scan, no ownership verification)
  |                           |---SMTP--> [Mail] (scan alerts with infra data)
  |
  |-- [Pentester] --findings/reports--> [App] --DB + FS--> [SQLite + /app/data/reports/]
  |
  |-- [Admin] --cross-org access--> [App] --DB--> [SQLite] (all orgs)
```

### Trust Boundaries

| Boundary | From | To | Controls |
|----------|------|----|----------|
| Internet to App | Unauthenticated | Public endpoints | Input validation, CSP, no auth required |
| App to OIDC | App | Zitadel | PKCE, nonce, state, audience validation |
| Auth to Org Data | Authenticated user | Org-scoped data | JWT cookie, org_pid cookie, org membership check |
| Org to Org | Org A member | Org B data | `find_by_pid_and_org()` pattern (well-enforced) |
| User to Admin | Regular user | Admin routes | `require_platform_admin!` macro |
| App to External | App | crt.sh, DNS, nmap targets | Hostname validation (but no DNS rebinding protection) |

### Attacker Capabilities

| Attacker | Capabilities | Key Attack Vectors |
|----------|-------------|-------------------|
| Unauthenticated | Free scan, contact form, public pages | Scan abuse (recon proxy), SMTP flooding, CSRF on authenticated users |
| Org Member | CRUD within own org | CSRF to create engagements, escalate via org switch |
| Cross-Org Member | Member of multiple orgs | CSRF org switch + action chain |
| Pentester | Findings/reports for assigned engagements | XML injection in findings → XXE in report builder |
| Compromised JWT | Valid token holder | Infinite refresh, cross-org if multi-org member |

### Attack Scenarios

1. **CSRF Org Switch Chain**: `<img src="/orgs/switch/{orgB}">` forces victim to OrgB, then auto-submit form creates engagement in wrong org
2. **Free Scan Weaponization**: Script submits hundreds of scans → platform becomes recon proxy against arbitrary domains
3. **XML Injection → XXE**: Pentester injects XML entities in finding description → report ZIP contains malicious XML → downstream docbuilder processes XXE
4. **DNS Rebinding SSRF**: Attacker-controlled domain alternates between public IP and 169.254.169.254 → nmap scans cloud metadata
5. **Session Persistence**: Stolen JWT refreshed indefinitely via `GET /api/auth/oidc/refresh` without re-authentication

---

## 5. Load Testing Appendix

### Scalability Assessment

| Metric | Current Limit | After Quick Fixes | With PostgreSQL |
|--------|--------------|-------------------|-----------------|
| Concurrent browsing users | ~20-30 | ~80-100 | 500+ |
| Concurrent writers | ~5-10 | ~30-50 (WAL) | 200+ |
| Findings per org (dashboard) | ~500 | 10,000+ | 100,000+ |
| Total findings (admin) | ~1,000 | Needs pagination | Needs pagination |
| Background scans concurrent | ~3-5 | ~10-15 | ~50+ |

### Quick Fixes (days)

1. `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;` on connection init
2. Replace `compute_severity()` with `GROUP BY` aggregate
3. `tokio::join!()` for dashboard queries
4. Add composite indexes: `job_definitions(org_id, job_type)`, `job_runs(job_definition_id, status)`, `findings(org_id)`

### Medium-Term Fixes (weeks)

5. Pagination for all listing endpoints (cursor-based, default 50)
6. Fix all N+1 patterns with JOINs or batch HashMap lookups
7. In-memory LRU cache for middleware auth context (5s TTL)
8. Rate limiting on free scan and contact form
9. Replace blocking `std::fs` with `tokio::fs` in report builder

### Load Test Profiles

| Profile | Users | Duration | Key Metrics |
|---------|-------|----------|-------------|
| Baseline | 10 concurrent | 5 min | Dashboard p95 <500ms, no errors |
| Medium | 50 concurrent + free scans | 10 min | p95 <2s, write errors <1% |
| Stress | 200 concurrent + 10 bg scans | 15 min | Identify failure point |

---

## 6. Code Review Appendix

### Architecture Assessment

The codebase follows Loco conventions well with clean separation:
- **Controllers** (25 files): HTTP handling, auth checks, form extraction
- **Models** (14 files): SeaORM entities with org-scoped queries
- **Services** (4 files): Business logic (ASM, port scan, reports, tiers)
- **Workers** (3 files): Background job dispatch and scheduling
- **Views** (7 files): Tera template context construction

### Dependency Audit

All 635 crate dependencies use MIT or Apache-2.0 licenses — fully AGPL-3.0 compatible. Core dependencies (loco-rs, sea-orm, axum, tokio) are current versions. `async-trait` could eventually be replaced with native async traits (Rust 1.75+). No unnecessary bloat detected.

### Complexity Hotspots

| Rank | Function | File | Lines | Issue |
|------|----------|------|-------|-------|
| 1 | `job_dispatcher::perform` | workers/job_dispatcher.rs | 101 | Job lifecycle + diff + alerting |
| 2 | `asm::process_results` | services/asm.rs | 105 | Cert dedup + subdomain extraction |
| 3 | `app::seed` | app.rs | 105 | Nested data insertion |
| 4 | `scan_target::add` | controllers/scan_target.rs | 99 | Validation + tier + job dispatch |
| 5 | `AsmScanExecutor::execute` | jobs/asm_scan.rs | 87 | Retry + DNS + port scan fan-out |

### Prioritized Refactor List

| # | Refactor | Effort | Impact |
|---|----------|--------|--------|
| 1 | Extract shared validation module (CODE-003) | 2-3h | High — eliminates security inconsistencies |
| 2 | Fix report builder XML escaping (CODE-002) | 1h | High — prevents XML injection |
| 3 | Fix stale tier check in settings (CODE-010) | 15min | High — functional bug |
| 4 | Add rate limiting to public endpoints (CODE-008) | 2-3h | High — prevents abuse |
| 5 | Replace N+1 queries with JOINs/batch (CODE-004/005) | 2-3h | Medium — performance |
| 6 | Remove dead code (CODE-011) | 30min | Medium — reduces confusion |
| 7 | Propagate DB errors (CODE-006) | 3-4h | Medium — observability |
| 8 | Validate report download path (CODE-014) | 15min | Medium — defense-in-depth |
| 9 | Extract shared find_by_pid helper (CODE-007) | 1-2h | Low — DRY |
| 10 | Add input length limits (CODE-012) | 2h | Medium — resource exhaustion |
| 11 | Replace manual XML parsing with quick-xml (CODE-009) | 2-3h | Low — correctness |
| 12 | Guard seed in production (CODE-013) | 15min | Low — operational hygiene |

### Test Coverage

~2,900 lines across 11 test files covering model operations and authorization. No unit tests within `src/` (no `#[cfg(test)]` modules). Services (asm.rs, port_scan.rs, report_builder.rs) are the most logic-heavy and completely untested.

---

## 7. Production Readiness Checklist

| Item | Status | Finding |
|------|--------|---------|
| CSRF protection | MISSING | SEC-001 |
| Rate limiting | MISSING | SEC-004, OPS-003 |
| Health check endpoint | MISSING | OPS-004 |
| Graceful shutdown | MISSING | OPS-007 |
| Data erasure mechanism | MISSING | LEGAL-001 |
| Data retention policy | MISSING | LEGAL-002 |
| SQLite WAL mode | MISSING | PERF-010 |
| Pagination | MISSING | PERF-009 |
| Audit logging | MISSING | OPS-014 |
| Backup strategy | MISSING | OPS-013 |
| CORS policy | MISSING | OPS-005 |
| Cookie consent review | NEEDS REVIEW | LEGAL-005 |
| Privacy policy completeness | INCOMPLETE | LEGAL-004 |
| TLS documentation | MISSING | OPS-017 |
| Container resource limits | MISSING | OPS-015 |
| Domain ownership verification | NOT ENFORCED | LEGAL-007 |
| Security headers | GOOD | — |
| Org-scoped access control | GOOD | — |
| Role enforcement (RBAC) | GOOD | — |
| OIDC implementation | GOOD | — |
| Input validation (domains/IPs) | GOOD | — |
| Tera auto-escaping (XSS) | GOOD | — |
| SeaORM (SQL injection) | GOOD | — |
| UUID PIDs (IDOR prevention) | GOOD | — |
| Non-root prod container | GOOD | — |
| CI pipeline (fmt/clippy/audit) | GOOD | — |

---

## 8. Recommendations & Next Steps

### Immediate (before next production deployment)

1. **Add CSRF middleware** — This is the single highest-impact security fix. Use `axum_csrf` or implement token-based CSRF. Change `GET /orgs/switch/:pid` to POST.
2. **Enable SQLite WAL mode** — One-line PRAGMA, massive concurrency improvement.
3. **Add rate limiting** — `tower-governor` on `/free-scan` (1/min per IP) and `/api/contact` (5/min per IP).
4. **Fix XML escaping in report builder** — Apply `xml_escape()` to all user-controlled fields.
5. **Correct "No Data Stored" claim** on free scan page.

### Short-term (1-2 weeks)

6. Fix all N+1 query patterns (5 locations) with JOINs or HashMap batch lookups.
7. Replace `compute_severity()` with `GROUP BY` aggregate.
8. Add `tokio::join!()` to dashboard queries.
9. Add missing database indexes.
10. Implement form validation error display in templates.
11. Add keyboard accessibility to `data-href` table rows.
12. Create admin dashboard/navigation hub page.
13. Extract shared input validation module.
14. Set `auto_migrate: false` in production config.

### Medium-term (1-2 months)

15. Implement pagination for all listing endpoints.
16. Add user account deletion / data erasure mechanism.
17. Define and implement data retention policies.
18. Complete privacy policy with all required elements.
19. Add health check endpoint (`/health`).
20. Implement graceful shutdown with job cleanup.
21. Add audit logging for security-sensitive operations.
22. Enforce domain ownership verification before port scans.
23. Add CORS configuration.
24. Consult legal counsel on the 15 open questions identified.

### Long-term

25. Migrate to PostgreSQL for concurrent write scalability.
26. Add structured metrics endpoint (Prometheus).
27. Add distributed tracing (OpenTelemetry).
28. Move nmap to a separate sidecar container.
29. Implement container image scanning in CI.
30. Add integration and end-to-end test suites.

---

## 9. Positive Findings (What's Working Well)

The audit is not all problems. These are genuinely strong:

- **Org-scoped data isolation**: `find_by_pid_and_org()` pattern consistently enforced across all client controllers. Cross-org IDOR is well-mitigated.
- **OIDC implementation**: PKCE, nonce verification, CSRF state with TTL, audience checks, backchannel logout.
- **Security headers**: CSP (`default-src 'none'`), HSTS (2-year), X-Frame-Options DENY, X-Content-Type-Options nosniff.
- **No SQL injection surface**: All queries via SeaORM query builder.
- **No XSS surface**: Tera auto-escaping confirmed; no `|safe` filters found. XSS payloads reflected safely.
- **UUID public identifiers**: No sequential IDOR enumeration possible.
- **Role hierarchy enforcement**: `require_role!` and `require_platform_admin!` consistently applied.
- **Clean dependency set**: Lean, well-chosen crates, all license-compatible.
- **CI pipeline**: Comprehensive (rustfmt, clippy pedantic, semgrep SAST, cargo-audit, tests).
- **Guest landing page**: Excellent UX — rated 9/10 by UX reviewer.
- **SRI integrity hashes**: All static assets protected.
- **Non-root production container**: Multi-stage build with dedicated appuser.
- **Skip link and form labels**: Accessibility basics done right.

---

*Report generated by 7 specialized audit agents coordinated by Claude Code.*
*Agents: Security Lead, Code Reviewer, QA Tester, UX/A11y Reviewer, DevOps Reviewer, Legal/Privacy Reviewer, Performance Engineer.*
