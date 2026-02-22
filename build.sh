#!/usr/bin/env bash
set -euo pipefail

# Detect host OS and set build target
case "$(uname)" in
    Darwin) export BUILD_TARGET=macos ;;
    *)      export BUILD_TARGET=linux ;;
esac

# On macOS, kill the running hotkey service before rebuilding
if [ "$BUILD_TARGET" = "macos" ]; then
    pkill -f project-switch-hotkey || true
fi

echo "Building for $BUILD_TARGET..."
echo "Removing bin folder..."
rm -rf bin

docker compose build build
docker compose run --rm build

echo "Build completed successfully!"

# On macOS, restart the hotkey service
if [ "$BUILD_TARGET" = "macos" ]; then
    echo "Restarting hotkey service..."
    bash bin/macos/start-hotkey.sh
fi
