# Project Switch (Rust)

Fast CLI tool to manage and switch between clients. Written in Rust for blazing fast performance (~1-5ms startup time).

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
# Switch between clients
project-switch switch

# Show current client
project-switch current

# List openable items for the current client (interactive)
project-switch list
```

## Configuration

Uses `~/.project-switch.yml` for configuration. See `example-config.yml` for reference.

### Config Sharing

To share client definitions across machines, use the `include` field to reference a shared config file (e.g. stored in a dotfiles repo):

**Shared file** (`~/dotfiles/project-switch.yml`):
```yaml
clients:
  - name: myapp
    description: My main application
    commands:
      - key: docs
        url: https://docs.myapp.com
      - key: github
        url: https://github.com/user/myapp
      - key: build
        command: cargo build --release  # Runs as terminal command, not in browser
global:
  - key: search
    url: https://google.com/search?q=
```

**Local file** (`~/.project-switch.yml`):
```yaml
include: ~/dotfiles/project-switch.yml

currentClient: myapp
defaultBrowser: chrome
shortcuts:
  enabled: true
clients:
  - name: myapp
    path: C:\Users\me\projects\myapp
    browser: chrome
```

**Merge rules:**
- **Scalars** (`currentClient`, `currentProject`, `defaultBrowser`): local wins if present, otherwise base
- **`clients`**: matched by `name`, then merged field-by-field (local fields win)
- **`projects`** (nested under a client): matched by `name`, merged field-by-field; a project cannot itself contain a `projects` field
- **`commands`** (project-level, client-level, and `global`): matched by `key`, then merged field-by-field
- **`shortcuts`**: local replaces entirely (machine-specific)
- Missing include file: warning printed, continues with local config only
- Only one level of include is supported (nested includes are ignored)
- The tool never writes to the included file

**Nested projects:**
Each client may contain a `projects:` array. When a project is selected, the effective command set is `project > client > global` (project commands override client commands; both override global). `project-switch switch` presents clients first; if the selected client has nested projects, a second prompt lets you pick the client itself (`<name> (client)`) or one of its projects.

**Schema migration:** Old configs using `projects:` / `currentProject:` are rewritten in place to `clients:` / `currentClient:` on first load. Included configs are migrated too.

See `example-include-config.yml` for a full shared config example.
