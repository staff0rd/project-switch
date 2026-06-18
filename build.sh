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

# On macOS, re-sign and restart the hotkey service. The cross-compiled
# (zigbuild) binaries carry a linker-signed ad-hoc signature that macOS's
# code-signing monitor rejects at launch (SIGKILL, crash bug_type 309), so
# re-sign them ad-hoc with the native codesign before running.
if [ "$BUILD_TARGET" = "macos" ]; then
    echo "Ad-hoc signing macOS binaries..."
    codesign --force --sign - bin/macos/project-switch
    codesign --force --sign - bin/macos/project-switch-hotkey
    echo "Restarting hotkey service..."
    bash bin/macos/start-hotkey.sh
fi
