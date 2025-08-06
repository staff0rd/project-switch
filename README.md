# Project Switch (Rust)

Fast CLI tool to manage and switch between projects. Rewritten in Rust for blazing fast performance.

## Performance Comparison

- **Node.js version**: ~100-200ms startup time
- **Rust version**: ~1-5ms startup time ⚡

## Build

```bash
docker-compose up build
```

This creates binaries for both platforms:
- `bin/windows/project-switch.exe` 
- `bin/linux/project-switch`

## Install

**Windows:**
```powershell
copy bin\windows\project-switch.exe C:\path\to\your\PATH\
```

**Linux/macOS:**
```bash
sudo cp bin/linux/project-switch /usr/local/bin/
```

## Usage

```bash
# Switch between projects
project-switch switch

# Add a new project
project-switch add [name]

# Show current project
project-switch current

# Open a URL for current project
project-switch open <key>
```

## Configuration

Uses the same `.project-switch.yml` format as the Node.js version. See `example-config.yml` for reference.