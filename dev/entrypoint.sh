#!/bin/sh
set -e

# Wait for Zitadel OIDC discovery endpoint before starting the app.
# Prevents OIDC init failure when app boots before IdP is ready.
OIDC_URL="${OIDC_ISSUER_URL:-http://localhost:8080}"
echo "Waiting for OIDC provider at ${OIDC_URL}..."
for i in $(seq 1 30); do
    if curl -sf "${OIDC_URL}/.well-known/openid-configuration" > /dev/null 2>&1; then
        echo "OIDC provider ready."
        break
    fi
    sleep 2
done

exec ./target/debug/fracture-pt-cli start
