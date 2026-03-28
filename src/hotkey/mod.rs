// Global hotkey registration and system tray integration.
// Implementation: Phase 4 (Hotkey & System Tray Integration).
// Windows/macOS only — Linux falls back to CLI mode.

#[cfg(any(windows, target_os = "macos"))]
pub use global_hotkey;
#[cfg(any(windows, target_os = "macos"))]
pub use muda;
#[cfg(any(windows, target_os = "macos"))]
pub use tray_icon;
