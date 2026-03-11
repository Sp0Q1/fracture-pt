# gethacked.eu -- Test Strategy & Acceptance Criteria

## 1. Overview

This document defines the complete test strategy for gethacked.eu, an EU-focused pentesting
and security services portal built on fracture-cms / fracture-core.

All tests follow fracture-cms conventions:
- **Framework**: `rstest` for parameterized tests, `insta` (with redactions + YAML) for
  snapshot testing, `serial_test` for DB-dependent tests
- **Runtime**: `#[tokio::test]` + `#[serial]` for all integration/model tests
- **DB lifecycle**: `boot_test::<App>()` boots a fresh test DB (migrations run automatically);
  `truncate()` cleans between tests
- **Containers**: All tests run inside podman containers exclusively -- no local Postgres
- **Test location**: `tests/` directory at workspace root, mirroring `tests/models/`,
  `tests/requests/`, `tests/tasks/`, `tests/workers/`

---

## 2. Role Model & Actor Definitions

The platform has three distinct actor types, mapped onto fracture-core's RBAC:

| Actor       | fracture-core Role   | Permissions                                                         |
|-------------|----------------------|---------------------------------------------------------------------|
| **Client**  | org Owner / Member   | Define pentest scope, view offers, accept/negotiate, view own findings & reports |
| **Pentester** | (special assignment) | Access ONLY assigned engagements, add findings, update engagement status |
| **Admin**   | platform-level admin | Review scope requests, create offers, assign pentesters, manage all engagements |

**Key constraint**: Pentesters MUST NOT see any client data beyond their assigned engagements.
Admins MUST NOT be modeled as org members of client orgs (they operate at platform level).

---

## 3. Pentest Workflow Phases

The core business flow is a 5-phase process. Every phase has distinct actors and test
requirements.

```
Phase 1: Scope Definition (Client)
  |
  v
Phase 2: Offer / Quote (Admin)
  |
  v
Phase 3: Acceptance (Client)
  |
  v
Phase 4: Pentest Execution (Pentester)
  |
  v
Phase 5: Report Delivery (Client views)
```

### Status Model for Pentest Requests / Engagements

```
pentest_requests:
  draft -> submitted -> under_review -> offer_sent -> accepted -> rejected -> withdrawn

engagements (created on acceptance):
  created -> pentester_assigned -> in_progress -> findings_review -> completed -> archived
```

---

## 4. New Domain Models

gethacked.eu introduces the following models beyond fracture-core's base (users, organizations,
org_members, org_invites, projects, notes):

| Model                  | Purpose                                                              |
|------------------------|----------------------------------------------------------------------|
| `services`             | Pentesting service catalog (web app, API, infra, mobile, etc.)       |
| `pricing_tiers`        | Subscription tiers (starter, professional, enterprise)               |
| `subscriptions`        | Org-to-tier binding with start/end dates and status                  |
| `pentest_requests`     | Client-submitted scope definition (targets, IP ranges, windows, exclusions, contact) |
| `offers`               | Admin-created quote for a pentest request (pricing, timeline, deliverables) |
| `engagements`          | Active pentest engagement (created when client accepts offer)        |
| `pentester_assignments`| Links pentester users to specific engagements                        |
| `findings`             | Vulnerability findings within an engagement (rich structured data)   |
| `reports`              | Generated reports (PDF) tied to an engagement                        |
| `domains`              | Domains/IPs owned by an org (scope targets)                          |
| `domain_verifications` | DNS TXT or HTTP-meta verification records                            |

---

## 5. Unit Test Plan

### 5.1 `tests/models/services.rs`

| Test                                       | Acceptance Criteria                                              |
|--------------------------------------------|------------------------------------------------------------------|
| `test_create_service`                      | Service created with name, slug, description, category           |
| `test_find_service_by_slug`                | Lookup by slug returns correct service                           |
| `test_find_active_services`                | Only services with `active=true` returned                        |
| `test_service_sets_pid_on_insert`          | UUID pid auto-assigned before insert                             |
| `test_service_slug_unique_constraint`      | Duplicate slug rejected at DB level                              |
| `test_service_deactivation`                | Setting `active=false` excludes from active listing              |

### 5.2 `tests/models/pricing_tiers.rs`

| Test                                       | Acceptance Criteria                                              |
|--------------------------------------------|------------------------------------------------------------------|
| `test_create_pricing_tier`                 | Tier with name, monthly_price_cents, annual_price_cents, features JSON |
| `test_find_all_tiers_ordered_by_price`     | Tiers returned sorted ascending by monthly_price_cents           |
| `test_find_tier_by_slug`                   | Lookup by slug returns correct tier                              |
| `test_tier_slug_unique`                    | Duplicate slug rejected                                          |
| `test_tier_sets_pid_on_insert`             | UUID pid auto-assigned                                           |
| `test_tier_with_zero_price_is_free`        | Free tier (price=0) created successfully                         |

### 5.3 `tests/models/subscriptions.rs`

| Test                                       | Acceptance Criteria                                              |
|--------------------------------------------|------------------------------------------------------------------|
| `test_create_subscription`                 | Links org_id to pricing_tier_id with status=active               |
| `test_find_active_subscription_for_org`    | Returns current active sub for org, None if none                 |
| `test_subscription_expiry`                 | Sub with `ends_at` in past marked as expired                     |
| `test_upgrade_subscription`               | Changing tier_id succeeds; old sub deactivated                   |
| `test_cancel_subscription`                 | Status set to cancelled, end_date set                            |
| `test_org_can_only_have_one_active_sub`    | Creating second active sub for same org fails or deactivates old |
| `test_subscription_requires_valid_org`     | FK constraint: nonexistent org_id rejected                       |
| `test_subscription_requires_valid_tier`    | FK constraint: nonexistent tier_id rejected                      |
| `test_find_subscription_with_tier_details` | Join query returns sub + tier info                               |

### 5.4 `tests/models/pentest_requests.rs`

| Test                                            | Acceptance Criteria                                          |
|-------------------------------------------------|--------------------------------------------------------------|
| `test_create_pentest_request`                   | Request created with org_id, service_id, status=draft        |
| `test_pentest_request_scope_fields`             | target_systems, ip_ranges, domains, testing_windows, exclusions, contact_info all stored |
| `test_submit_draft_request`                     | draft -> submitted transition works                          |
| `test_cannot_submit_empty_scope`                | Submission fails if required scope fields missing            |
| `test_find_requests_by_org`                     | Only org's own requests returned, ordered by created_at DESC |
| `test_pentest_request_status_transitions`       | draft -> submitted -> under_review -> offer_sent             |
| `test_withdraw_request`                         | submitted/under_review -> withdrawn                          |
| `test_cannot_modify_submitted_request`          | Fields locked after submission (except withdraw)             |
| `test_pentest_request_sets_pid_on_insert`       | UUID pid auto-assigned                                       |
| `test_pentest_request_cross_org_isolation`      | Org A cannot see Org B's requests                            |
| `test_pentest_request_requires_valid_org`       | FK constraint on org_id                                      |
| `test_pentest_request_requires_valid_service`   | FK constraint on service_id                                  |
| `test_find_request_by_pid_and_org`              | Scoped lookup by pid + org_id                                |

### 5.5 `tests/models/offers.rs`

| Test                                            | Acceptance Criteria                                          |
|-------------------------------------------------|--------------------------------------------------------------|
| `test_create_offer`                             | Offer created with pentest_request_id, price_cents, timeline_days, deliverables, status=pending |
| `test_offer_links_to_pentest_request`           | FK constraint: valid pentest_request_id required             |
| `test_find_offer_by_request`                    | Lookup by pentest_request_id returns correct offer           |
| `test_offer_sets_pid_on_insert`                 | UUID pid auto-assigned                                       |
| `test_offer_status_transitions`                 | pending -> accepted / rejected / expired                     |
| `test_accept_offer_creates_engagement`          | Accepting an offer triggers engagement creation              |
| `test_reject_offer`                             | Client rejects; request returns to under_review for re-quote |
| `test_offer_expiry`                             | Offer with expires_at in past cannot be accepted             |
| `test_only_one_active_offer_per_request`        | Cannot create second offer if one is pending                 |
| `test_offer_visible_to_client_org`              | Client org can view offer for their request                  |
| `test_offer_not_visible_to_other_orgs`          | Other orgs cannot see the offer                              |

### 5.6 `tests/models/engagements.rs`

| Test                                            | Acceptance Criteria                                          |
|-------------------------------------------------|--------------------------------------------------------------|
| `test_create_engagement_from_accepted_offer`    | Engagement created with org_id, service_id, offer_id, status=created |
| `test_find_engagements_by_org`                  | Only org's own engagements returned, ordered by created_at DESC |
| `test_engagement_status_transitions`            | created -> pentester_assigned -> in_progress -> findings_review -> completed |
| `test_invalid_status_transition`                | completed -> created rejected                                |
| `test_engagement_sets_pid_on_insert`            | UUID pid auto-assigned                                       |
| `test_engagement_cross_org_isolation`           | Org A cannot see Org B's engagements                         |
| `test_engagement_requires_valid_org`            | FK constraint on org_id                                      |
| `test_engagement_requires_valid_service`        | FK constraint on service_id                                  |
| `test_find_engagement_by_pid_and_org`           | Scoped lookup by pid + org_id                                |
| `test_engagement_archives`                      | completed -> archived transition works                       |
| `test_engagement_scope_inherited_from_request`  | Engagement retains scope details from original request       |

### 5.7 `tests/models/pentester_assignments.rs`

| Test                                            | Acceptance Criteria                                          |
|-------------------------------------------------|--------------------------------------------------------------|
| `test_assign_pentester_to_engagement`           | Assignment created with engagement_id, user_id, assigned_at  |
| `test_find_assignments_by_engagement`           | All pentesters for an engagement returned                    |
| `test_find_engagements_for_pentester`           | All engagements a pentester is assigned to                   |
| `test_cannot_assign_same_pentester_twice`       | Duplicate assignment rejected (unique constraint)            |
| `test_remove_pentester_assignment`              | Assignment can be removed                                    |
| `test_assignment_requires_valid_engagement`     | FK constraint on engagement_id                               |
| `test_assignment_requires_valid_user`           | FK constraint on user_id                                     |
| `test_pentester_sees_only_assigned_engagements` | Pentester cannot query non-assigned engagements              |

### 5.8 `tests/models/findings.rs`

| Test                                            | Acceptance Criteria                                          |
|-------------------------------------------------|--------------------------------------------------------------|
| `test_create_finding`                           | Finding created with all required structured fields          |
| `test_finding_required_fields`                  | description, technical_description, impact, recommendation all required |
| `test_finding_severity_values`                  | extreme, high, elevated, moderate, low all accepted          |
| `test_finding_invalid_severity_rejected`        | Invalid severity string rejected at model level              |
| `test_finding_category_values`                  | injection, broken_auth, xss, idor, misconfig, etc. all accepted |
| `test_finding_invalid_category_rejected`        | Unrecognized category value rejected                         |
| `test_find_findings_by_engagement`              | All findings for an engagement returned, ordered by severity DESC |
| `test_find_findings_by_engagement_and_category` | Filter findings by category within engagement                |
| `test_finding_sets_pid_on_insert`               | UUID pid auto-assigned                                       |
| `test_finding_cross_org_isolation`              | Findings scoped through engagement->org; cross-org lookup fails |
| `test_finding_requires_valid_engagement`        | FK constraint on engagement_id                               |
| `test_finding_created_by_pentester`             | created_by_user_id tracks which pentester added it           |
| `test_finding_update_preserves_audit_trail`     | updated_at timestamp changes on modification                 |
| `test_finding_with_all_fields_populated`        | Snapshot test of a fully populated finding                   |

### 5.9 `tests/models/reports.rs`

| Test                                       | Acceptance Criteria                                              |
|--------------------------------------------|------------------------------------------------------------------|
| `test_create_report`                       | Report created with engagement_id, format, status=generating     |
| `test_find_reports_by_engagement`          | All reports for an engagement returned                           |
| `test_report_status_transitions`           | generating -> ready / failed                                     |
| `test_report_sets_pid_on_insert`           | UUID pid auto-assigned                                           |
| `test_report_cross_org_isolation`          | Reports scoped through engagement->org; cross-org lookup fails   |
| `test_report_download_url_set_when_ready`  | download_url populated only after status=ready                   |
| `test_report_requires_valid_engagement`    | FK constraint on engagement_id                                   |
| `test_report_includes_all_findings`        | Report generation includes all findings from engagement          |

### 5.10 `tests/models/domains.rs`

| Test                                       | Acceptance Criteria                                              |
|--------------------------------------------|------------------------------------------------------------------|
| `test_create_domain`                       | Domain created with org_id, fqdn, verified=false                 |
| `test_find_domains_by_org`                 | Only org's own domains returned                                  |
| `test_domain_fqdn_unique_per_org`          | Same FQDN in same org rejected; different org allowed            |
| `test_domain_sets_pid_on_insert`           | UUID pid auto-assigned                                           |
| `test_domain_cross_org_isolation`          | Org A cannot see Org B's domains                                 |
| `test_mark_domain_verified`                | verified=true, verified_at timestamp set                         |
| `test_domain_requires_valid_org`           | FK constraint on org_id                                          |

### 5.11 `tests/models/domain_verifications.rs`

| Test                                       | Acceptance Criteria                                              |
|--------------------------------------------|------------------------------------------------------------------|
| `test_create_verification_record`          | Record with domain_id, method (dns_txt/http_meta), token, status |
| `test_verification_token_generation`       | Token is unique, sufficiently random (>= 32 chars)               |
| `test_verification_expiry`                 | Verification tokens expire after configured TTL                  |
| `test_mark_verification_complete`          | status=verified, marks parent domain as verified                 |
| `test_verification_requires_valid_domain`  | FK constraint on domain_id                                       |

---

## 6. Integration Test Plan

### 6.1 Full Pentest Workflow: Scope -> Offer -> Accept -> Execute -> Report

File: `tests/requests/pentest_workflow.rs`

```
test_full_pentest_lifecycle:
  1.  Client creates user via OIDC (find_or_create_from_oidc)
  2.  Verify personal org auto-created
  3.  Client subscribes org to "professional" tier
  4.  Client adds domain to org and verifies it
  5.  Client creates a pentest_request (scope definition):
      - target_systems: ["api.example.com", "app.example.com"]
      - ip_ranges: ["203.0.113.0/24"]
      - testing_windows: "Mon-Fri 02:00-06:00 CET"
      - exclusions: ["billing.example.com"]
      - contact_info: {name: "Jane CTO", email: "jane@example.com", phone: "..."}
  6.  Client submits the request (draft -> submitted)
  7.  Admin reviews request (submitted -> under_review)
  8.  Admin creates an offer (pricing, timeline, deliverables)
  9.  Client views the offer in-app
  10. Client accepts the offer -> engagement auto-created
  11. Admin assigns pentester to engagement
  12. Pentester starts engagement (pentester_assigned -> in_progress)
  13. Pentester adds findings:
      - Finding 1: SQL Injection (severity: extreme, category: injection)
      - Finding 2: Missing CSP Header (severity: low, category: misconfig)
      - Finding 3: IDOR on /api/users/{id} (severity: high, category: idor)
  14. Pentester marks engagement as findings_review
  15. Admin reviews, marks completed
  16. Report generated from findings
  17. Client downloads report
  18. Verify all data scoped to org throughout
  19. Verify pentester can no longer access engagement data after completion (optional)
```

### 6.2 Offer Negotiation Flow

File: `tests/requests/offer_flow.rs`

| Test                                          | What it covers                                              |
|-----------------------------------------------|-------------------------------------------------------------|
| `test_client_rejects_offer`                   | Client rejects; admin can create new offer                  |
| `test_offer_expires_before_acceptance`        | Expired offer cannot be accepted; request returns to review |
| `test_multiple_offer_rounds`                  | Reject -> new offer -> accept works correctly               |
| `test_client_withdraws_after_offer_sent`      | Client can withdraw request even with pending offer         |

### 6.3 Subscription & Billing Flow

File: `tests/requests/subscription_flow.rs`

| Test                                          | What it covers                                              |
|-----------------------------------------------|-------------------------------------------------------------|
| `test_subscribe_to_free_tier`                 | Org subscribes to free tier                                 |
| `test_upgrade_from_free_to_professional`      | Tier upgrade, old sub deactivated, new quota applied         |
| `test_downgrade_tier`                         | Tier downgrade; active engagements handled gracefully        |
| `test_expired_subscription_blocks_new_requests`| Cannot create pentest requests when sub is expired          |
| `test_cancelled_subscription_blocks_requests` | Cannot create pentest requests when sub is cancelled         |

---

## 7. Security Tests (CRITICAL)

This section is the most important for gethacked.eu. As a security company's app, it must
be bulletproof against the vulnerabilities it tests for in clients.

### 7.1 IDOR (Insecure Direct Object Reference) Tests

File: `tests/requests/security_idor.rs`

**Principle**: Every resource access must verify BOTH authentication AND authorization
(org membership + role). No endpoint should return data by guessing IDs/PIDs alone.

| Test                                                | Acceptance Criteria                                      |
|-----------------------------------------------------|----------------------------------------------------------|
| `test_idor_pentest_request_by_pid`                  | GET /requests/{pid} with valid pid but wrong org -> 403  |
| `test_idor_offer_by_pid`                            | GET /offers/{pid} with valid pid but wrong org -> 403    |
| `test_idor_engagement_by_pid`                       | GET /engagements/{pid} with valid pid but wrong org -> 403|
| `test_idor_finding_by_pid`                          | GET /findings/{pid} with valid pid but wrong org -> 403  |
| `test_idor_report_by_pid`                           | GET /reports/{pid} with valid pid but wrong org -> 403   |
| `test_idor_report_download_by_pid`                  | GET /reports/{pid}/download with wrong org -> 403        |
| `test_idor_domain_by_pid`                           | GET /domains/{pid} with valid pid but wrong org -> 403   |
| `test_idor_subscription_by_pid`                     | GET /subscriptions/{pid} with wrong org -> 403           |
| `test_idor_sequential_id_enumeration`               | Incrementing numeric IDs in URL does not leak data       |
| `test_idor_uuid_guessing`                           | Random valid UUID format returns 404, not data from another org |
| `test_idor_modify_other_orgs_pentest_request`       | PUT /requests/{pid} for another org's request -> 403     |
| `test_idor_delete_other_orgs_domain`                | DELETE /domains/{pid} for another org's domain -> 403    |
| `test_idor_add_finding_to_other_orgs_engagement`    | POST /engagements/{pid}/findings for wrong engagement -> 403 |
| `test_idor_accept_other_orgs_offer`                 | POST /offers/{pid}/accept for another org's offer -> 403 |

### 7.2 Privilege Escalation Tests

File: `tests/requests/security_privesc.rs`

| Test                                                       | Acceptance Criteria                                   |
|------------------------------------------------------------|-------------------------------------------------------|
| `test_client_cannot_assign_pentester`                      | Client role -> POST /engagements/{id}/assign -> 403   |
| `test_client_cannot_create_offer`                          | Client role -> POST /requests/{id}/offer -> 403       |
| `test_client_cannot_add_findings`                          | Client role -> POST /engagements/{id}/findings -> 403 |
| `test_client_cannot_change_engagement_status`              | Client role -> PUT /engagements/{id}/status -> 403    |
| `test_client_cannot_access_admin_endpoints`                | Client -> GET /admin/* -> 403                         |
| `test_pentester_cannot_create_offers`                      | Pentester -> POST /requests/{id}/offer -> 403         |
| `test_pentester_cannot_view_client_org_data`               | Pentester -> GET /orgs/{client_org}/* -> 403          |
| `test_pentester_cannot_view_unassigned_engagements`        | Pentester -> GET unassigned engagement -> 403         |
| `test_pentester_cannot_modify_engagement_status_to_completed` | Pentester -> cannot mark completed (admin only)    |
| `test_pentester_cannot_access_subscription_data`           | Pentester -> GET /subscriptions/* -> 403              |
| `test_pentester_cannot_access_other_pentesters_findings`   | Pentester A -> Pentester B's engagement -> 403        |
| `test_viewer_cannot_submit_pentest_request`                | Viewer role -> POST /requests -> 403                  |
| `test_viewer_cannot_accept_offer`                          | Viewer role -> POST /offers/{id}/accept -> 403        |
| `test_member_cannot_manage_subscription`                   | Member role -> PUT /subscription -> 403               |
| `test_role_manipulation_via_request_body`                  | Sending role=admin in request body does not elevate    |
| `test_self_promotion_to_admin`                             | User cannot change their own role to admin             |

### 7.3 Cross-Tenant Data Leakage Tests

File: `tests/requests/security_tenant_isolation.rs`

| Test                                                       | Acceptance Criteria                                   |
|------------------------------------------------------------|-------------------------------------------------------|
| `test_org_a_cannot_list_org_b_pentest_requests`            | List endpoint returns only own org's data              |
| `test_org_a_cannot_list_org_b_engagements`                 | List endpoint returns only own org's engagements       |
| `test_org_a_cannot_list_org_b_findings`                    | Findings endpoint never includes foreign org data      |
| `test_org_a_cannot_list_org_b_reports`                     | Reports endpoint never includes foreign org data       |
| `test_org_a_cannot_list_org_b_domains`                     | Domains endpoint never includes foreign org data       |
| `test_api_response_never_contains_other_org_ids`           | JSON responses validated to not contain foreign org IDs|
| `test_search_does_not_leak_cross_org_data`                 | Search/filter endpoints respect org scoping            |
| `test_pagination_does_not_leak_cross_org_counts`           | Total count in pagination reflects only own org data   |
| `test_error_messages_do_not_leak_existence`                | 404 returned (not 403) for resources in other orgs to prevent enumeration |
| `test_finding_detail_does_not_leak_pentester_assignment`   | Client cannot determine which pentester is assigned    |

### 7.4 OWASP Top 10 Tests

File: `tests/requests/security_owasp.rs`

| Test                                                 | OWASP Category        | Acceptance Criteria                              |
|------------------------------------------------------|-----------------------|--------------------------------------------------|
| `test_sql_injection_in_scope_fields`                 | A03: Injection        | SQL metacharacters in scope text safely handled  |
| `test_xss_in_finding_description`                    | A03: Injection        | Script tags in finding fields sanitized on output|
| `test_xss_in_pentest_request_scope`                  | A03: Injection        | HTML in scope fields sanitized                   |
| `test_xss_in_domain_fqdn`                            | A03: Injection        | Malicious FQDN input sanitized                   |
| `test_csrf_protection_on_state_changing_endpoints`   | A01: Broken Access    | State-changing POST/PUT/DELETE require CSRF token|
| `test_session_fixation_after_oidc_login`             | A07: Auth Failures    | Session ID regenerated after authentication      |
| `test_session_invalidation_on_logout`                | A07: Auth Failures    | Session fully invalidated, cookie cleared        |
| `test_jwt_tampering_rejected`                        | A07: Auth Failures    | Modified JWT signature -> 401                    |
| `test_jwt_expired_token_rejected`                    | A07: Auth Failures    | Expired JWT -> 401                               |
| `test_jwt_none_algorithm_rejected`                   | A07: Auth Failures    | JWT with alg=none -> 401                         |
| `test_mass_assignment_on_user_creation`              | A08: Software Integrity | Extra fields in POST body ignored (no role escalation) |
| `test_mass_assignment_on_engagement_update`          | A08: Software Integrity | Cannot set org_id via request body              |
| `test_rate_limiting_on_login`                        | A07: Auth Failures    | >N failed attempts -> 429                        |
| `test_rate_limiting_on_api_endpoints`                | A04: Insecure Design  | API rate limiting enforced                       |
| `test_sensitive_data_not_in_url`                     | A02: Crypto Failures  | No tokens/secrets in URL query params            |
| `test_error_responses_no_stack_traces`               | A05: Misconfig        | 500 errors return generic message, no internals  |
| `test_security_headers_present`                      | A05: Misconfig        | X-Frame-Options, CSP, HSTS, X-Content-Type-Options |
| `test_cors_policy_restrictive`                       | A05: Misconfig        | CORS only allows configured origins              |
| `test_file_upload_type_validation`                   | A08: Software Integrity | Report uploads (if any) validate MIME type      |
| `test_path_traversal_in_report_download`             | A01: Broken Access    | ../../../etc/passwd in download path -> rejected |

### 7.5 Pentester-Specific Security Tests

File: `tests/requests/security_pentester.rs`

| Test                                                        | Acceptance Criteria                                  |
|-------------------------------------------------------------|------------------------------------------------------|
| `test_pentester_can_only_add_findings_to_assigned_engagement` | POST /findings for non-assigned engagement -> 403  |
| `test_pentester_can_only_read_assigned_engagement_scope`    | GET engagement scope for non-assigned -> 403         |
| `test_pentester_cannot_modify_scope`                        | PUT scope fields -> 403                              |
| `test_pentester_cannot_view_client_contact_info`            | Contact info excluded from pentester's engagement view|
| `test_pentester_cannot_view_offer_pricing`                  | Pricing/financials excluded from pentester's view    |
| `test_pentester_cannot_list_all_engagements`                | List endpoint returns only assigned engagements      |
| `test_pentester_assignment_checked_on_every_request`        | Removing assignment immediately blocks access        |
| `test_pentester_cannot_reassign_engagement`                 | POST /engagements/{id}/assign -> 403                 |
| `test_pentester_session_scoped_to_assignments`              | Session does not cache stale assignment data         |

---

## 8. Authorization Tests (Comprehensive)

File: `tests/requests/authorization.rs`

### 8.1 Organization Isolation (extends Section 7.3)

| Test                                            | Acceptance Criteria                                          |
|-------------------------------------------------|--------------------------------------------------------------|
| `test_org_a_cannot_view_org_b_pentest_requests` | GET /{org_b}/requests returns 403/404 for org_a user         |
| `test_org_a_cannot_view_org_b_engagements`      | GET /{org_b}/engagements returns 403/404 for org_a user      |
| `test_org_a_cannot_view_org_b_findings`         | GET /{org_b}/findings returns 403/404                        |
| `test_org_a_cannot_download_org_b_reports`      | GET /{org_b}/reports/{id}/download returns 403/404           |
| `test_org_a_cannot_manage_org_b_domains`        | POST/DELETE /{org_b}/domains returns 403/404                 |
| `test_org_a_cannot_modify_org_b_subscription`   | PUT /{org_b}/subscription returns 403/404                    |
| `test_cross_org_request_creation_rejected`      | Creating request referencing another org's domain fails      |

### 8.2 Role Enforcement Matrix

Every endpoint must be tested against every role. Use `rstest` parameterization:

```rust
#[rstest]
#[case::viewer(OrgRole::Viewer, 403)]
#[case::member(OrgRole::Member, 201)]
#[case::admin(OrgRole::Admin, 201)]
#[case::owner(OrgRole::Owner, 201)]
async fn test_create_pentest_request_by_role(
    #[case] role: OrgRole,
    #[case] expected_status: u16,
) { ... }
```

| Endpoint                                | Viewer | Member | Admin | Owner | Pentester (assigned) | Pentester (unassigned) | Unauthenticated |
|-----------------------------------------|--------|--------|-------|-------|----------------------|------------------------|-----------------|
| GET /requests                           | 200    | 200    | 200   | 200   | 403                  | 403                    | 401             |
| POST /requests                          | 403    | 201    | 201   | 201   | 403                  | 403                    | 401             |
| GET /requests/{pid}                     | 200    | 200    | 200   | 200   | 403                  | 403                    | 401             |
| PUT /requests/{pid}                     | 403    | 200    | 200   | 200   | 403                  | 403                    | 401             |
| POST /requests/{pid}/submit            | 403    | 200    | 200   | 200   | 403                  | 403                    | 401             |
| GET /offers/{pid}                       | 200    | 200    | 200   | 200   | 403                  | 403                    | 401             |
| POST /offers/{pid}/accept              | 403    | 403    | 200   | 200   | 403                  | 403                    | 401             |
| POST /offers/{pid}/reject              | 403    | 403    | 200   | 200   | 403                  | 403                    | 401             |
| GET /engagements                        | 200    | 200    | 200   | 200   | 200*                 | 403                    | 401             |
| GET /engagements/{pid}                  | 200    | 200    | 200   | 200   | 200*                 | 403                    | 401             |
| GET /engagements/{pid}/findings         | 200    | 200    | 200   | 200   | 200*                 | 403                    | 401             |
| POST /engagements/{pid}/findings        | 403    | 403    | 403   | 403   | 201*                 | 403                    | 401             |
| GET /reports/{pid}                      | 200    | 200    | 200   | 200   | 403                  | 403                    | 401             |
| GET /reports/{pid}/download             | 200    | 200    | 200   | 200   | 403                  | 403                    | 401             |
| POST /engagements/{pid}/assign          | 403    | 403    | 403   | 403   | 403                  | 403                    | 401             |
| POST /requests/{pid}/offer (admin API)  | 403    | 403    | 403   | 403   | 403                  | 403                    | 401             |
| PUT /subscription                       | 403    | 403    | 200   | 200   | 403                  | 403                    | 401             |

`*` = only for assigned engagements

**Admin-only endpoints** (platform admin, not org admin):

| Endpoint                                | Admin (platform) | All other roles |
|-----------------------------------------|-------------------|-----------------|
| GET /admin/requests                     | 200               | 403             |
| PUT /admin/requests/{pid}/status        | 200               | 403             |
| POST /admin/requests/{pid}/offer        | 201               | 403             |
| POST /admin/engagements/{pid}/assign    | 200               | 403             |
| PUT /admin/engagements/{pid}/status     | 200               | 403             |
| GET /admin/pentesters                   | 200               | 403             |

---

## 9. Landing Page & Pricing Page Tests

File: `tests/requests/pages.rs`

### 9.1 Landing Page

| Test                                    | Acceptance Criteria                                              |
|-----------------------------------------|------------------------------------------------------------------|
| `test_landing_page_renders_200`         | GET / returns 200 for unauthenticated user                       |
| `test_landing_page_contains_hero`       | Response body includes hero section / headline text              |
| `test_landing_page_contains_cta`        | Response body includes call-to-action (signup/login link)        |
| `test_landing_page_seo_meta_tags`       | Response includes og:title, og:description, canonical link       |
| `test_landing_page_eu_compliance`       | Response includes cookie consent banner / GDPR notice            |
| `test_authenticated_user_sees_dashboard`| GET / for authenticated user renders dashboard, not landing page |

### 9.2 Pricing Page

| Test                                    | Acceptance Criteria                                              |
|-----------------------------------------|------------------------------------------------------------------|
| `test_pricing_page_renders_200`         | GET /pricing returns 200                                         |
| `test_pricing_page_shows_all_tiers`     | Response includes all active pricing tiers                       |
| `test_pricing_page_tier_prices_correct` | Each tier card shows correct monthly/annual price                |
| `test_pricing_page_tier_features_listed`| Each tier lists its included features                            |
| `test_pricing_page_cta_links`           | Each tier has a working signup/subscribe CTA                     |

---

## 10. API Endpoint Tests

### 10.1 Pentest Request API

File: `tests/requests/api_pentest_requests.rs`

| Test                                           | Acceptance Criteria                                        |
|------------------------------------------------|------------------------------------------------------------|
| `test_post_request_creates_draft`              | POST /api/orgs/{org}/requests returns 201, status=draft    |
| `test_post_request_with_full_scope`            | All scope fields stored correctly                          |
| `test_post_request_validates_scope_fields`     | Missing required scope fields -> 422                       |
| `test_put_request_updates_draft`               | PUT /api/orgs/{org}/requests/{pid} updates draft fields    |
| `test_put_request_rejects_submitted`           | Cannot modify submitted request -> 409                     |
| `test_post_request_submit`                     | POST /api/orgs/{org}/requests/{pid}/submit -> submitted    |
| `test_post_request_withdraw`                   | POST /api/orgs/{org}/requests/{pid}/withdraw -> withdrawn  |
| `test_get_request_list`                        | GET /api/orgs/{org}/requests returns org-scoped list       |
| `test_get_request_detail`                      | GET /api/orgs/{org}/requests/{pid} returns full scope      |
| `test_post_request_requires_active_subscription`| Returns 403 if no active subscription                    |
| `test_post_request_requires_authentication`    | Returns 401 without valid auth                             |

### 10.2 Offer API

File: `tests/requests/api_offers.rs`

| Test                                           | Acceptance Criteria                                        |
|------------------------------------------------|------------------------------------------------------------|
| `test_get_offer_for_request`                   | GET /api/orgs/{org}/requests/{pid}/offer returns offer     |
| `test_post_offer_accept`                       | POST /api/orgs/{org}/offers/{pid}/accept creates engagement|
| `test_post_offer_reject`                       | POST /api/orgs/{org}/offers/{pid}/reject updates status    |
| `test_expired_offer_cannot_be_accepted`        | Accept on expired offer -> 409                             |
| `test_admin_create_offer`                      | POST /admin/requests/{pid}/offer creates offer (admin only)|

### 10.3 Findings API

File: `tests/requests/api_findings.rs`

| Test                                           | Acceptance Criteria                                        |
|------------------------------------------------|------------------------------------------------------------|
| `test_post_finding_creates_finding`            | POST /api/engagements/{pid}/findings returns 201           |
| `test_post_finding_validates_required_fields`  | Missing description/impact/recommendation -> 422           |
| `test_post_finding_validates_severity`          | Invalid severity string -> 422                             |
| `test_post_finding_validates_category`         | Invalid category -> 422                                    |
| `test_get_findings_by_engagement`              | GET /api/engagements/{pid}/findings returns list           |
| `test_get_findings_filtered_by_severity`       | ?severity=extreme filters correctly                        |
| `test_get_findings_filtered_by_category`       | ?category=injection filters correctly                      |
| `test_get_findings_pagination`                 | Pagination with ?page=&per_page= works correctly           |
| `test_put_finding_updates_fields`              | PUT /api/findings/{pid} updates finding                    |
| `test_finding_only_writable_by_assigned_pentester` | Non-assigned user -> 403 on POST/PUT               |
| `test_finding_readable_by_client_org`          | Client org member can GET finding -> 200                   |
| `test_finding_not_readable_by_other_org`       | Other org member -> 403/404                                |

### 10.4 Report API

File: `tests/requests/api_reports.rs`

| Test                                           | Acceptance Criteria                                        |
|------------------------------------------------|------------------------------------------------------------|
| `test_post_report_triggers_generation`         | POST /api/orgs/{org}/engagements/{pid}/report returns 202  |
| `test_get_report_status`                       | GET /api/orgs/{org}/reports/{pid} returns current status   |
| `test_get_report_download_when_ready`          | GET /api/orgs/{org}/reports/{pid}/download returns signed URL|
| `test_get_report_download_when_not_ready`      | Returns 409 if report still generating                     |
| `test_report_generation_requires_completed_engagement` | Cannot generate report for in-progress engagement  |
| `test_report_download_requires_org_membership` | Non-member -> 403/404                                      |
| `test_report_download_link_is_time_limited`    | Signed URL expires after configured TTL                    |

### 10.5 Domain Management API

File: `tests/requests/api_domains.rs`

| Test                                         | Acceptance Criteria                                         |
|----------------------------------------------|-------------------------------------------------------------|
| `test_post_domain_creates_unverified`        | POST /api/orgs/{org}/domains returns 201, verified=false    |
| `test_post_domain_duplicate_fqdn_rejected`   | Returns 409 if FQDN already registered for this org         |
| `test_get_domains_lists_org_domains`         | GET /api/orgs/{org}/domains returns only org's domains      |
| `test_delete_domain_removes_domain`          | DELETE /api/orgs/{org}/domains/{pid} returns 204            |
| `test_delete_domain_with_active_engagement_blocked` | Returns 409 if domain in active engagement scope     |
| `test_post_domain_verify_initiates_check`    | POST /api/orgs/{org}/domains/{pid}/verify returns 202       |
| `test_domain_verify_callback_marks_verified` | Verification completion marks domain as verified            |

### 10.6 Admin API

File: `tests/requests/api_admin.rs`

| Test                                         | Acceptance Criteria                                         |
|----------------------------------------------|-------------------------------------------------------------|
| `test_admin_list_pending_requests`           | GET /admin/requests?status=submitted returns all pending    |
| `test_admin_review_request`                  | PUT /admin/requests/{pid}/status to under_review            |
| `test_admin_create_offer_for_request`        | POST /admin/requests/{pid}/offer creates offer              |
| `test_admin_assign_pentester`                | POST /admin/engagements/{pid}/assign links pentester        |
| `test_admin_mark_engagement_completed`       | PUT /admin/engagements/{pid}/status to completed            |
| `test_admin_list_pentesters`                 | GET /admin/pentesters returns pentester user list            |
| `test_admin_endpoints_reject_non_admin`      | Client/pentester users get 403 on all admin endpoints       |

---

## 11. Edge Case Tests

File: `tests/models/edge_cases.rs` and `tests/requests/edge_cases.rs`

### 11.1 Expired Subscriptions

| Test                                            | Acceptance Criteria                                        |
|-------------------------------------------------|------------------------------------------------------------|
| `test_expired_sub_blocks_request_creation`      | Cannot create pentest requests when sub expired            |
| `test_expired_sub_allows_viewing_past_findings` | Past findings remain accessible after sub expires          |
| `test_expired_sub_allows_report_download`       | Past reports remain downloadable after sub expires         |
| `test_in_progress_engagement_continues_on_expiry` | Active engagement not interrupted by sub expiry          |
| `test_resubscribe_restores_access`              | Re-subscribing after expiry restores full access           |

### 11.2 Concurrent Request/Engagement Handling

| Test                                            | Acceptance Criteria                                        |
|-------------------------------------------------|------------------------------------------------------------|
| `test_max_concurrent_engagements_per_tier`      | Tier-based limit on simultaneous active engagements        |
| `test_engagement_completion_frees_slot`          | Completing engagement allows new one                       |
| `test_multiple_pending_requests_allowed`         | Org can have multiple requests in draft/submitted          |

### 11.3 Domain Ownership Verification

| Test                                            | Acceptance Criteria                                        |
|-------------------------------------------------|------------------------------------------------------------|
| `test_dns_txt_verification_happy_path`          | Correct TXT record -> domain marked verified                |
| `test_dns_txt_verification_wrong_token`         | Wrong TXT value -> verification fails                       |
| `test_http_meta_verification_happy_path`        | Correct meta tag at well-known path -> verified             |
| `test_verification_token_expiry`                | Expired token cannot be used for verification               |
| `test_reverification_required_after_transfer`   | Domain removed and re-added requires new verification       |
| `test_verification_rate_limit`                  | Max N verification attempts per hour per domain             |

### 11.4 Data Integrity Edge Cases

| Test                                            | Acceptance Criteria                                        |
|-------------------------------------------------|------------------------------------------------------------|
| `test_delete_org_cascades_correctly`            | Deleting org removes requests, engagements, findings, reports |
| `test_delete_engagement_cascades_to_findings`   | Deleting engagement removes associated findings            |
| `test_domain_fqdn_normalized`                   | FQDN stored lowercase, trailing dots stripped               |
| `test_finding_severity_enum_validated`           | Only extreme/high/elevated/moderate/low accepted            |
| `test_scope_fields_sanitized`                   | HTML/script tags stripped from user-provided scope text     |
| `test_finding_description_sanitized`            | HTML/script tags stripped from pentester-provided text      |

### 11.5 Workflow Edge Cases

| Test                                            | Acceptance Criteria                                        |
|-------------------------------------------------|------------------------------------------------------------|
| `test_offer_for_withdrawn_request_rejected`     | Cannot create offer for withdrawn request                  |
| `test_accept_offer_while_engagement_limit_hit`  | Accept fails gracefully if concurrent limit reached        |
| `test_pentester_removed_mid_engagement`         | Removing assignment blocks further finding additions       |
| `test_finding_added_after_findings_review`      | Cannot add findings after engagement in findings_review    |
| `test_report_generation_with_zero_findings`     | Report generated with "no findings" content                |

---

## 12. Snapshot Testing with Insta

Following the fracture-cms pattern, use `insta` with redactions for deterministic snapshots.

### Redaction patterns

```rust
macro_rules! configure_insta {
    ($($expr:expr),*) => {
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_suffix("gethacked");
        // Redact dynamic fields
        settings.add_redaction(".id", "[ID]");
        settings.add_redaction(".pid", "[UUID]");
        settings.add_redaction(".created_at", "[TIMESTAMP]");
        settings.add_redaction(".updated_at", "[TIMESTAMP]");
        settings.add_redaction(".started_at", "[TIMESTAMP]");
        settings.add_redaction(".completed_at", "[TIMESTAMP]");
        settings.add_redaction(".verified_at", "[TIMESTAMP]");
        settings.add_redaction(".expires_at", "[TIMESTAMP]");
        settings.add_redaction(".accepted_at", "[TIMESTAMP]");
        settings.add_redaction(".assigned_at", "[TIMESTAMP]");
        settings.add_redaction(".token", "[TOKEN]");
        settings.add_redaction(".download_url", "[URL]");
        settings.add_redaction(".created_by_user_id", "[USER_ID]");
        let _guard = settings.bind_to_scope();
    };
}
```

### Snapshot targets

- Service catalog listing response
- Pricing tier listing response
- Pentest request detail (with scope fields)
- Offer detail response
- Engagement detail response
- Finding detail response (with all structured fields)
- Findings list response (with severity breakdown)
- Report metadata response
- Domain listing response
- Error responses (quota exceeded, unauthorized, forbidden, not found, validation)
- Admin dashboard views

---

## 13. Test Helpers

Following the existing `create_test_user()` helper pattern from fracture-cms tests:

```rust
/// Creates a test user via OIDC, with auto-created personal org.
async fn create_test_user(db: &DatabaseConnection, suffix: &str) -> users::Model { ... }

/// Creates a test org (non-personal) and adds the user as owner.
async fn create_test_org(db: &DatabaseConnection, user: &users::Model, name: &str) -> organizations::Model { ... }

/// Subscribes an org to a pricing tier.
async fn subscribe_org(db: &DatabaseConnection, org_id: i32, tier_slug: &str) -> subscriptions::Model { ... }

/// Creates and verifies a domain for an org.
async fn create_verified_domain(db: &DatabaseConnection, org_id: i32, fqdn: &str) -> domains::Model { ... }

/// Creates a pentest request with full scope definition.
async fn create_pentest_request(
    db: &DatabaseConnection,
    org_id: i32,
    service_slug: &str,
    scope: PentestScope,
) -> pentest_requests::Model { ... }

/// Creates an offer for a pentest request.
async fn create_offer(
    db: &DatabaseConnection,
    request_id: i32,
    price_cents: i64,
    timeline_days: i32,
) -> offers::Model { ... }

/// Creates an engagement from an accepted offer.
async fn create_engagement_from_offer(
    db: &DatabaseConnection,
    offer: &offers::Model,
) -> engagements::Model { ... }

/// Creates a pentester user and assigns them to an engagement.
async fn create_assigned_pentester(
    db: &DatabaseConnection,
    engagement_id: i32,
    suffix: &str,
) -> (users::Model, pentester_assignments::Model) { ... }

/// Creates a finding with all required structured fields.
async fn create_finding(
    db: &DatabaseConnection,
    engagement_id: i32,
    pentester_id: i32,
    severity: &str,  // "extreme", "high", "elevated", "moderate", "low"
    category: &str,
) -> findings::Model { ... }

/// Creates a complete test scenario: user, org, subscription, domain, request,
/// offer, engagement, pentester, findings, report.
async fn create_full_pentest_scenario(
    db: &DatabaseConnection,
    suffix: &str,
) -> PentestScenario { ... }

/// Struct holding all entities for a complete test scenario.
struct PentestScenario {
    client_user: users::Model,
    client_org: organizations::Model,
    subscription: subscriptions::Model,
    domain: domains::Model,
    request: pentest_requests::Model,
    offer: offers::Model,
    engagement: engagements::Model,
    pentester: users::Model,
    assignment: pentester_assignments::Model,
    findings: Vec<findings::Model>,
    report: Option<reports::Model>,
}
```

These helpers go in `tests/helpers/mod.rs` or inline per test module, matching the existing pattern.

---

## 14. Performance Testing Considerations

Performance tests are NOT part of the standard `cargo test` suite. They should be run
separately, gated behind a feature flag or separate binary.

### Areas to benchmark

| Area                                   | Target                                              |
|----------------------------------------|-----------------------------------------------------|
| Findings query (1000+ findings)        | < 200ms for paginated response                      |
| Engagement listing (100+ engagements)  | < 150ms for paginated response                      |
| Authorization check (org + role)       | < 10ms per request middleware                        |
| Report generation trigger              | < 100ms to enqueue (actual generation is async)      |
| Landing page render                    | < 100ms TTFB                                        |
| Pricing page with tier data            | < 100ms TTFB                                        |
| Admin request listing (all orgs)       | < 200ms for paginated response                      |

### Approach

- Use `criterion` for micro-benchmarks on model methods
- Use `k6` or `wrk` for HTTP endpoint load testing (run against podman-deployed instance)
- Performance regression tests: store baseline metrics, fail CI if >20% regression

---

## 15. Test Execution Environment

### Podman container setup

```
# Test database container
podman run -d --name gethacked-test-db \
  -e POSTGRES_DB=gethacked_test \
  -e POSTGRES_USER=gethacked \
  -e POSTGRES_PASSWORD=test \
  -p 5433:5432 \
  docker.io/library/postgres:16-alpine

# Run tests
DATABASE_URL=postgres://gethacked:test@localhost:5433/gethacked_test \
  cargo test --workspace
```

### CI pipeline

1. Start podman postgres container
2. Run migrations
3. `cargo test --workspace -- --test-threads=1` (serial_test ensures ordering)
4. Collect coverage with `cargo-llvm-cov`
5. Upload snapshots if `insta` tests fail (for review)

---

## 16. Acceptance Criteria Summary (by Feature Area)

### Services & Catalog
- [ ] All service CRUD operations tested
- [ ] Service deactivation hides from public listing
- [ ] Service slugs are unique and URL-safe

### Pricing & Subscriptions
- [ ] All tiers created and queryable
- [ ] Subscription lifecycle (create, upgrade, downgrade, cancel, expire) tested
- [ ] Single active subscription per org enforced

### Pentest Requests (Scope Definition)
- [ ] Full scope definition with all fields (targets, IPs, domains, windows, exclusions, contact)
- [ ] Draft -> submitted -> under_review lifecycle tested
- [ ] Withdrawal flow tested
- [ ] Scope fields immutable after submission
- [ ] Cross-org isolation on all request endpoints

### Offers (Quote System)
- [ ] Offer creation by admin tested
- [ ] Accept/reject/expire lifecycle tested
- [ ] Offer acceptance triggers engagement creation
- [ ] Multiple negotiation rounds tested
- [ ] Offer visibility scoped to client org + admin

### Engagements
- [ ] Full lifecycle tested (created -> pentester_assigned -> in_progress -> findings_review -> completed)
- [ ] Status transition validation (no invalid transitions)
- [ ] Org scoping enforced at every step

### Pentester Assignments
- [ ] Assignment and removal tested
- [ ] Pentester sees ONLY assigned engagements
- [ ] Pentester cannot see client org data, contact info, or pricing
- [ ] Duplicate assignment prevented

### Findings
- [ ] All structured fields tested (description, technical_description, impact, recommendation, severity, category)
- [ ] Severity enum validation (extreme/high/elevated/moderate/low)
- [ ] Category validation against allowed values
- [ ] Only assigned pentesters can create/edit findings
- [ ] Client org can view findings (read-only)
- [ ] Cross-org isolation verified
- [ ] Filtering by severity and category works

### Reports
- [ ] Report generation triggered correctly
- [ ] Download available only when status=ready
- [ ] Report access scoped to client org
- [ ] Report includes all engagement findings
- [ ] Download link is time-limited

### Domains
- [ ] Domain creation and verification flow complete
- [ ] Verification token security (expiry, uniqueness)
- [ ] FQDN uniqueness per org
- [ ] Scope references only verified domains

### Security (CRITICAL -- must all pass before launch)
- [ ] ALL IDOR tests pass (14 tests)
- [ ] ALL privilege escalation tests pass (16 tests)
- [ ] ALL cross-tenant leakage tests pass (10 tests)
- [ ] ALL OWASP Top 10 tests pass (20 tests)
- [ ] ALL pentester isolation tests pass (9 tests)
- [ ] Complete role enforcement matrix tested (every endpoint x every role)
- [ ] Error responses reveal no internal details
- [ ] Security headers present on all responses
- [ ] No data leakage in pagination metadata

### Landing & Pricing Pages
- [ ] Pages render correctly for unauthenticated users
- [ ] Dynamic pricing data matches database
- [ ] SEO meta tags present
- [ ] GDPR/EU compliance elements present

### Edge Cases
- [ ] Expired subscriptions handled gracefully (ongoing engagements continue)
- [ ] Concurrent engagement limits enforced per tier
- [ ] Domain verification expiry enforced
- [ ] Cascade deletes maintain data integrity
- [ ] Workflow edge cases (mid-engagement pentester removal, etc.)

---

## 17. Test File Structure

```
tests/
  mod.rs                              # (add new modules)
  helpers/
    mod.rs                            # Shared test helpers + PentestScenario builder
  models/
    mod.rs                            # (add new modules)
    services.rs
    pricing_tiers.rs
    subscriptions.rs
    pentest_requests.rs
    offers.rs
    engagements.rs
    pentester_assignments.rs
    findings.rs
    reports.rs
    domains.rs
    domain_verifications.rs
    edge_cases.rs
    snapshots/                        # insta snapshot files
  requests/
    mod.rs                            # (add new modules)
    pages.rs                          # Landing + pricing page tests
    pentest_workflow.rs               # Full 5-phase lifecycle integration test
    offer_flow.rs                     # Offer negotiation tests
    subscription_flow.rs              # Subscription management tests
    api_pentest_requests.rs           # Pentest request API tests
    api_offers.rs                     # Offer API tests
    api_findings.rs                   # Findings API tests
    api_reports.rs                    # Report API endpoint tests
    api_domains.rs                    # Domain management API tests
    api_admin.rs                      # Admin-only API endpoint tests
    authorization.rs                  # Org isolation + role enforcement matrix
    security_idor.rs                  # IDOR vulnerability tests
    security_privesc.rs               # Privilege escalation tests
    security_tenant_isolation.rs      # Cross-tenant data leakage tests
    security_owasp.rs                 # OWASP Top 10 tests
    security_pentester.rs             # Pentester-specific access control tests
    edge_cases.rs                     # Edge case request tests
  tasks/
    mod.rs
  workers/
    mod.rs
    report_worker.rs                  # Report generation worker tests
```

---

## 18. Definition of Done

A feature is considered fully tested when:

1. All model unit tests pass (creating, reading, updating, deleting)
2. Cross-org isolation is verified for every data access path
3. Role enforcement is tested for every endpoint against the full role matrix
4. IDOR tests pass for every resource type (using guessed/enumerated PIDs)
5. Pentester isolation is verified (only assigned engagement access)
6. Snapshot tests are reviewed and committed
7. Edge cases for the feature area are covered
8. All security tests pass (IDOR, privesc, tenant isolation, OWASP)
9. All tests run green in podman containers in CI
10. No test uses `#[ignore]` without a tracking issue

---

## 19. Future Extensibility (Out of Scope, Design For)

The test infrastructure should be designed to easily accommodate future requirements without
requiring a rewrite. Current tests use the shared multi-tenant model (fracture-core orgs with
org-scoped queries), but must not assume this is the permanent architecture.

### 19.1 Per-Client Isolated Environments

The long-term goal is full isolation per client -- each client org gets its own isolated
deployment (separate DB, container/namespace, or tenant instance), auto-provisioned on
account creation.

**Test infrastructure design principles to support this transition**:

- **Abstract DB connection in test helpers**: All test helpers (`create_test_user`,
  `create_full_pentest_scenario`, etc.) must accept a `&DatabaseConnection` parameter
  rather than reaching into global state. This allows the same helpers to work against
  a per-tenant DB connection in the future.
- **No cross-tenant joins in test assertions**: Tests must never assert by joining across
  orgs in a single query. Each org's data should be verifiable independently. This ensures
  tests remain valid when orgs move to separate databases.
- **Parameterizable connection strings**: Test setup (podman container creation, migration
  running) must be driven by config/env vars, not hardcoded. This lets CI spin up N
  databases for N tenants if needed.
- **Isolation tests must be architectural, not just query-scoped**: Current cross-org
  isolation tests verify that queries are org-scoped. Future tests should also verify
  that tenant provisioning creates the correct isolated resources (DB, namespace, etc.).
  Design test helpers with a `TenantContext` abstraction that can wrap either a shared-DB
  org_id or an isolated DB connection.

**Future test additions (not yet implemented, but design for)**:

| Test Area                                  | What to test                                              |
|--------------------------------------------|-----------------------------------------------------------|
| `test_account_creation_triggers_provisioning` | New org creation triggers isolated environment setup    |
| `test_provisioned_env_has_migrations_applied` | New tenant DB has all migrations applied automatically |
| `test_provisioned_env_is_network_isolated` | Tenant container/namespace cannot reach other tenants     |
| `test_tenant_teardown_cleans_resources`    | Deleting org removes its isolated environment             |
| `test_shared_to_isolated_data_migration`   | Existing org data migrates cleanly to isolated DB         |
| `test_cross_tenant_network_boundary`       | Requests from tenant A's container cannot reach tenant B  |

**Test helper abstraction to prepare now**:

```rust
/// Wraps tenant-specific context. Currently just holds org_id for shared DB.
/// Future: will hold a dedicated DatabaseConnection + namespace info.
struct TenantContext {
    org: organizations::Model,
    db: DatabaseConnection,  // Currently shared; future: per-tenant
    // Future fields:
    // namespace: Option<String>,
    // container_id: Option<String>,
}

impl TenantContext {
    /// Current: returns the shared DB connection.
    /// Future: returns the tenant's isolated DB connection.
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}
```

### 19.2 Backdoor Agent for Local Assessments

Model for agent registration, agent-to-engagement linking, agent result ingestion.
Test helpers should allow mocking agent communication.

### 19.3 Automated Scanning

If automated tools are added alongside manual pentesting, the findings model and test
helpers already support programmatic finding creation.

### 19.4 Multi-Region Deployment

Test database setup should be parameterizable for different connection strings. The
`TenantContext` abstraction above supports this naturally -- each region could be
treated as a separate tenant context in tests.
