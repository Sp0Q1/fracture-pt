# Testing fracture-pt

The end-to-end flow for validating a change locally — the same gates CI enforces,
plus how to actually run the app and exercise the core workflows by hand.

See [`DEVELOPMENT.md`](DEVELOPMENT.md) for first-time setup and tooling rationale;
this doc is the "what to run, in what order, and what to check" checklist.

## 1. Automated gates (must all pass before a PR)

These mirror `.github/workflows/rust.yml` exactly. Run them with the local
toolchain (fast); only semgrep needs podman.

```bash
# format
cargo fmt --all -- --check

# strict lint
cargo clippy --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms

# tests (SQLite file DB; the suite is #[serial])
DATABASE_URL='sqlite:///tmp/gethacked_test.sqlite?mode=rwc' cargo test --all-features --all

# dependency audit (RUSTSEC-2023-0071 is the accepted RSA-Marvin advisory, no fix)
cargo audit --ignore RUSTSEC-2023-0071

# SAST (podman)
podman run --rm -v "$PWD:/src:ro" -w /src docker.io/semgrep/semgrep:latest \
  semgrep scan --config auto --error \
  --exclude-rule python.django.security.django-no-csrf-token.django-no-csrf-token .
```

`./dev/ci.sh` bundles fmt + clippy + semgrep + tests in one shot.

Expected: **126 tests pass** (31 unit + 95 integration), clippy clean, semgrep
0 findings, audit clean (only unmaintained-crate *warnings*).

> The full suite runs in a single process and boots the app once per test. loco
> installs a template file-watcher in debug builds; the test environment nulls it
> (`src/initializers/view_engine.rs`) so watchers don't accumulate and exhaust
> the OS inotify limit. If you ever see `error creating file watcher`, that guard
> regressed.

## 2. Quick smoke — run the app with no IdP

For fast iteration on public pages / templates / migrations, run natively with
OIDC disabled (the OIDC initializer skips when `client_id`/`issuer` are empty):

```bash
export DATABASE_URL='sqlite:///tmp/gethacked_pt_dev.sqlite?mode=rwc'
export OIDC_CLIENT_ID='' OIDC_CLIENT_SECRET='' OIDC_ISSUER_URL=''
export JWT_SECRET="$(openssl rand -base64 32)"
export SERVER_BINDING='127.0.0.1'
cargo run --bin fracture-pt-cli -- start --environment development
```

Verify:
- boot log shows every migration `has been applied` on the fresh DB,
- `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5150/` → `200`,
- `/pricing`, `/blog`, `/scope` render `200`,
- no `error`/`panic` lines in the log, no CSP violations in the browser console.

Authenticated pages (engagements, findings, jobs, admin) redirect to OIDC and
need the full stack below.

## 3. Full stack — with auth (Zitadel)

```bash
./dev/setup.sh           # writes .env (dev secrets + JWT_SECRET), provisions Zitadel
podman compose up -d     # zitadel + zitadel-db + mailcrab + app
./dev/build-docbuilder.sh   # one-off; needed for report PDFs
podman compose logs -f app  # watch while exercising
```

- App: http://localhost:5150 · MailCrab (outbound email): http://localhost:1080
- Sign in via the OIDC flow; the dev realm's seeded users are printed by `setup.sh`.

## 4. Manual smoke checklist (core domain)

After any change touching a handler, view, template, model, or migration, walk
the golden path plus one edge case. Minimum coverage for the pentest lifecycle:

- [ ] **Onboarding** — a fresh OIDC login lands in an org (the framework places
      brand-new users in the default org; staff reach all orgs).
- [ ] **Engagement request** → **offer** → **accept** → **pentester assignment**
      → **finding** → **report** completes end to end.
- [ ] **Org isolation (IDOR)** — a user in org A gets `404` (not `403`) on an
      engagement/finding/report owned by org B.
- [ ] **Scan-authz gate** — an *active* scan against an unverified, unsigned
      target is refused; verification or a signed engagement (or staff override,
      which is logged) unlocks it. See [`SCAN_AUTHZ.md`](SCAN_AUTHZ.md).
- [ ] **Jobs** — a queued job runs (JobRunner initializer), the run page
      live-updates, and a completed run shows its diff.
- [ ] **RBAC negative cases** — a Viewer cannot mutate; a client cannot manage
      staff-owned resources.
- [ ] **Browser console** — zero CSP violations on every page touched.

Do not mark work complete on `cargo test` alone. If the IdP-dependent steps
can't be run (no Zitadel), say so explicitly in the PR.
