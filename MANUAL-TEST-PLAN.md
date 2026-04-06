# Manual Test Plan — fracture-pt (GetHacked.eu)

**Date**: 2026-04-06
**Roles used**: Platform Admin (PA), Client User, Pentester
**Environment**: Local dev at http://localhost:5150 with Zitadel at http://localhost:8080

> Walk through each section in order. Where it says "as Admin" or "as Client", use the appropriate browser session. Use two separate browser profiles (or incognito) for cross-org isolation tests. Check each box as you go.

---

## 1. Guest Experience (no login)

Open http://localhost:5150 in a fresh browser (no session).

- [ ] **TC-001**: The landing page loads with the hero section, service cards, pricing tiers, and call-to-action buttons.
- [ ] **TC-002**: Top nav shows: Services dropdown, Pricing, Scope Your Pentest, About, Contact, Blog, Free Scan, and a user icon for Sign in.
- [ ] **TC-003**: Click each service link (Penetration Testing, Vulnerability Scanning, Red Teaming, Attack Surface Mapping, Incident Response) — each loads a styled page.
- [ ] **TC-004**: Click **Pricing** — the page shows Free/Strike/Offensive tiers with a comparison table and FAQ.
- [ ] **TC-005**: Click **Scope Your Pentest** — the scope wizard loads with approach selector (Crystal/Grey/Black/Red-team), complexity slider, and live price estimate.
- [ ] **TC-006**: Move the complexity slider to "Medium" — duration shows ~6 days. Move to "Enterprise" — shows ~25 days. Switch between approaches — the differences are small (1-3 days).
- [ ] **TC-007**: Click **About**, **Contact**, **Blog** — each renders a styled page without errors.
- [ ] **TC-008**: Footer links work: Privacy Policy, Terms of Service, Impressum each load their legal page.
- [ ] **TC-009**: Visit `/robots.txt` and `/sitemap.xml` — both return valid content.

### Free Scan (unauthenticated)

- [ ] **TC-010**: Click **Free Scan** in the nav. Enter a valid domain (e.g. `example.com`) and click the scan button.
- [ ] **TC-011**: You're redirected to a progress page showing the domain name and a spinner or status indicator.
- [ ] **TC-012**: Try entering `localhost` — the form shows a validation error and does not submit.
- [ ] **TC-013**: Try `foo.test`, `bar.onion`, `x` (single label), empty — all rejected with clear error messages.
- [ ] **TC-014**: Open the scan results URL in a different browser (without the cookie) — you get "Access denied".
- [ ] **TC-015**: Verify the badge says "Temporary Results" (not "No Data Stored").

---

## 2. Authentication

- [ ] **TC-016**: Click the user icon → "Sign in / Create account". You're redirected to the Zitadel login page.
- [ ] **TC-017**: Complete login with your admin account. You're redirected back to the dashboard showing your name, org switcher, and stat cards.
- [ ] **TC-018**: Click your avatar → "Sign out". You're logged out and returned to the guest landing page.
- [ ] **TC-019**: After signing out, try visiting `/targets` directly — you're redirected to login, not shown a 500 error.
- [ ] **TC-020**: Log back in. Wait 15+ minutes without interacting. Try navigating — the session should handle expiry gracefully (redirect or refresh prompt, not a broken page).

---

## 3. Organization Management

Log in as the admin user.

- [ ] **TC-021**: The org switcher in the top nav shows your current org name. Click it — a dropdown shows all your orgs, plus "Create Organization" and "Manage Organizations" links.
- [ ] **TC-022**: Click **Create Organization**. Enter a name (e.g. "Test Org Alpha") and submit. The new org appears in the org switcher.
- [ ] **TC-023**: Create a second org ("Test Org Beta") the same way.
- [ ] **TC-024**: Use the org switcher to switch between Alpha and Beta. The page reloads showing the selected org's context.
- [ ] **TC-025**: Click **Manage Organizations**. You see a list of all your orgs. Click into one.
- [ ] **TC-026**: Go to **org settings** (click the org → Settings). You see the org name, plan tier, and feature flags.
- [ ] **TC-027**: Change the org name and click **Save Changes**. The name updates.
- [ ] **TC-028**: As a platform admin, the Plan Tier dropdown is visible. Change it from Free to Pro and save. The features section updates to show scheduling, port scans, and email alerts enabled.
- [ ] **TC-029**: Go to **Manage Members**. You see yourself listed. The invite form is available.
- [ ] **TC-030**: Invite a user by email. The invitation appears (or the user is added if they already exist).

---

## 4. Cross-Org Isolation (CRITICAL)

**Setup**: You need two orgs (Alpha and Beta from above) and ideally two separate user accounts. If using one admin account, switch between orgs. The key test: data from one org must not be accessible when viewing another org.

### Prepare test data
- [ ] **TC-031**: Switch to Org Alpha. Add a scan target (e.g. `alpha-target.example.com`). Note the target's URL.
- [ ] **TC-032**: Create an engagement request in Org Alpha. Note the engagement URL.
- [ ] **TC-033**: Switch to Org Beta.

### Test isolation
- [ ] **TC-034**: In Org Beta, paste the Org Alpha target URL into the address bar. You should see a **404 Not Found** page (not 403 — no existence leak).
- [ ] **TC-035**: Paste the Org Alpha engagement URL. Again, 404.
- [ ] **TC-036**: Go to `/targets` in Org Beta — only Beta's targets appear (empty if none created).
- [ ] **TC-037**: Go to `/engagements` in Org Beta — only Beta's engagements appear.
- [ ] **TC-038**: If a report exists in Org Alpha, try its download URL from Org Beta — 404.
- [ ] **TC-039**: If findings exist in Org Alpha, try their URL from Org Beta — 404.

### Admin cross-org access (expected)
- [ ] **TC-040**: As platform admin, visit `/admin/engagements` — you see engagements from ALL orgs with org names displayed (not "Org #4").
- [ ] **TC-041**: Visit `/admin/scan-targets` — all targets across orgs are listed with correct org names.
- [ ] **TC-042**: Visit `/admin/findings` — cross-org findings visible with org context.

### Pentester isolation
- [ ] **TC-043**: As a pentester assigned to an engagement in Org Alpha, visit `/pentester/engagements` — you see only your assigned engagements.
- [ ] **TC-044**: Try accessing a different org's engagement via `/pentester/engagements/{other_pid}` — 404.

---

## 5. Scan Target Management

Switch to an org with Pro or Enterprise tier.

- [ ] **TC-045**: Click **Scans** in the nav. You see the targets list (empty or with existing targets).
- [ ] **TC-046**: Click **Add Target**. Fill in a domain name (e.g. `test-domain.example.com`) with type "Domain". Submit.
- [ ] **TC-047**: You're redirected to the target detail page. An ASM scan should auto-start — you see a spinner or "running" status.
- [ ] **TC-048**: Add an IP target (e.g. `93.184.216.34`) with type "IP". It's created successfully.
- [ ] **TC-049**: Try adding `localhost` as a hostname — rejected with "Reserved hostname".
- [ ] **TC-050**: Try adding `192.168.1.1` as an IP — rejected with "private range" error.
- [ ] **TC-051**: Try adding `100.64.0.1` — rejected as CGNAT.
- [ ] **TC-052**: Try adding `169.254.169.254` — rejected (cloud metadata IP).
- [ ] **TC-053**: On the target detail page, click **Trigger Port Scan** (Pro/Enterprise only). A port scan job starts.
- [ ] **TC-054**: On the targets list, click the delete button for a target. Confirm the dialog. The target is removed.

### Tier limits
- [ ] **TC-055**: Switch to a Free tier org. Add 1 target — succeeds. Try adding a 2nd — you see "Your Free plan allows up to 1 target. Upgrade to add more."
- [ ] **TC-056**: On a Free tier org, the port scan button should either be hidden or return an error about the Free plan.

---

## 6. Engagement Workflow (Client Side)

- [ ] **TC-057**: Click **Pentests** in the nav. You see the engagements list.
- [ ] **TC-058**: Click **Request Pentest**. Fill in the form: title, target systems, contact name, email. Select a service from the dropdown. Submit.
- [ ] **TC-059**: The engagement appears in the list with status "requested".
- [ ] **TC-060**: Click into the engagement. You see the detail page with scope info, contact details, and status badge.

---

## 7. Engagement Workflow (Admin Side)

As platform admin:

- [ ] **TC-061**: Click **Admin** in the top nav. Then navigate to engagements (or visit `/admin/engagements`).
- [ ] **TC-062**: The list shows the pending engagement request with the client's org name.
- [ ] **TC-063**: Click into the engagement. You see the full detail: client info, scope, and action panels.
- [ ] **TC-064**: Fill in the **Create Offer** form (amount, currency, timeline, deliverables). Submit. Status changes to "offer_sent".
- [ ] **TC-065**: (As client) Go back to the engagement detail. The offer is visible. Click **Accept**. Status becomes "accepted".
- [ ] **TC-066**: (As admin) Back on the admin engagement page, the **Assign Pentester** dropdown is now available. Select a user and assign them. Status moves to "in_progress".
- [ ] **TC-067**: Try assigning a client org member as pentester — rejected with "Cannot assign a client org member as pentester to their own engagement".
- [ ] **TC-068**: Use the status dropdown to move through: in_progress → review → delivered. Each transition should work.
- [ ] **TC-069**: Try an invalid transition (e.g. delivered → in_progress) — rejected with "Invalid transition" error.
- [ ] **TC-070**: Test cancellation: create a new engagement, then cancel it from admin. Status becomes "cancelled" and completed_at is set.

---

## 8. Pentester Workspace

As a user who has been assigned as pentester to an in_progress engagement:

- [ ] **TC-071**: Click **Workspace** in the nav (visible for admins and assigned pentesters). The engagement list shows your assigned engagement with org name and status.
- [ ] **TC-072**: The org switcher in the top nav shows your org context (not "No Org").
- [ ] **TC-073**: Click into the engagement. You see the detail page with sections for Findings and Non-Findings.
- [ ] **TC-074**: Click **Add Finding**. Fill in all fields: title, description, technical description, impact, recommendation, severity (choose "high"), category, CVE ID, evidence, affected asset. Submit.
- [ ] **TC-075**: The finding appears in the findings list on the engagement page.
- [ ] **TC-076**: Click **Edit** on the finding. Change the severity to "elevated" and update. The change is saved.
- [ ] **TC-077**: Try adding a finding with severity "critical" (not in the allowed list) — rejected with "Invalid severity level".
- [ ] **TC-078**: Click **Delete** on a finding. Confirm. The finding is removed.
- [ ] **TC-079**: Click **Add Non-Finding**. Enter a title and content. Submit. It appears in the non-findings list.
- [ ] **TC-080**: Edit and delete a non-finding — both work.

### Status guards
- [ ] **TC-081**: If the admin changes the engagement status to "review", try adding a new finding — rejected with "Cannot add findings in current state".

---

## 9. Report Generation & Download

Continue in the pentester workspace with an in_progress engagement that has findings:

- [ ] **TC-082**: Click the **Report** tab/link. You see the report page showing finding count, non-finding count, and a "Generate Report" button.
- [ ] **TC-083**: Click **Generate Report**. The page shows a "running" status indicator.
- [ ] **TC-084**: Refresh after a few seconds. The report should appear in the reports list with a download link.
- [ ] **TC-085**: Click **Download**. The browser downloads a ZIP file. Open it — it contains `source/report.xml` and finding XML files.
- [ ] **TC-086**: Try generating a report with zero findings — rejected with "Cannot generate report without findings".

### Download access control
- [ ] **TC-087**: As the client (org member), go to **Reports** in the nav. The report appears. Click download — it works.
- [ ] **TC-088**: As the platform admin, the download also works.
- [ ] **TC-089**: As an unrelated user (different org, not pentester), try the download URL — 404.

---

## 10. Admin Panel

As platform admin, click **Admin** in the nav:

- [ ] **TC-090**: Visit `/admin/engagements` — engagement list across all orgs, org names visible.
- [ ] **TC-091**: Visit `/admin/engagements/all` — all engagements (all statuses).
- [ ] **TC-092**: Visit `/admin/findings` — cross-org findings list.
- [ ] **TC-093**: Visit `/admin/users` — all registered users with org counts.
- [ ] **TC-094**: Visit `/admin/scan-targets` — all targets with org names.
- [ ] **TC-095**: Visit `/admin/reports` — all reports.
- [ ] **TC-096**: Visit `/admin/subscriptions` — subscription list.
- [ ] **TC-097**: Visit `/admin/invoices` — invoice list.
- [ ] **TC-098**: Visit `/admin/non-findings` — non-findings list.

### Admin access control
- [ ] **TC-099**: Log in as a regular (non-admin) user. Try visiting `/admin/engagements` — you're blocked (403 or redirect).
- [ ] **TC-100**: Try `/admin/users` as non-admin — blocked.

---

## 11. Dashboard

- [ ] **TC-101**: Log in and visit the home page. The dashboard shows four stat cards: Targets, Engagements, Findings, Reports — each with a count and a link.
- [ ] **TC-102**: Click each stat card — it navigates to the corresponding list page.
- [ ] **TC-103**: If findings exist, the severity breakdown bar is visible showing colored segments for each level (extreme/high/elevated/moderate/low) with percentages.
- [ ] **TC-104**: Active engagements section shows in_progress/review engagements with finding counts.
- [ ] **TC-105**: If an ASM scan has completed, the "Latest Scan" widget shows domain, subdomain count, resolved count, and cert count.
- [ ] **TC-106**: Visit the homepage as a guest (not logged in) — the guest landing page appears, not the dashboard.

---

## 12. Contact Form & Scope Wizard

- [ ] **TC-107**: Go to the **Contact** page. Fill in name, email, subject, message. Submit. You see a success page.
- [ ] **TC-108**: Submit with an empty name — you see a validation error.
- [ ] **TC-109**: Submit with an invalid email (no `@`) — validation error.
- [ ] **TC-110**: On the **Scope Your Pentest** page, configure a scope (approach, complexity, duration). Click **Request Quote**. Fill in the form and submit. The scope wizard data is included in the submission.

---

## 13. Navigation & Role Visibility

- [ ] **TC-111**: As a **regular client user**: the nav shows Dashboard, Scans, Pentests, Reports, Blog. There is NO "Workspace" or "Admin" link.
- [ ] **TC-112**: As a **platform admin**: the nav shows all of the above PLUS "Workspace" and "Admin" links.
- [ ] **TC-113**: As a **pentester with assignments**: the nav shows "Workspace" (if also admin; otherwise workspace is accessible but may not show in nav depending on role).
- [ ] **TC-114**: The org switcher always shows the current org. Switching orgs reloads the page with the new context.
- [ ] **TC-115**: Breadcrumbs are visible on detail pages (e.g. Organizations > Org Name > Settings).
- [ ] **TC-116**: The skip-to-content link is present (Tab from the top of the page).

---

## 14. Tier Enforcement Summary

| Feature | Free | Pro | Enterprise |
|---------|------|-----|------------|
| Max targets | 1 | 10 | Unlimited |
| ASM scans | Yes | Yes | Yes |
| Port scans | No | Yes | Yes |
| Scheduling | No | Yes | Yes |
| Email alerts | No | Yes | Yes |

- [ ] **TC-117**: Verify the org settings page correctly displays these restrictions for each tier.
- [ ] **TC-118**: Verify the plan tier can only be changed by a platform admin (the dropdown is hidden for non-admins).

---

**Total: 118 test cases**

> After completing all tests, note any failures with screenshots and the browser console output. File issues for any P0/P1 failures before deploying.
