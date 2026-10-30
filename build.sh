#!/usr/bin/env bash
set -euo pipefail

if [ -n "${WSL_DISTRO_NAME:-}" ] || grep -qi microsoft /proc/version 2>/dev/null; then
    echo "Refusing to build in WSL. Run build.ps1 from a Windows checkout." >&2
    exit 1
fi

# Detect host OS and set build target
case "$(uname)" in
    Darwin) export BUILD_TARGET=macos ;;
    *)      export BUILD_TARGET=linux ;;
esac

STAGED_DIR="bin/.staging/$BUILD_TARGET"
BIN_DIR="bin/$BUILD_TARGET"

echo "Building for $BUILD_TARGET..."
docker compose build build
docker compose run --rm build

if [ -z "$(ls -A "$STAGED_DIR" 2>/dev/null)" ]; then
    echo "Build produced no files in $STAGED_DIR" >&2
    exit 1
fi

echo "Build completed successfully!"

# The hotkey runs straight from bin/macos, so it must be stopped before its binary is replaced
if [ "$BUILD_TARGET" = "macos" ]; then
    echo "Stopping project-switch-hotkey..."
    pkill -x project-switch-hotkey || true
    for _ in $(seq 1 50); do
        pgrep -x project-switch-hotkey >/dev/null || break
        sleep 0.1
    done
fi

echo "Updating $BIN_DIR..."
rm -rf "$BIN_DIR"
mv "$STAGED_DIR" "$BIN_DIR"
rm -rf bin/.staging

# Zigbuild's linker-signed ad-hoc signature is rejected at launch (SIGKILL, bug_type 309), so re-sign natively
if [ "$BUILD_TARGET" = "macos" ]; then
    echo "Ad-hoc signing macOS binaries..."
    codesign --force --sign - "$BIN_DIR/project-switch"
    codesign --force --sign - "$BIN_DIR/project-switch-hotkey"
    echo "Restarting hotkey service..."
    bash "$BIN_DIR/start-hotkey.sh"
fi
