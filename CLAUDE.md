# CLAUDE.md — fracture-pt (gethacked.eu)

These rules are non-negotiable for any code change in this repo. They exist so that semgrep, the strict CSP, the framework conventions, and the safety properties of pentest-tool execution never regress. Read this file before editing.

## What this project is

Rust + Loco (Axum) pentesting / security portal targeting EU customers (gethacked.eu). Built on `fracture-core` (the library half of `fracture-cms`) as a git dependency. PT extends CMS with engagements, scan targets, findings, reports, invoicing, and tool-driven scans.

**Dependency direction: PT → CMS only.** Never patch CMS via fork-and-modify here; upstream the change instead.

**Severity scale:** Extreme / High / Elevated / Moderate / Low. Not CVSS, not numeric. Do not introduce CVSS scoring without an explicit decision.
**No compliance claims** (GDPR/ISO/NIS2/DORA) and **no certificate claims** ("certified", etc.) — use "experienced", "hands-on", "seasoned".

## Build & test commands

Use **local cargo** (Rust 1.94+ installed). Podman only for semgrep, the app stack, and pentest-tool sidecars.

| Task | Command |
|---|---|
| Format check | `cargo fmt --all -- --check` |
| Format apply | `cargo fmt --all` |
| Lint (matches CI) | `cargo clippy --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms` |
| Tests | `DATABASE_URL=sqlite:///tmp/gethacked_test.sqlite?mode=rwc cargo test --all-features --all` |
| Full local CI | `./dev/ci.sh` |
| Audit (with known ignores) | `cargo audit --ignore RUSTSEC-2023-0071` |
| Run app stack | `podman compose up -d` |
| Rebuild app | `podman compose down app && podman compose build app && podman compose up -d app` |
| Build docbuilder | `./dev/build-docbuilder.sh` |

`./dev/ci.sh` MUST pass before opening a PR.

## Pipelines that must always succeed

CI in `.github/workflows/rust.yml` runs:

1. `rustfmt` — `cargo fmt --all -- --check`
2. `clippy` — strict (`-D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms`)
3. `test` — `cargo test --all-features --all`
4. `semgrep` — auto config, errors fail the job
5. `cargo-audit` — `--ignore RUSTSEC-2023-0071` (the rsa advisory; remove once upstream releases a fix)

If clippy or semgrep finds something that genuinely cannot be fixed, justify inline:

- Clippy: `#[allow(clippy::lint_name)] // Reason: <explanation>`
- Semgrep: `// nosemgrep: <rule-id> -- <reason>` immediately above the offending line.

Never `--no-verify`. Never weaken the audit ignore list without an issue link.

## Security rules (mandatory)

### Authorization (IDOR prevention)

- **Every resource fetch in a handler goes through an org-scoped lookup**: `Model::find_by_pid_and_org(db, pid, org_id)` (or, after PR-3, the `OrgScoped` trait). Never call `find_by_pid` directly from a controller without a separate authorization check.
- **Use core macros for auth gates:**
  - `require_user!(user)` — must be authenticated
  - `require_role!(org_ctx, OrgRole::Viewer | Member | Admin)` — role gate
  - `require_platform_admin!(org_ctx)` — platform-admin gate
- **Pentester role is per-engagement, not per-org.** It lives in CMS as `ResourceAssignment` (after PR-4). PT consumes it via `auth::can_edit_findings(user_id, engagement_id)` etc. Do not introduce a new pentester table in PT.
- **Return 404 (not 403)** when access is denied — match the existing pattern.
- **Free-scan flow** is the only public scan entry point and is rate-limited and CAPTCHA-gated. Do not add additional unauthenticated scan endpoints.

### Pentest-tool execution (highest scrutiny)

- **All tools run in per-tool sidecar containers.** Never invoke an external binary directly from the app process.
- **All tool inputs validated by `services::tool_input::validate_target()`** (or the per-tool equivalent). Validation is the single bottleneck — no other place may build target strings.
- **Argv only.** Never use `sh -c`, `bash -c`, or pass `String` to a shell. `tokio::process::Command::new("...").args(&[...])` with a fixed argv shape.
- **Targets must be validated against the reserved-suffix denylist** (`.local`, `.internal`, `.onion`, `.example`) and the private/reserved IP ranges. The current canonical implementation is `services/port_scan.rs:validate_target` — share or extend that, do not fork it.
- **Active scans (nmap, nuclei, sslscan) require the tiered authorization gate**:
  1. Caller has `Member+` role on the org.
  2. AND one of: (a) DNS TXT proof of scope ownership on the target domain, (b) a signed engagement covering the asset, (c) platform admin override (logged).
  Passive scans (amass passive, viewdns lookups, crt.sh, dig) require `Member+` only.
  Implementation lives in `services::scan_authz`. Use it; never bypass.
- **Tool processes have hard limits**: 10-min wall clock, capped CPU/memory, read-only FS, no host network namespace where possible. The container runtime enforces this; the JobExecutor sets the limits.
- **Tool output captured fully but stored truncated**: `result_summary` (parsed JSON) is canonical; `result_output` (raw) is truncated to 100 KB to protect the DB. Larger raw output goes to disk as an upload tied to the job run.
- **Never log target inputs that fail validation as errors with full content** — log a redacted form. Validation errors at user input boundaries are user errors, not system errors.

### Content Security Policy

The strict CSP in `src/initializers/security_headers.rs` must stay:

```
default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'self'; form-action 'self'; base-uri 'self';
frame-ancestors 'none'
```

**Forbidden:**
- `unsafe-inline`, `unsafe-eval`, wildcards
- Removing `frame-ancestors 'none'`

Visualization (graphs, charts) **must use server-rendered SVG or static `<canvas>` driven by classified `/static/...` JS** — never inline scripts.

### SRI (subresource integrity)

All `<link rel="stylesheet">` and `<script>` from `/static/` must include `integrity="sha384-..."` and `crossorigin="anonymous"`. Update the hash when the file changes. The base template `assets/views/base.html` is the reference.

To compute SRI hashes:
```
openssl dgst -sha384 -binary path/to/file | openssl base64 -A
```

### Headers

Do not weaken `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `HSTS`, `X-Permitted-Cross-Domain-Policies`, `Permissions-Policy`.

### Database

- **No raw SQL.** SeaORM only.
- **Migrations** for org-owned tables include `org_id` FK + index, cascade-delete on org removal.
- **PIDs are UUIDs** stored as strings. Do not expose internal `id` (the autoincrement) in URLs.

### Templates

- Tera autoescape on; never `| safe` user input.
- Markdown via `services/markdown.rs` with `unsafe = false`. Do not flip.
- No inline `<script>` / `<style>`.
- **Reports** (PDF) go through pentext-docbuilder. Engagement and finding text are XML-escaped via `services/report_builder.rs:xml_escape`. Do not introduce new code paths that interpolate user content into the report XML without going through this helper.

### Secrets

- All secrets via env vars; never hardcoded. `.env` is gitignored; `.env.example` is the spec.
- `JWT_SECRET` must be base64-decodable.
- **Never use `$(cat ...)` to read tokens** in shell commands. Use `gh auth login` for GitHub auth and env vars in compose.

## Framework conventions

- **Loco 0.16, Axum 0.8, SeaORM 1.1, Tera, Fluent.** Match existing module shapes:
  - `src/controllers/<resource>.rs`
  - `src/models/<resource>.rs` (and untouched `src/models/_entities/<resource>.rs` from SeaORM-cli)
  - `src/views/<resource>.rs` (Tera context)
  - `assets/views/<resource>/...html`
  - Migration in `migration/src/m<date>_<name>.rs`, registered in `lib.rs`
- **Job pipeline:** new tools register a `JobExecutor` in `src/jobs/` and a runner in `src/services/`. Job results: structured into `result_summary`, raw output into `result_output` (truncated). Diffs go through `job_run_diffs`.
- **Auth extractors** (`OrgAuth<Role>`) — preferred over inline checks where the role is fixed.
- **Logging via `tracing`** — structured fields, no interpolated secrets.
- **Errors** propagate via `?`. No `unwrap`/`expect` in request or job-execution paths.

## Adding a new pentest tool — checklist

1. Author a new sidecar image: `containers/tools/<tool>/Containerfile` with the binary pinned to a version. Build via `dev/build-tools.sh`.
2. Add a `services/<tool>.rs` runner that builds argv, validates inputs via the shared validator, invokes the sidecar via `services::tool_runner`.
3. Add a `jobs/<tool>.rs` `JobExecutor` that calls the runner, parses output into `result_summary`, persists raw to upload if > 100 KB.
4. Register the executor in `app.rs::routes()`.
5. Add a Tera template fragment for the parsed-output view in `assets/views/jobs/<tool>.html`.
6. Wire the active/passive classification into `services::scan_authz` so the tiered gate applies correctly.
7. Tests: golden-fixture parser test, integration test for the JobExecutor with a mocked sidecar, IDOR test for the controller that triggers it.
8. Docs: ARCHITECTURE.md (where it sits in the pipeline), DEVELOPMENT.md (how to run the sidecar locally), DEPLOYMENT.md (image tag pinning).

## Testing

- Unit + integration tests for new logic.
- IDOR test for any new controller that fetches an org-owned resource by id.
- Golden fixtures for tool output parsers — never call live external services in tests.
- `cargo test --all-features --all` must pass.

## Manual local testing (mandatory before marking work complete)

Automated tests verify code correctness; they do not verify feature correctness in a browser, nor do they verify that a tool sidecar actually runs end-to-end. After every change that touches a request handler, view, template, asset, scan pipeline, tool runner, or the data model:

1. Rebuild and bring up the affected services with the latest working tree:
   - App-only: `podman compose down app && podman compose build app && podman compose up -d app`.
   - Tools/worker: also rebuild the worker / tool sidecar(s) you changed (e.g. `podman compose build worker amass nuclei sslscan && podman compose up -d`).
2. Tail logs: `podman compose logs -f app worker`.
3. Walk the affected pages in a browser. Always check:
   - Golden path
   - At least one edge case (empty input, role boundary, target denied by validator)
   - The browser console for CSP violations — any new violation is a regression and blocks the PR.
4. For scan/tool work:
   - Trigger a job through the UI against a known-safe target (e.g., a domain you own).
   - Confirm job output is visible in the UI, parsed into the structured view, and the raw output is captured.
   - Confirm passive vs. active gate behaves correctly (an active scan against an unverified target must be refused with a clear message).
5. For RBAC work, exercise each affected role and the platform-admin override; explicitly verify the negative case (a role that should NOT have access).
6. For migrations, confirm `auto_migrate` runs cleanly on a fresh DB *and* on a DB seeded from the prior schema.

Do not mark a task or PR complete on `cargo test` alone. If the manual test cannot be run (missing IdP creds, missing target ownership, etc.), state this explicitly in the PR description rather than claim success.

## Documentation

- README — high-level only.
- `docs/ARCHITECTURE.md` — target architecture, module map, scan pipeline, tool sandbox model. Must be updated for any change to the scan pipeline or auth model.
- `docs/DEVELOPMENT.md` — local dev setup, debug, test workflow.
- `docs/DEPLOYMENT.md` — production deployment via fracture-ctl.
- `docs/THREAT_MODEL.md` — security posture, gates, surfaces.

## PR hygiene

- One concern per PR. Don't bundle unrelated cleanups.
- PR title: imperative, < 70 chars.
- PR body: summary + test plan checklist.
- PR must: pass CI, include tests, update docs if contract changes.
- Never force-push or amend after merge. New PR for follow-ups.
- The user reviews and merges. PRs that include security-sensitive changes must explicitly call out: which gate they touch, which validators they change, and whether new external network is reached.

## Don't do

- Don't add scan endpoints that bypass the tiered auth gate.
- Don't pass user input directly to a shell.
- Don't relax the CSP, even briefly, for convenience.
- Don't store unredacted user-provided targets in error logs.
- Don't introduce floating git deps that point at branches; always pin to a tag or rev.
- Don't add CVSS scoring or compliance claims (see project memory).
- Don't fork CMS code into PT — upstream the change.
