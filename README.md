# GetHacked

EU-focused penetration testing and attack surface management platform. Built on [fracture-core](https://github.com/Sp0Q1/fracture-cms) (Rust, Loco framework, SeaORM, Tera templates).

Clients scope engagements, track findings in real time, and download reports — all from a single dashboard. Pentesters log findings as they go. Authentication is handled by an external OIDC provider (Zitadel).

## Prerequisites

- [Podman](https://podman.io/) and `podman compose`
- `curl`, `jq` (for the setup script)
- `openssl` (for generating secrets)

All build, test, and run commands go through containers. No local Rust toolchain required.

## Quick Start (Development)

```bash
# 1. Clone the repository
git clone <repo-url> && cd gethacked

# 2. Run the automated setup script
#    This starts Zitadel + Postgres + MailCrab, creates an OIDC app,
#    generates a JWT secret, and writes .env for you.
./dev/setup.sh

# 3. Start the remaining services (MailCrab + app)
podman compose up -d mailcrab app

# 4. Open the app
open http://localhost:5150
```

The setup script creates a test user (`testuser` / `TestPassword1!`) and generates all secrets automatically.

### Development Services

| Service | URL | Purpose |
|---|---|---|
| App | http://localhost:5150 | The application |
| Zitadel (IdP) | http://localhost:8080 | OIDC identity provider |
| MailCrab | http://localhost:1080 | Email trap (catches all outbound mail) |

### Development Environment Variables

The setup script writes `.env` with generated values. You can also create it manually:

```bash
cp .env.example .env

# Generate a cryptographically secure JWT secret (min. 32 bytes):
openssl rand -base64 32
# Paste the output as JWT_SECRET in .env
```

| Variable | Required | Description |
|---|---|---|
| `JWT_SECRET` | Yes | HMAC signing key for session JWTs. **Must be at least 32 bytes of random data.** Generate with `openssl rand -base64 32`. |
| `OIDC_CLIENT_ID` | Yes | OIDC client ID from your identity provider. |
| `OIDC_CLIENT_SECRET` | Yes | OIDC client secret. Treat as a password. |
| `OIDC_PROJECT_ID` | No | Zitadel project ID (optional, used for audience validation). |
| `OIDC_ISSUER_URL` | No | OIDC issuer URL. Default: `http://localhost:8080` |
| `OIDC_REDIRECT_URI` | No | OAuth callback URL. Default: `http://localhost:5150/api/auth/oidc/callback` |
| `OIDC_POST_LOGOUT_REDIRECT_URI` | No | Post-logout redirect. Default: `http://localhost:5150` |
| `DATABASE_URL` | No | Database connection string. Default: `sqlite://gethacked_development.sqlite?mode=rwc` |
| `MAILER_HOST` | No | SMTP host. Default: `localhost` |
| `MAILER_PORT` | No | SMTP port. Default: `1025` |

### Platform Admin Access

Platform admin privileges (engagement management, pentester assignment) require membership in the `gethacked-admin` org, which is seeded automatically by the database migration.

After logging in as `testuser` at least once (so the user exists in the local DB), grant admin access:

```bash
podman exec gethacked_app_1 sqlite3 /app/data/gethacked_development.sqlite \
  "INSERT INTO org_members (org_id, user_id, role, created_at, updated_at)
   SELECT o.id, u.id, 'owner', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
   FROM organizations o, users u
   WHERE o.slug = 'gethacked-admin' AND u.email = 'testuser@example.com';"
```

The `gethacked-admin` slug is reserved — no user can create an org with this slug (enforced by a unique database constraint).

### Rebuilding After Changes

```bash
# Full rebuild (code, templates, static assets):
podman compose down && podman compose build app && podman compose up -d

# Template/CSS-only changes in release mode require a container restart:
podman restart gethacked_app_1
```

### Running CI Locally

```bash
./dev/ci.sh
```

Runs rustfmt, clippy (pedantic + nursery), semgrep, and all tests inside containers.

## Production Deployment

### 1. Generate Secrets

All secrets must be generated from a cryptographically secure source. Never reuse development values.

```bash
# Generate JWT secret (32 bytes, base64-encoded):
openssl rand -base64 32

# Generate Zitadel master key (must be exactly 32 ASCII characters):
openssl rand -hex 16
```

Store secrets in your deployment platform's secret manager (e.g. Kubernetes Secrets, Vault, cloud provider secret store). Do **not** commit secrets to the repository or store them in plain-text files on disk.

### 2. Environment Variables

```bash
# --- Required ---
JWT_SECRET=<output of: openssl rand -base64 32>
OIDC_CLIENT_ID=<from your identity provider>
OIDC_CLIENT_SECRET=<from your identity provider>
OIDC_ISSUER_URL=https://auth.example.com
OIDC_REDIRECT_URI=https://gethacked.eu/api/auth/oidc/callback
OIDC_POST_LOGOUT_REDIRECT_URI=https://gethacked.eu
DATABASE_URL=postgres://user:password@db-host:5432/gethacked

# --- Mail ---
MAILER_HOST=smtp.example.com
MAILER_PORT=587
MAILER_USER=<smtp username>
MAILER_PASSWORD=<smtp password>

# --- Optional ---
APP_URL=https://gethacked.eu
PORT=5150
SERVER_BINDING=0.0.0.0
OIDC_PROJECT_ID=<zitadel project id>
OIDC_PROVIDER_NAME=zitadel
```

### 3. Build the Container Image

```bash
podman build -f Containerfile.prod -t gethacked:latest .
```

The image is a multi-stage Rust build that produces a release binary in a minimal Debian runtime image. Database migrations run automatically on startup.

### 4. Run

```bash
podman run -d \
  --name gethacked \
  -p 5150:5150 \
  --env-file /path/to/production.env \
  -v /path/to/config:/app/config:ro \
  -v /path/to/assets:/app/assets:ro \
  gethacked:latest
```

Or with compose — adapt `compose.yaml` for production by:
- Replacing the Zitadel dev instance with your production OIDC provider
- Replacing MailCrab with a real SMTP relay
- Using PostgreSQL instead of SQLite
- Mounting secrets from your secret manager instead of `.env`

### 5. Security Checklist

- [ ] `JWT_SECRET` is at least 32 bytes of cryptographically random data (`openssl rand -base64 32`)
- [ ] OIDC client secret is stored in a secret manager, not in environment files on disk
- [ ] Database credentials are stored in a secret manager
- [ ] SMTP credentials are stored in a secret manager
- [ ] HTTPS is terminated at a reverse proxy (nginx, Caddy, cloud LB) in front of the app
- [ ] `APP_URL` uses `https://`
- [ ] `OIDC_REDIRECT_URI` and `OIDC_POST_LOGOUT_REDIRECT_URI` use `https://`
- [ ] Database is not exposed to the public internet
- [ ] Container runs as a non-root user in production
- [ ] Static assets are served through a CDN or reverse proxy with caching headers

## Project Structure

```
config/              Loco YAML configs (development, production, test)
assets/
  views/             Tera HTML templates
  static/            CSS, JS, images
  i18n/              Fluent translation files (en-GB, de-DE)
src/
  controllers/       Route handlers
  models/            SeaORM entity models
  views/             Template render functions
  initializers/      App startup hooks (OIDC, security headers)
migration/           SeaORM database migrations
dev/                 Development scripts and CI tooling
```

## License

AGPL-3.0-or-later
