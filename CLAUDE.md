When running the program, use powershell.

## Building

To build the project, use Docker Compose:

```powershell
./build.ps1
```

Building should happen after a set of changes are implemented to confirm no compilation errors.

This will create binaries in the `bin/` directory:

- `bin/windows/project-switch.exe` - Windows executable
- `bin/linux/project-switch` - Linux executable
