# Technology Stack: project-switch

## Primary Language

- **Rust** (Edition 2021)
- Chosen for near-zero startup latency and compiled binary distribution

## Main CLI (`project-switch`)

| Category | Crate | Version | Purpose |
|---|---|---|---|
| CLI Framework | clap | 4.5 | Command parsing with derive macros |
| Serialization | serde | 1.0 | YAML config deserialization |
| Serialization | serde_yaml | 0.9 | YAML format support |
| Terminal UI | inquire | 0.7.5 | Interactive selection prompts |
| Terminal UI | colored | 2.1 | Colored terminal output |
| Error Handling | anyhow | 1.0 | Ergonomic error propagation |
| Platform Paths | dirs | 5.0 | Cross-platform home/config directories |
| Utilities | meval | 0.2 | Math expression evaluation |
| Utilities | urlencoding | 2.1 | URL-encoding for arguments |
| Build | winresource | 0.1 | Windows executable resource embedding |

## Hotkey Service (`project-switch-hotkey`)

| Category | Crate | Version | Purpose |
|---|---|---|---|
| System Tray | tray-icon | 0.19 | Notification area icon |
| System Tray | muda | 0.15 | Right-click context menu |
| Hotkey | global-hotkey | 0.7 | Global hotkey registration |
| Windowing | tao | 0.34 | Event loop (minimal, no default features) |
| Config | serde_yaml | 0.9 | YAML config reading |
| Platform Paths | dirs | 5.0 | Cross-platform home/config directories |
| macOS | objc2-core-foundation | 0.3 | macOS platform bindings |
| Windows | windows | 0.58 | Win32 API bindings (WindowsAndMessaging) |
| Build | winresource | 0.1 | Windows executable resource embedding |

## Build & Distribution

- **Docker** cross-compilation pipeline producing platform-specific binaries
- **Targets:** Windows (`bin/windows/project-switch.exe`, `bin/windows/project-switch-hotkey.exe`), Linux (`bin/linux/project-switch`)
- **Release profile:** LTO enabled, single codegen unit, panic=abort, symbols stripped

## Tooling (Non-Product)

- **Node.js / TypeScript** — commit scripts only (`scripts/commit.ts`), not part of the shipped product
- **Docker Compose** — orchestrates the build pipeline

## Target Platforms

- **Windows** — primary, full support (CLI + hotkey service)
- **macOS** — primary, full support (CLI + hotkey service)
- **Linux** — secondary, CLI binary produced but hotkey service is Windows/macOS focused
