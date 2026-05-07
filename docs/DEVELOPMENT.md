# Development — fracture-pt

How to set up, run, debug, and extend the gethacked.eu portal locally.

## Prerequisites

- **Rust 1.94+** (matches CI). `rustup` recommended.
- **Podman** (the project uses podman-compose, not docker-compose). Rootless ok.
- **Git** with access to `Sp0Q1/fracture-pt` and `Sp0Q1/fracture-cms`.

The Rust toolchain is installed locally for fast iteration; podman is reserved for the running app stack and semgrep. See the global rule in `~/.claude/CLAUDE.md`.

## First-time setup

```bash
# 1. Clone
git clone https://github.com/Sp0Q1/fracture-pt
cd fracture-pt

# 2. Generate dev secrets and bring the stack up
./dev/setup.sh                # creates .env, generates JWT_SECRET, etc.
podman compose up -d          # zitadel + mailcrab + app

# 3. Build pentext-docbuilder (one-off; needed for report PDFs)
./dev/build-docbuilder.sh

# 4. Visit
open http://localhost:5150
```

The `setup.sh` script writes a `.env` file containing dev-only secrets. **Never commit `.env`.** A `.gitignore` and a `.semgrepignore` already exclude it.

## Daily workflow

| Task | Command |
|---|---|
| Format check | `cargo fmt --all -- --check` |
| Format apply | `cargo fmt --all` |
| Strict lint (matches CI) | `cargo clippy --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms` |
| Build | `cargo build` |
| Fast type check | `cargo check` |
| Run tests | `DATABASE_URL=sqlite:///tmp/gethacked_test.sqlite?mode=rwc cargo test --all-features --all` |
| Full local CI | `./dev/ci.sh` (runs fmt + clippy + semgrep + tests) |
| Audit | `cargo audit --ignore RUSTSEC-2023-0071` |
| Run app stack | `podman compose up -d` |
| Rebuild app container | `podman compose down app && podman compose build app && podman compose up -d app` |
| App logs | `podman compose logs -f app` |
| Stop everything | `podman compose down` |
| DB shell | `sqlite3 /home/<user>/.local/share/containers/storage/volumes/gethacked_app_data/_data/gethacked.sqlite` (or `psql` for postgres) |

`./dev/ci.sh` MUST pass before opening a PR. CI in `.github/workflows/rust.yml` runs the same checks plus `cargo audit`.

## Debugging

### Logs

`tracing` is structured-log first. In dev, logs go to stdout in compact format; level set in `config/development.yaml` (`level: debug`).

Search for a specific request:
```bash
podman compose logs app | grep -E 'request_id|engagement_id'
```

A successful job execution emits a single `Job completed successfully` line at INFO with `job_run_id`, `job_type`, and `diffs` count. A failed job emits `Job execution failed` at ERROR with the error message.

### Inspecting the database

The dev DB lives on a podman volume. Mount it once into a one-shot container or use the host path (depends on rootless config):

```bash
podman volume inspect gethacked_app_data
sqlite3 <path>/gethacked.sqlite ".tables"
sqlite3 <path>/gethacked.sqlite "SELECT pid, status, hostname FROM scan_targets LIMIT 5;"
```

For postgres in prod-like setups, use `psql` against the configured `DATABASE_URL`.

### Browser

Always check the browser dev console while exercising a UI change. Any new CSP violation is a regression and blocks the PR. Inline scripts/styles violate CSP — if you need behavior, attach a `data-*` attribute and handle it in `assets/static/app.js`.

### OIDC issues

The `JWT_SECRET` env var must be a base64-decodable string. The `setup.sh` script generates one correctly; if you see "invalid JWT" errors after manual edits, regenerate via:
```bash
openssl rand -base64 32
```
and update `.env`.

If OIDC silently does nothing, check `config/development.yaml` — the OIDC initializer in fracture-core currently disables OIDC when `client_id` / `issuer` / `client_secret` are missing. (A pending CMS PR makes this loud.)

### Slow tests

Tests are `#[serial]` to avoid SQLite write conflicts. The full PT suite runs in ~12 s. If a single test hangs, it's almost always a `loco_rs::testing::boot_test` timeout — check that the test DB path in `DATABASE_URL` is writable.

## Adding a new pentest tool

Follow the `Adding a new pentest tool` checklist in [`CLAUDE.md`](../CLAUDE.md). Key points:

1. Author a sidecar `Containerfile` under `containers/tools/<tool>/`. Pin the binary version.
2. Add a runner in `src/services/<tool>.rs` — argv only, validates input via the shared validator.
3. Add a `JobExecutor` in `src/jobs/<tool>.rs`. Output → `result_summary` (parsed) and optionally `result_output` (raw, truncated).
4. Register the executor in `src/app.rs::routes()`.
5. Tier-classify in [`SCAN_AUTHZ.md`](SCAN_AUTHZ.md) — passive or active.
6. Wire the active/passive classification into the scan-authz check at the call site.
7. Add the stage to the pipeline graph (when the orchestrator lands).
8. Add a parsed-output template fragment in `assets/views/jobs/<tool>.html`.
9. Tests: parser golden fixtures, executor integration test with mocked sidecar, controller IDOR test.
10. Update `ARCHITECTURE.md` and add a manual-test entry in `MANUAL-TEST-PLAN.md`.

## Adding a migration

```bash
# 1. Create a new migration file
touch migration/src/m20260YYMMDD_NNNNNN_<name>.rs

# 2. Implement up() and down(). Existing migrations are good templates.
#    For org-owned tables, REQUIRED:
#    - org_id integer NOT NULL with FK to organizations.id ON DELETE CASCADE
#    - index on org_id
#    - pid uuid NOT NULL with unique index

# 3. Register in migration/src/lib.rs (mod m... + Box::new(...))

# 4. Run tests; auto-migration applies on test boot
DATABASE_URL=sqlite:///tmp/gethacked_test.sqlite?mode=rwc cargo test --all-features --all
```

For domain models, also implement `super::OrgScoped` in the model file (provided by fracture-core after PR #69 lands):

```rust
impl super::OrgScoped for Entity {
    fn org_id_column() -> Self::Column {
        Column::OrgId
    }
}
```

## Manual local testing (mandatory)

Per `CLAUDE.md`, you must exercise the change in a running browser before marking work complete. For changes touching scan tools or the worker:

1. Rebuild affected containers: `podman compose build app worker && podman compose up -d`
2. Tail logs: `podman compose logs -f app worker`
3. Walk the affected page in a browser. Check golden + edge case + browser console for CSP regressions.
4. For RBAC work, log in as users with each affected role and confirm both positive and negative cases.
5. For migration work, confirm `auto_migrate` runs on a fresh DB *and* on a DB seeded from the prior schema.

If you can't manually test (missing IdP creds, etc.), say so explicitly in the PR description. Do not claim success on `cargo test` alone.

## File-system layout

```
fracture-pt/
├── CLAUDE.md                  # invariants for future contributors
├── README.md                  # high-level + quick start
├── docs/
│   ├── ARCHITECTURE.md        # this is the doc you should read second
│   ├── DEVELOPMENT.md         # this file
│   ├── SCAN_AUTHZ.md          # passive vs active scan gate
│   └── (USE_CASES.md, THREAT_MODEL.md — being added)
├── src/
│   ├── app.rs                 # AppHooks, route registration, seeding
│   ├── lib.rs                 # public crate surface
│   ├── bin/main.rs            # CLI entry point
│   ├── controllers/           # HTTP handlers
│   ├── models/                # domain model + SeaORM entities (in _entities/)
│   ├── services/              # tool runners, scan_authz, tier, asm, port_scan, ...
│   ├── jobs/                  # JobExecutor implementations
│   ├── workers/               # job dispatcher + scheduler
│   ├── views/                 # Tera context builders
│   ├── initializers/          # security headers, OIDC, view engine, SQLite pragmas
│   └── mailers/               # email mailers
├── migration/                 # SeaORM migrations
├── assets/
│   ├── views/                 # Tera templates
│   ├── static/                # CSS / JS (with SRI)
│   ├── i18n/                  # Fluent translations
│   └── uploads/               # runtime uploads (ignored)
├── config/                    # development.yaml / production.yaml / test.yaml
├── dev/                       # ci.sh, setup.sh, build-docbuilder.sh, entrypoint.sh
├── tests/
│   ├── models/                # model-layer tests
│   ├── requests/              # integration tests through the HTTP layer
│   └── services/              # service-layer tests (e.g. scan_authz)
├── compose.yaml               # podman compose stack
├── Containerfile.dev          # dev container
├── Containerfile.prod         # prod multi-stage build
└── .github/workflows/         # CI
```

## Performance & disk hygiene

The `target/` directory grows quickly (debug test binaries are large). Periodically:
```bash
cargo clean
```
or use `cargo sweep` if installed. CI caches `~/.cargo/registry`, `~/.cargo/git`, and `target/` keyed by `Cargo.lock` hash.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `cargo test` hangs | DB path not writable | Check `DATABASE_URL` |
| `JWT_SECRET` rejected | Not base64 | Regenerate with `openssl rand -base64 32` |
| OIDC redirect loop | Zitadel not reachable | `podman compose ps zitadel`; check ports |
| Migration fails on boot | Schema mismatch from a prior dev DB | Drop the dev volume: `podman volume rm gethacked_app_data` (DESTROYS DEV DATA) |
| Clippy "too many lines" on a migration | Migration's `up()` body is large | Add `#[allow(clippy::too_many_lines)]` with a one-line justification (migrations are inherently long) |
| Semgrep flags a finding | Real or false positive | Fix the code, or add `// nosemgrep: <rule-id> -- <reason>` immediately above the line with a justification |
| CSP violation in browser | Inline script/style | Move to `/static/`; never relax CSP |
