# Product Guide: project-switch

## Initial Concept

A fast, keyboard-driven app launcher for developers who work across multiple projects.

## Product Overview

project-switch is a cross-platform app launcher designed for software developers and power users who demand instant, keyboard-driven access to project-specific URLs, tools, and commands. Activated via a global hotkey (Alt+Space on Windows, Cmd+Space on macOS), it provides a launcher experience where the same keyword can resolve to different destinations depending on the currently active project context.

The tool is built in Rust for near-zero startup latency, ensuring the launcher feels native and never misses a keystroke — even at touch-typing speed.

## Target Users

- **Software developers** who work across multiple codebases and need fast, context-aware access to project URLs, documentation, CI dashboards, and commands.
- **Keyboard-driven power users** who prefer launching tools and navigating workflows without reaching for the mouse.

## Core Capabilities

### Primary: App Launcher (`list`)

The heart of the product. When triggered via the global hotkey, the launcher presents a filterable list of commands for the current project. The user types to filter, selects an entry, and the corresponding URL is opened or command is executed. The interaction must be fast enough to support touch typing without dropped keystrokes.

### Secondary: Context Switching (`switch`)

Allows the user to change the active project context. Once switched, all launcher keywords resolve against the new project's command set. This enables a single muscle-memory workflow (e.g. typing "ci" + Enter) to open different URLs depending on which project is active.

### Configuration & Portability

- YAML-based configuration (`~/.project-switch.yml`) with a hierarchical include/merge system.
- Shared project definitions can live in a dotfiles repo and be included from the local config, enabling consistent setups across multiple workstations.
- Merge rules allow machine-specific overrides (paths, browsers) while inheriting shared project commands.

## Platform Support

- **Windows** — global hotkey via Alt+Space, launched in Windows Terminal
- **macOS** — global hotkey via Cmd+Space, with platform-appropriate launcher behavior

## Distribution

Self-compiled from source using Docker cross-compilation. Produces platform-specific binaries for Windows and Linux/macOS.

## Product Vision

project-switch will remain a lean, focused launcher — doing one thing well with minimal feature creep. The planned evolution is a transition from the current CLI-based interface to a native windowed UI, delivering a more polished launcher experience while preserving the speed and simplicity of the underlying tool.
