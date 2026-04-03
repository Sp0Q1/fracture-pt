# fracture-pt

Penetration testing platform built on [fracture-core](https://github.com/Sp0Q1/fracture-cms). Rust, Loco framework, SeaORM, SQLite/PostgreSQL, Tera templates, OIDC authentication.

## Architecture

```
fracture-core (library)     ← upstream CMS framework
  └── fracture-pt (this repo) ← pentest platform
        ├── src/controllers/   ← Axum route handlers
        ├── src/models/        ← SeaORM entities + business logic
        ├── src/jobs/          ← JobExecutor implementations (ASM scan, port scan, reports)
        ├── src/services/      ← External service integrations (crt.sh, nmap, report builder)
        ├── src/workers/       ← Loco background workers (job dispatcher)
        ├── migration/         ← SeaORM migrations (chained after fracture-core migrations)
        ├── assets/views/      ← Tera HTML templates (override fracture-core defaults)
        ├── assets/static/     ← CSS, JS (OatCSS framework)
        └── config/            ← Loco YAML config per environment
```

**Key features from fracture-core:** Multi-tenant orgs with RBAC, OIDC auth, blog, generic jobs system, admin dashboard with entity registry.

**fracture-pt adds:** Engagements, findings, non-findings, scan targets, ASM scans, port scans, report generation, pentester assignments, subscriptions, invoices.

## Development

Prerequisites: [Podman](https://podman.io/), `podman-compose`, `curl`, `openssl`.

```bash
git clone https://github.com/Sp0Q1/fracture-pt.git && cd fracture-pt
./dev/setup.sh                       # starts Zitadel + MailCrab, provisions OIDC app
podman compose up -d mailcrab app    # build and start app
```

Open http://localhost:5150 — log in with `testuser` / `TestPassword1!`

| Service | URL |
|---|---|
| App | http://localhost:5150 |
| Zitadel (IdP) | http://localhost:8080 |
| MailCrab (SMTP) | http://localhost:1080 |

### Rebuild after code changes

```bash
podman compose down app && podman compose build app && podman compose up -d app
```

Database is persisted on the `app_data` volume — survives rebuilds.

### CI

```bash
./dev/ci.sh    # fmt, clippy, semgrep, tests — all via podman
```

## Production Deployment

### Prerequisites

- Podman with `podman-compose`
- nginx with TLS (certbot)
- [Zitadel](https://zitadel.com) instance (self-hosted or managed)
- [`fracture-ctl`](https://github.com/Sp0Q1/fracture-cms/releases)

### Deploy

```bash
fracture-ctl init --image ghcr.io/sp0q1/fracture-pt:latest
```

Edit `.env.prod`:

```
JWT_SECRET=<base64-encoded-32-byte-secret>
OIDC_ISSUER_URL=https://auth.yourdomain.com
OIDC_CLIENT_ID=<from Zitadel>
OIDC_CLIENT_SECRET=<from Zitadel>
OIDC_PROJECT_ID=<from Zitadel project>
OIDC_REDIRECT_URI=https://yourdomain.com/api/auth/oidc/callback
OIDC_POST_LOGOUT_REDIRECT_URI=https://yourdomain.com
```

> `OIDC_ISSUER_URL` must not have a trailing slash. `JWT_SECRET` must be valid base64.

Clone assets for volume mount:

```bash
git clone --depth 1 https://github.com/Sp0Q1/fracture-pt.git repo
ln -s repo/assets assets
ln -s repo/config config
```

Start:

```bash
fracture-ctl up
```

Migrations run automatically on startup (`auto_migrate: true`).

### nginx

Proxy with TLS. Don't add security headers — the app sets CSP, HSTS, X-Frame-Options, etc.

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

### Database Backup & Restore

SQLite database lives on the `fracture-pt_app_data` volume.

**Backup:**
```bash
fracture-ctl down
podman run --rm -v fracture-pt_app_data:/data -v $(pwd):/backup docker.io/library/alpine \
  cp /data/gethacked.sqlite /backup/gethacked-$(date +%Y%m%d).sqlite
fracture-ctl up
```

**Restore:**
```bash
fracture-ctl down
podman run --rm -v fracture-pt_app_data:/data -v $(pwd):/backup docker.io/library/alpine \
  cp /backup/gethacked-YYYYMMDD.sqlite /data/gethacked.sqlite
fracture-ctl up
```

### Updating

Template/CSS changes (no image rebuild):

```bash
cd repo && git pull && cd ..
fracture-ctl down && fracture-ctl up
```

New app version:

```bash
sed -i 's/fracture-pt:[0-9.]*/fracture-pt:X.Y.Z/' .env.prod compose.prod.yaml
fracture-ctl up
```

### Admin Setup

After first OIDC login, grant yourself platform admin access:

```bash
fracture-ctl down
# Fix volume ownership for sqlite3 tool
podman unshare chown 1000:1000 $(podman volume inspect fracture-pt_app_data --format '{{.Mountpoint}}') \
  $(podman volume inspect fracture-pt_app_data --format '{{.Mountpoint}}')/gethacked.sqlite

podman run --rm -v fracture-pt_app_data:/data docker.io/keinos/sqlite3 sqlite3 /data/gethacked.sqlite \
  "UPDATE organizations SET is_platform_admin = 1 WHERE slug = 'gethacked-admin';
   INSERT INTO org_members (org_id, user_id, role, created_at, updated_at)
   SELECT o.id, u.id, 'owner', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
   FROM organizations o, users u
   WHERE o.slug = 'gethacked-admin'
   AND NOT EXISTS (SELECT 1 FROM org_members WHERE org_id = o.id AND user_id = u.id);"

# Restore ownership
podman unshare chown 1000:1000 $(podman volume inspect fracture-pt_app_data --format '{{.Mountpoint}}') \
  $(podman volume inspect fracture-pt_app_data --format '{{.Mountpoint}}')/gethacked.sqlite
fracture-ctl up
```

### Troubleshooting

| Problem | Fix |
|---|---|
| `Authentication Not Available` | Check OIDC env vars. Restart app after Zitadel is ready. |
| `Invalid audiences` | Set `OIDC_PROJECT_ID` to the Zitadel project ID. |
| `No email claim` | Enable "ID Token User Info Assertion" in Zitadel OIDC app. |
| Container can't reach Zitadel | Add `extra_hosts: ["auth.domain:host-gateway"]` to compose. |
| `unauthorized!` on login | `JWT_SECRET` must be valid base64. |
| SQLite read-only errors | Check volume ownership matches the app container user (uid 1000). |

## License

AGPL-3.0-or-later
