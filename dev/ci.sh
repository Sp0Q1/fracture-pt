#!/bin/bash
# Run all CI checks locally. Uses local Rust toolchain for speed,
# podman only for semgrep (which needs its own image).
set -euo pipefail

SRC="$(cd "$(dirname "$0")/.." && pwd)"
SEMGREP_IMAGE="docker.io/semgrep/semgrep:latest"

# Warn if there are uncommitted changes
if [ -d "$SRC/.git" ] && ! git -C "$SRC" diff --quiet 2>/dev/null; then
    echo "WARNING: uncommitted changes detected -- local CI tests your"
    echo "   working tree, not what is committed. CI in GitHub will differ."
    echo ""
fi

passed=0
failed=0
failures=""

run_check() {
    local name="$1"
    shift
    echo ""
    echo "--- $name ---"
    if "$@"; then
        echo "[PASS] $name passed"
        passed=$((passed + 1))
    else
        echo "[FAIL] $name FAILED"
        failed=$((failed + 1))
        failures="$failures  - $name\n"
    fi
}

# --- rustfmt ---
run_check "rustfmt" \
    cargo fmt --all -- --check

# --- clippy ---
run_check "clippy" \
    cargo clippy --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms

# --- semgrep (still needs podman) ---
run_check "semgrep" \
    podman run --rm -v "$SRC:/src:ro" -w /src "$SEMGREP_IMAGE" \
    semgrep scan --config auto --error \
    --exclude-rule python.django.security.django-no-csrf-token.django-no-csrf-token .

# --- tests ---
run_check "test" \
    sh -c "DATABASE_URL=sqlite:///tmp/gethacked_test.sqlite?mode=rwc cargo test --all-features --all"

# --- summary ---
echo ""
echo "======================"
echo "  $passed passed, $failed failed"
if [ "$failed" -gt 0 ]; then
    echo ""
    echo "  Failures:"
    echo -e "$failures"
    echo "======================"
    exit 1
fi
echo "======================"
