#!/bin/sh
set -eu

TARGET="${BUILD_TARGET:-linux}"

case "$TARGET" in
    windows)
        echo "Building for Windows..."
        cargo build --release --target x86_64-pc-windows-gnu
        cd hotkey && cargo build --release --target x86_64-pc-windows-gnu && cd ..
        mkdir -p /output/windows
        cp target/x86_64-pc-windows-gnu/release/project-switch.exe /output/windows/
        cp hotkey/target/x86_64-pc-windows-gnu/release/project-switch-hotkey.exe /output/windows/
        ;;
    linux)
        echo "Building for Linux..."
        cargo build --release --target x86_64-unknown-linux-gnu
        mkdir -p /output/linux
        cp target/x86_64-unknown-linux-gnu/release/project-switch /output/linux/
        ;;
    macos)
        echo "Building for macOS (aarch64)..."
        cargo zigbuild --release --target aarch64-apple-darwin
        cd hotkey && cargo zigbuild --release --target aarch64-apple-darwin && cd ..
        mkdir -p /output/macos
        cp target/aarch64-apple-darwin/release/project-switch /output/macos/
        cp hotkey/target/aarch64-apple-darwin/release/project-switch-hotkey /output/macos/
        cp hotkey/start-hotkey.sh /output/macos/
        chmod +x /output/macos/start-hotkey.sh
        ;;
    *)
        echo "Unknown BUILD_TARGET: $TARGET"
        exit 1
        ;;
esac

echo "Build complete: $TARGET"
