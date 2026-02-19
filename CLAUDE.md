When running the program, use powershell. See README.md for usage and configuration details.

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

To build the project:

```
assist run build
```

Building should happen after a set of changes are implemented to confirm no compilation errors.

This will create binaries in the `bin/` directory:

- `bin/windows/project-switch.exe` - Windows executable
- `bin/windows/project-switch-hotkey.exe` - Windows hotkey listener
- `bin/linux/project-switch` - Linux executable
