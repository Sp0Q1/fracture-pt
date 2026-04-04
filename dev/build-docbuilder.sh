#!/bin/bash
set -euo pipefail

# Build the pentext-docbuilder image for PDF report generation.
# Run this once on each server before generating reports.

IMAGE="pentext-docbuilder:latest"
REPO="https://github.com/radicallyopensecurity/pentext-docker.git"
TMP_DIR=$(mktemp -d)

echo "==> Checking for existing image..."
if podman image exists "$IMAGE" 2>/dev/null; then
    echo "    Image already exists. Rebuild? [y/N]"
    read -r answer
    if [ "$answer" != "y" ] && [ "$answer" != "Y" ]; then
        echo "    Skipped."
        exit 0
    fi
fi

echo "==> Cloning pentext-docker..."
git clone --depth 1 "$REPO" "$TMP_DIR"

echo "==> Fixing image references for podman..."
sed -i 's|FROM eclipse-temurin|FROM docker.io/library/eclipse-temurin|g; s|FROM ubuntu|FROM docker.io/library/ubuntu|g' "$TMP_DIR/docbuilder/Dockerfile"

echo "==> Building docbuilder image (this may take a few minutes)..."
podman build -t "$IMAGE" -f "$TMP_DIR/docbuilder/Dockerfile" "$TMP_DIR/docbuilder/"

echo "==> Cleaning up..."
rm -rf "$TMP_DIR"

echo ""
echo "========================================"
echo "  pentext-docbuilder image ready"
echo "========================================"
echo ""
echo "  Image: $IMAGE"
echo "  Size:  $(podman image inspect "$IMAGE" --format '{{.Size}}' | numfmt --to=iec 2>/dev/null || podman image inspect "$IMAGE" --format '{{.Size}}')"
echo ""
