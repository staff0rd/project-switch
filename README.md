# Project Switch (Rust)

Fast CLI tool to manage and switch between projects. Written in Rust for blazing fast performance (~1-5ms startup time).

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

Uses `.project-switch.yml` for configuration. See `example-config.yml` for reference.