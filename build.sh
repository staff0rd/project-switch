#!/usr/bin/env bash
set -euo pipefail

# Detect host OS and set build target
case "$(uname)" in
    Darwin) export BUILD_TARGET=macos ;;
    *)      export BUILD_TARGET=linux ;;
esac

echo "Building for $BUILD_TARGET..."
echo "Removing bin folder..."
rm -rf bin

docker compose build build
docker compose run --rm build

echo "Build completed successfully!"
