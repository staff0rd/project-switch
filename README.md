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

Uses `~/.project-switch.yml` for configuration. See `example-config.yml` for reference.

### Config Sharing

To share project definitions across machines, use the `include` field to reference a shared config file (e.g. stored in a dotfiles repo):

**Shared file** (`~/dotfiles/project-switch.yml`):
```yaml
projects:
  - name: myapp
    description: My main application
    commands:
      - key: docs
        url: https://docs.myapp.com
      - key: github
        url: https://github.com/user/myapp
global:
  - key: search
    url: https://google.com/search?q=
    url_encode: true
```

**Local file** (`~/.project-switch.yml`):
```yaml
include: ~/dotfiles/project-switch.yml

currentProject: myapp
defaultBrowser: chrome
shortcuts:
  enabled: true
projects:
  - name: myapp
    path: C:\Users\me\projects\myapp
    browser: chrome
```

**Merge rules:**
- **Scalars** (`currentProject`, `defaultBrowser`): local wins if present, otherwise base
- **`projects`**: matched by `name`, then merged field-by-field (local fields win)
- **`commands`** (project-level and `global`): matched by `key`, then merged field-by-field
- **`shortcuts`**: local replaces entirely (machine-specific)
- Missing include file: warning printed, continues with local config only
- Only one level of include is supported (nested includes are ignored)
- The tool never writes to the included file

See `example-include-config.yml` for a full shared config example.