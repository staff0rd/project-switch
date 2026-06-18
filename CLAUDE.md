When running the program, use powershell. See README.md for usage and configuration details.

## Comments

Prefer self-descriptive code over comments. Do not write comments that restate what the code already says. Only add a comment when something genuinely would not make sense without it (a non-obvious why, a hack, a workaround). No multi-line comment blocks on small functions — one terse line at most.

## Programs

This repo produces two Rust binaries:

### project-switch (src/)

The main CLI tool for managing and switching between projects. Built with clap.

Subcommands: `switch`, `add`, `current`, `open` (deprecated/hidden), `list`.

### project-switch-hotkey (hotkey/)

A background Windows service that registers a global ALT+SPACE hotkey. When triggered, it kills any running `project-switch.exe` instances and launches `project-switch list` in Windows Terminal.

- Standalone crate (not a workspace member), built through the Docker pipeline
- System tray app (`#![windows_subsystem = "windows"]`) — no console window
- Uses `tray-icon` + `muda` crates for notification area icon with right-click "Exit" menu
- Uses `RegisterHotKey` with `MOD_ALT | MOD_NOREPEAT` and `PeekMessageW` polling with 50ms sleep
- Auto-start: create a shortcut in `shell:startup` (Win+R -> `shell:startup`)

## Building

After every `/verify` that passes, always run `/build` immediately — no exceptions, do not wait to be asked.

This will create binaries in the `bin/` directory:

- `bin/windows/project-switch.exe` - Windows executable
- `bin/windows/project-switch-hotkey.exe` - Windows hotkey listener
- `bin/linux/project-switch` - Linux executable
