#!/bin/bash
set -euo pipefail

# Builds the pinned pentest-tool sidecar images the job runners invoke via
# `podman run ... <image>` with `--pull=never`. Without these, scan jobs
# (amass_passive, port scans) fail with "exited with status 125" because
# podman can't start a missing image.
#
# The pinned tags MUST match the constants in the code:
#   - src/services/amass.rs      -> subfinder image
#   - src/services/port_scan.rs  -> nmap image
# Bump both together when updating a tool.

COMPOSE_ENGINE="${CONTAINER_ENGINE:-podman}"

SUBFINDER_IMAGE="ghcr.io/sp0q1/fracture-pt-subfinder:v2.6.6"
NMAP_IMAGE="ghcr.io/sp0q1/fracture-pt-nmap:7.97"

echo "==> Building ${SUBFINDER_IMAGE}"
$COMPOSE_ENGINE build -t "$SUBFINDER_IMAGE" containers/tools/subfinder/

echo "==> Building ${NMAP_IMAGE}"
$COMPOSE_ENGINE build -t "$NMAP_IMAGE" containers/tools/nmap/

echo ""
echo "Tool images built:"
$COMPOSE_ENGINE images --format "  {{.Repository}}:{{.Tag}}" | grep fracture-pt || true
echo ""
echo "Scan jobs (amass_passive, port scans) can now run."
