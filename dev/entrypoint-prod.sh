#!/bin/sh
set -e

# Validate data directory
if [ ! -d /app/data ]; then
    echo "FATAL: /app/data does not exist. Mount a volume with -v /host/path:/app/data"
    exit 1
fi

if [ ! -w /app/data ]; then
    echo "FATAL: /app/data is not writable by UID $(id -u)."
    echo "  Fix: chown $(id -u):$(id -g) /host/path/to/data"
    exit 1
fi

echo "Data directory OK: /app/data (writable)"
echo "DATABASE_URL: ${DATABASE_URL:-<not set, using config default>}"

exec ./gethacked-cli start
