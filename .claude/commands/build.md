---
description: Build the project via Docker cross-compilation
allowed-tools: Bash, PowerShell
---

Run the build. On Windows or WSL, run `pwsh ./build.ps1` (not `./build.sh`, which fails on the root-owned files in `bin/`). On macOS use `./build.sh`. This cross-compiles via Docker and produces binaries in `bin/`.

Show the full output to the user so they can see build progress and any errors. Don't pipe it through `tail` or `head`: the script starts `project-switch-hotkey.exe` at the end, which can hold the pipe open so the command never returns.
