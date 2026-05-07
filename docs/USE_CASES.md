# Use Cases — fracture-pt

Named user journeys exercised by the portal. Use these for product-level reasoning, regression testing, and sequencing of features. Where they intersect security-critical contracts (scan authorization, IDOR prevention), the relevant constraint is called out.

## Personas

- **Customer Owner** — created an org, owns their data. Org role: `Owner`. Wants to monitor their company's attack surface and request pentests.
- **Customer Member** — invited into a customer org by the Owner. Org role: `Member` or `Viewer`. May view findings; may run scans depending on tier and target verification.
- **Pentester** — external contractor. *Not* an org member. Granted access to specific engagements via `ResourceAssignment`. Sees only their assigned engagements.
- **Platform Admin** — gethacked.eu staff. Has the platform-admin flag. Can view any org and override the active-scan gate (logged).
- **Anonymous Visitor** — no account. Can browse marketing pages and run a single rate-limited free scan.

## UC-1 — Anonymous free scan

```mermaid
sequenceDiagram
    actor Visitor
    participant Web as Free-scan page
    participant RateLimit
    participant Captcha
    participant Worker

    Visitor->>Web: enter example.com
    Web->>RateLimit: check IP
    RateLimit-->>Web: ok
    Web->>Captcha: verify
    Captcha-->>Web: ok
    Web->>Worker: enqueue PASSIVE asm_scan(example.com)
    Worker->>Worker: crt.sh + DNS resolution
    Worker-->>Web: subdomains, certs, IPs
    Web-->>Visitor: surface map + CTA to sign up
```

**Constraints**: only passive tools run. No nmap, no nuclei, no sslscan. Rate-limited per IP. Captcha-gated.

## UC-2 — Self-service signup and continuous monitoring

```mermaid
sequenceDiagram
    actor Owner as Customer Owner
    participant Portal
    participant OIDC
    participant Worker

    Owner->>Portal: click "Sign in"
    Portal->>OIDC: redirect to provider
    OIDC-->>Portal: authorization code
    Portal->>Portal: find_or_create_from_oidc
    note over Portal: personal org auto-created
    Portal-->>Owner: dashboard

    Owner->>Portal: add scan_target "company.example"
    Portal->>Worker: enqueue PASSIVE asm_scan
    Worker-->>Portal: subdomains, IPs

    Owner->>Portal: "Verify ownership"
    Portal-->>Owner: TXT token to place
    Owner->>Owner: place TXT in DNS
    Owner->>Portal: "Re-check"
    Portal->>Portal: look up TXT, set verified_at
    Portal-->>Owner: verified ✓

    Owner->>Portal: "Enable continuous scanning"
    Portal->>Worker: schedule recurring stages (active now allowed)
    Worker-->>Owner: email on diff (subdomain added, port opened, finding)
```

**Constraints**:
- Free tier limits to 1 target.
- Active tools blocked until `verified_at` is set, OR a signed engagement covers the target.
- Email alerts only on tiers ≥ Recon.

## UC-3 — Engagement-driven manual pentest

```mermaid
sequenceDiagram
    actor Customer
    actor Admin as Platform Admin
    actor Pentester
    participant Portal
    participant ResourceAssignment as RA
    participant Worker

    Customer->>Portal: request engagement (web app pentest)
    Portal->>Portal: engagement.status = requested
    Admin->>Portal: scope + create engagement_targets
    Admin->>Portal: send offer
    Portal->>Customer: offer email
    Customer->>Portal: accept offer
    Portal->>Portal: engagement.status = accepted
    note over Portal: linked targets now unlock active scans (test window opens)

    Admin->>RA: ResourceAssignment::assign(pentester, engagement, "pentester")
    Pentester->>Portal: log in
    Portal->>RA: list_for_user(pentester, "engagement")
    Portal-->>Pentester: dashboard with assigned engagements only

    Pentester->>Worker: run nuclei against linked target
    Worker->>Worker: scan_authz: SignedEngagement → active allowed
    Worker-->>Pentester: structured findings

    Pentester->>Portal: edit findings, mark severity
    Pentester->>Portal: build report
    Portal->>Portal: pentext-docbuilder → PDF
    Portal-->>Customer: report deliverable + invoice
```

**Constraints**:
- Pentester sees only assigned engagements (cross-engagement IDOR blocked at query layer via `ResourceAssignment`).
- Pentester cannot manage org membership.
- Active scans permitted *only* on linked targets in `accepted`/`in_progress` status with the test window open.

## UC-4 — Scheduled regression scan

```mermaid
sequenceDiagram
    participant Scheduler as job_scheduler
    participant Dispatcher as job_dispatcher
    participant ScanAuthz
    participant Worker
    participant Mailer

    loop on schedule (tier-gated)
        Scheduler->>Dispatcher: enqueue scheduled run
        Dispatcher->>ScanAuthz: evaluate(target, caller)
        alt active allowed
            ScanAuthz-->>Dispatcher: ok
            Dispatcher->>Worker: run active stages
        else only passive
            Dispatcher->>Worker: run passive stages, skip active with reason
        end
        Worker-->>Dispatcher: result + diffs
        opt diffs non-empty AND tier allows alerts
            Dispatcher->>Mailer: alert email
        end
    end
```

**Constraints**:
- Scheduling enabled at tier ≥ Strike.
- Email alerts at tier ≥ Recon.

## UC-5 — Cross-engagement isolation (IDOR prevention)

This is a *negative* use case: it must not work.

```mermaid
sequenceDiagram
    actor BobPentester as Bob (pentester on Eng-A)
    participant Portal
    participant RA as ResourceAssignment

    BobPentester->>Portal: GET /engagements/<Eng-B-pid> [Bob NOT assigned]
    Portal->>RA: has_assignment(bob, "engagement", Eng-B.id, "pentester")
    RA-->>Portal: false
    Portal-->>BobPentester: 404 (existence not leaked)
```

**Constraints**: 404, not 403, to avoid leaking existence. The check is at the model/query layer, not just at the controller.

## UC-6 — Platform admin override

Used by gethacked.eu staff for incident response or customer support. Always logged.

```mermaid
sequenceDiagram
    actor Admin as Platform Admin
    participant Portal
    participant ScanAuthz
    participant AuditLog

    Admin->>Portal: trigger active scan on customer target
    Portal->>ScanAuthz: evaluate(target, caller{is_platform_admin: true})
    ScanAuthz-->>Portal: active allowed (PlatformAdminOverride)
    Portal->>AuditLog: log override (actor, target, reason)
    Portal->>Portal: enqueue active job
```

**Constraints**: every override entry must be auditable (actor + target + timestamp). The audit log itself is a separate follow-up.

## UC-7 — Subscription change

```mermaid
sequenceDiagram
    actor Owner as Customer Owner
    participant Portal
    participant Tier as PlanTier

    Owner->>Portal: upgrade to Strike (€299/mo)
    Portal->>Portal: subscriptions row inserted
    Portal->>Portal: org settings.plan_tier = "strike"
    Portal->>Tier: from_org() → Strike
    note over Portal: scheduling_enabled = true; max_targets = 3
    Portal-->>Owner: features unlocked
```

## UC-8 — Manual scope-of-work decline

```mermaid
sequenceDiagram
    actor Customer
    actor Admin
    participant Portal

    Customer->>Portal: request engagement
    Admin->>Portal: review — scope unrealistic
    Admin->>Portal: reject
    Portal->>Portal: engagement.status = rejected
    Portal-->>Customer: explanatory email
```

**Constraints**: targets that were linked to a rejected engagement do not unlock active scans (status check filters them out).

## Anti-use-cases (must fail)

These exist to be tested explicitly:

1. **Anonymous user runs nmap.** Free-scan flow is passive-only. Must fail.
2. **Customer Member runs nuclei against an unverified target with no engagement.** scan_authz denies. Must fail.
3. **Pentester sees an engagement they are not assigned to.** Returns 404 on direct PID access. Must fail.
4. **One org's user reads another org's findings via guessed PID.** `OrgScoped::find_in_org` filters at the model layer. Must fail.
5. **Active scan against `localhost`, `127.0.0.1`, `*.local`, `*.internal`, `*.onion`, `*.example`.** `validate_target` rejects. Must fail.
6. **Active scan against private RFC1918 IPs.** `validate_target` rejects. Must fail.
7. **Markdown content in a finding executes JavaScript when rendered.** comrak `unsafe = false` strips it. Must fail.

Each of these has (or will have) at least one test asserting the failure.
