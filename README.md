# GetHacked

EU-focused penetration testing and attack surface management platform. Built on [fracture-core](https://github.com/Sp0Q1/fracture-cms) (Rust, Loco framework, SeaORM, Tera templates).

## Development

Prerequisites: [Podman](https://podman.io/), `podman-compose`, `curl`, `jq`, `openssl`.

```bash
git clone https://github.com/Sp0Q1/fracture-pt.git && cd fracture-pt
./dev/setup.sh                       # starts Zitadel + MailCrab, creates OIDC app
podman compose up -d mailcrab app    # start app
```

Open http://localhost:5150 — log in with `testuser` / `TestPassword1!`

| Service | URL |
|---|---|
| App | http://localhost:5150 |
| Zitadel | http://localhost:8080 |
| MailCrab | http://localhost:1080 |

### CI

```bash
./dev/ci.sh    # fmt, clippy, semgrep, tests — all via podman
```

### Rebuild after code changes

```bash
podman compose down && podman compose build app && podman compose up -d
```

## Production

### Prerequisites

- Podman with `podman-compose`
- nginx with TLS (e.g. certbot)
- A [Zitadel](https://zitadel.com) instance (self-hosted or managed)
- [`fracture-ctl`](https://github.com/Sp0Q1/fracture-cms/releases) — download from GitHub releases

### 1. Set up Zitadel

Use `compose.zitadel.yaml` or any existing Zitadel instance.

After Zitadel is running, create an OIDC application through the Zitadel console:

1. Create a **Project**
2. Add an **OIDC Application** with:
   - Type: **Web**, auth method: **Basic**
   - Redirect URI: `https://yourdomain.com/api/auth/oidc/callback`
   - Post-logout redirect: `https://yourdomain.com`
   - **Enable** "ID Token User Info Assertion" (Token tab)
3. Note the **Client ID**, **Client Secret**, and **Project ID**

### 2. Deploy the app

Generate config:

```bash
fracture-ctl init --image ghcr.io/sp0q1/fracture-pt:latest
```

Edit `.env.prod` with your OIDC credentials:

```
OIDC_ISSUER_URL=https://auth.yourdomain.com
OIDC_CLIENT_ID=<from Zitadel>
OIDC_CLIENT_SECRET=<from Zitadel>
OIDC_PROJECT_ID=<from Zitadel project>
OIDC_REDIRECT_URI=https://yourdomain.com/api/auth/oidc/callback
OIDC_POST_LOGOUT_REDIRECT_URI=https://yourdomain.com
```

> **Important:** `OIDC_ISSUER_URL` must not have a trailing slash.

Clone assets for volume-mounted templates:

```bash
git clone --depth 1 https://github.com/Sp0Q1/fracture-pt.git repo
ln -s repo/assets assets
ln -s repo/config config
```

If Zitadel runs on the same host, add to `compose.prod.yaml` under the app service (rootless podman can't reach the host's own public IP from bridge networks):

```yaml
    extra_hosts:
      - "auth.yourdomain.com:host-gateway"
```

Start:

```bash
fracture-ctl up
```

### 3. nginx

Proxy the app with TLS. Don't add security headers — the app sets them.

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

### Updating

Template/CSS changes (no image rebuild):

```bash
cd repo && git pull && cd ..
fracture-ctl down && fracture-ctl up
```

New app version (Rust code changes):

```bash
# Update image tag in both files
sed -i 's/fracture-pt:[0-9.]*/fracture-pt:X.Y.Z/' .env.prod compose.prod.yaml
fracture-ctl up
```

### Troubleshooting

| Problem | Fix |
|---|---|
| `Authentication Not Available` | Check OIDC env vars are set. Ensure `OIDC_ISSUER_URL` has no trailing slash. |
| `Invalid audiences` | Set `OIDC_PROJECT_ID` to the Zitadel project ID. |
| `No email claim in ID token` | Enable "ID Token User Info Assertion" in the Zitadel OIDC app settings. |
| Container can't reach Zitadel | Add `extra_hosts: ["auth.domain:host-gateway"]` to compose. |

## License

AGPL-3.0-or-later
