#!/usr/bin/env bash
set -euo pipefail

echo "Removing bin folder..."
rm -rf bin
echo "bin folder removed."

echo "Building Docker container and running build service..."

docker compose build
docker compose run --rm build

echo "Build completed successfully!"
