When running the program, use powershell. See README.md for usage and configuration details.

## Programs

This repo produces two Rust binaries:

### project-switch (src/)

The main CLI tool for managing and switching between projects. Built with clap.

Subcommands: `switch`, `add`, `current`, `open` (deprecated/hidden), `list`.

### project-switch-hotkey (hotkey/)

A background Windows service that registers a global ALT+SPACE hotkey. When triggered, it kills any running `project-switch.exe` instances and launches `project-switch list` in Windows Terminal.

## Building

To build the project, use Docker Compose:

```powershell
./build.ps1
```

Building should happen after a set of changes are implemented to confirm no compilation errors.

This will create binaries in the `bin/` directory:

- `bin/windows/project-switch.exe` - Windows executable
- `bin/windows/project-switch-hotkey.exe` - Windows hotkey listener
- `bin/linux/project-switch` - Linux executable
