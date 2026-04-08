# fracture-pt

Open-source penetration testing platform built on [fracture-core](https://github.com/Sp0Q1/fracture-cms). Manages engagements, findings, scan targets, reports, and team collaboration for security assessments.

## Architecture

```
fracture-core (library)     <- upstream CMS framework
  └── fracture-pt (this repo) <- pentest platform
        ├── src/controllers/   <- Axum route handlers
        ├── src/models/        <- SeaORM entities + business logic
        ├── src/jobs/          <- JobExecutor implementations (ASM scan, port scan, reports)
        ├── src/services/      <- External service integrations (crt.sh, nmap, report builder)
        ├── src/workers/       <- Loco background workers (job dispatcher)
        ├── migration/         <- SeaORM migrations (chained after fracture-core migrations)
        ├── assets/views/      <- Tera HTML templates (override fracture-core defaults)
        ├── assets/static/     <- CSS, JS (OatCSS framework)
        └── config/            <- Loco YAML config per environment
```

**Tech stack:** Rust, [Loco](https://loco.rs) (Axum), [SeaORM](https://www.sea-ql.org/SeaORM/), [Tera](https://keats.github.io/tera/) templates, SQLite (default) or PostgreSQL, OIDC authentication.

**From fracture-core:** Multi-tenant orgs with RBAC, OIDC auth, blog system, generic jobs system, file uploads, admin dashboard with entity registry.

**fracture-pt adds:** Engagements, findings, non-findings, scan targets, ASM/port scans, report generation (PDF via pentext-docbuilder), pentester assignments, comments, markdown editor, network map, dashboard, file uploads per engagement, subscription tiers.

## Features

- **Engagements** -- Create and manage penetration test engagements with scope, timeline, and team
- **Findings** -- Document vulnerabilities with severity (Extreme/High/Elevated/Moderate/Low), descriptions, and recommendations
- **Reports** -- Generate PDF reports from engagement findings
- **Scan targets** -- Define target hosts and networks for scanning
- **ASM scans** -- Automated attack surface mapping via crt.sh subdomain discovery
- **Port scans** -- Network port scanning via nmap
- **File uploads** -- Attach evidence files to engagements and findings
- **Markdown editor** -- Rich text editing for finding descriptions and recommendations
- **Comments** -- Threaded discussion on engagements and findings
- **Network map** -- Visual representation of discovered infrastructure
- **Dashboard** -- Overview of active engagements, recent findings, and scan status

### Tier System

| Feature | Free | Pro | Enterprise |
|---------|------|-----|------------|
| Engagements | 3 | Unlimited | Unlimited |
| Findings per engagement | 10 | Unlimited | Unlimited |
| Scan targets | 5 | 50 | Unlimited |
| ASM scans | -- | Yes | Yes |
| Port scans | -- | Yes | Yes |
| Report generation | -- | Yes | Yes |
| Team members | 1 | 5 | Unlimited |

## Prerequisites

- [Rust](https://rustup.rs/) 1.94+ (for local development)
- [Podman](https://podman.io/) and `podman-compose` (for running the dev stack)
- `curl`, `openssl` (used by setup script)

## Quick Start

```bash
git clone https://github.com/Sp0Q1/fracture-pt.git && cd fracture-pt
./dev/setup.sh                       # starts Zitadel + MailCrab, provisions OIDC app
podman compose up -d mailcrab app    # build and start app
```

Open http://localhost:5150 -- log in with `testuser` / `TestPassword1!`

| Service | URL |
|---|---|
| App | http://localhost:5150 |
| Zitadel (IdP) | http://localhost:8080 |
| MailCrab (SMTP) | http://localhost:1080 |

### Rebuild after code changes

```bash
podman compose down app && podman compose build app && podman compose up -d app
```

Database is persisted on the `app_data` volume -- survives rebuilds.

### CI

```bash
./dev/ci.sh
```

This runs rustfmt, clippy, and tests using local `cargo`, and semgrep via podman. Matches the GitHub Actions CI pipeline.

## Configuration

Configuration lives in `config/development.yaml` and `config/production.yaml`. Both support environment variable substitution (e.g., `{{get_env(name="JWT_SECRET")}}`).

Key settings:
- `database.auto_migrate: true` -- runs pending migrations on startup
- `auth.jwt.secret` -- session signing key (from `JWT_SECRET` env var)
- OIDC settings -- all from environment variables

## Production Deployment

### Prerequisites

- Podman with `podman-compose`
- nginx with TLS (certbot)
- An OIDC provider (Zitadel, Keycloak, Auth0, etc.)
- [`fracture-ctl`](https://github.com/Sp0Q1/fracture-cms/releases) -- the deployment CLI

### Deploy

```bash
fracture-ctl init --image ghcr.io/sp0q1/fracture-pt:latest --repo https://github.com/Sp0Q1/fracture-pt.git
```

This generates `.env.prod` and `compose.prod.yaml`, and clones `assets/` and `config/` from the repo.

Edit `.env.prod` to configure OIDC and SMTP, then start:

```bash
fracture-ctl up
```

Migrations run automatically on startup (`auto_migrate: true`). The `up` command takes a pre-deploy backup automatically if the app is already running.

### First-time admin setup

After your first OIDC login, promote yourself to platform admin:

```bash
fracture-ctl admin set user@example.com
```

This gives you access to the admin dashboard and platform-wide settings.

### nginx

Proxy with TLS. The app sets its own security headers (CSP, HSTS, X-Frame-Options, etc.) -- do not duplicate them in nginx.

```nginx
server {
    listen 443 ssl http2;
    server_name yourdomain.com;

    ssl_certificate     /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:5150;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
    }
}
```

### Database backup and restore

```bash
fracture-ctl backup                        # creates backup-{timestamp}.sqlite
fracture-ctl backup -o my-backup.sqlite    # custom filename
fracture-ctl restore my-backup.sqlite      # prompts for confirmation
fracture-ctl restore my-backup.sqlite --yes  # skip confirmation
```

Both SQLite and PostgreSQL are supported. The tool auto-detects which database is configured.

### Updating

Template/CSS changes (no image rebuild):

```bash
cd assets && git pull && cd ..
fracture-ctl down && fracture-ctl up
```

New app version:

```bash
# Update APP_IMAGE in .env.prod, then:
fracture-ctl up    # pulls and deploys the new image
```

### Troubleshooting

| Problem | Fix |
|---|---|
| `Authentication Not Available` | Check OIDC env vars. Restart app after the IdP is ready. |
| `Invalid audiences` | Set `OIDC_PROJECT_ID` to the Zitadel project ID. |
| `No email claim` | Enable "ID Token User Info Assertion" in Zitadel OIDC app. |
| Container can't reach IdP | Add `extra_hosts: ["auth.domain:host-gateway"]` to compose. |
| `unauthorized!` on login | `JWT_SECRET` must be valid base64. |
| SQLite read-only errors | Check volume ownership matches container user (uid 1000). |

### fracture-ctl command reference

| Command | Description |
|---|---|
| `fracture-ctl init --image <img>` | Generate `.env.prod` and `compose.prod.yaml` |
| `fracture-ctl up` | Pull image, backup, and start services |
| `fracture-ctl down` | Stop all services |
| `fracture-ctl backup [-o file]` | Back up the database |
| `fracture-ctl restore <file> [--yes]` | Restore from backup |
| `fracture-ctl admin set <email>` | Promote user to platform admin |
| `fracture-ctl admin list` | List platform admins |
| `fracture-ctl update` | Self-update to latest release |

See [fracture-cms DEPLOYMENT.md](https://github.com/Sp0Q1/fracture-cms/blob/main/docs/DEPLOYMENT.md) for full deployment documentation.

## License

AGPL-3.0-or-later
