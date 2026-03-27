# Spec: Migrate Launcher from CLI to Native Windowed UI

## Overview

Replace the current terminal-based launcher (`inquire` in Windows Terminal) with a native windowed UI. The hotkey service currently spawns `wt.exe` running `project-switch list`; after this migration, it will instead show/hide a lightweight native window directly. The core business logic (config loading, command resolution, browser/shortcut launching) remains unchanged.

## Current Architecture

### project-switch (CLI)
- `src/commands/list.rs` — the launcher. Uses `inquire::Text` with a custom `Autocomplete` implementation (`ListAutocomplete`) to show a filterable list of commands, shortcuts, file paths, and a calculator mode.
- `src/config.rs` — `ConfigManager` loads and merges YAML configs, resolves current project, provides command/shortcut data.
- `src/utils/browser.rs` — opens URLs in browsers, runs terminal commands, launches shortcuts.
- `src/utils/shortcuts.rs` — scans Start Menu / Applications for launchable shortcuts.

### project-switch-hotkey (system tray)
- Registers Alt+Space (Windows) / Cmd+Space (macOS) via `global-hotkey`.
- On hotkey press, kills existing `project-switch.exe` instances and spawns `wt.exe -- cmd /c project-switch.exe list`.
- System tray icon with context menu (Open, Shortcuts toggle, Exit).

## Target Architecture

### Merged Binary
The hotkey service and the launcher window live in a single process. On hotkey press, the window is shown/focused. On action or Escape, the window is hidden. No terminal is involved.

### UI Framework
Use `iced` (Rust-native, cross-platform GUI library). It supports:
- Custom rendering (not platform-native widgets, but minimal and fast)
- Text input with keyboard handling
- Scrollable lists
- Window show/hide control
- Both Windows and macOS

Alternative: `egui` via `eframe`. Lighter weight, immediate-mode. Better for this use case (simple input + list).

**Decision deferred to Phase 1 research task.** The spec is framework-agnostic.

## Functional Requirements

### FR-1: Launcher Window
- A single text input at the top.
- A filtered list below, updating in real-time as the user types.
- List items show: key, destination (URL/command/app), styled by type (command vs shortcut vs file path).
- Selected item is visually highlighted.

### FR-2: Keyboard Interaction
- **Arrow Up/Down:** Navigate the list.
- **Enter:** Execute the selected item (open URL, run command, launch shortcut).
- **Escape:** Hide the window without action.
- **Typing:** Filters the list by keyword match. Must handle touch-typing speed with zero perceived lag.

### FR-3: Calculator Mode
- Input starting with `=` shows the evaluated result inline (e.g., `=5+3` shows `= 8`).
- Enter on a calculator result copies it to clipboard (or just displays it).

### FR-4: File Path Mode
- Input that looks like a file path (drive letter or UNC) shows filesystem suggestions.
- Tab-completing into a directory auto-expands its contents.
- Enter opens the selected path.

### FR-5: Context Display
- The window title or a subtle label shows the current project name.
- Commands are resolved per the active project context, same as today.

### FR-6: Hotkey Activation
- Alt+Space (Windows) / Cmd+Space (macOS) toggles the window.
- If hidden: show, focus, clear input, populate list with all items.
- If visible: hide.

### FR-7: System Tray
- Retain the system tray icon with context menu (Open, Shortcuts toggle, Exit).
- "Open" triggers the same show behavior as the hotkey.

### FR-8: Existing CLI Commands
- `project-switch switch`, `add`, `current` remain as CLI subcommands (terminal-based, unchanged).
- `project-switch list` can optionally still work in terminal mode for scripting/debugging, controlled by a flag or auto-detected (if no GUI environment).

## Non-Functional Requirements

### NFR-1: Startup Latency
- Window must appear within 50ms of hotkey press (perceived instant).

### NFR-2: Keystroke Responsiveness
- List filtering must complete within a single frame (~16ms) to avoid dropped keystrokes or visible lag.

### NFR-3: Memory Footprint
- Idle (hidden window) should use minimal memory. Target: <20MB resident.

### NFR-4: Platform Parity
- Windows and macOS must have feature parity for the launcher window, hotkey, and system tray.

### NFR-5: Build
- The merged binary must be producible via the existing Docker cross-compilation pipeline.

## Out of Scope

- Rich visual themes or customization (colors, fonts, etc.) — use defaults.
- Mouse-driven workflows — keyboard-only is the primary interaction model.
- Plugin system or extensibility.
- Migrating `switch`, `add`, `current` to windowed UI.
