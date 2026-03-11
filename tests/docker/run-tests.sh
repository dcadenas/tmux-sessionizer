#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

echo "Building test image..."
docker build -f tests/docker/Dockerfile -t tms-test .

echo "Running tests..."
docker run --rm tms-test "$@"
