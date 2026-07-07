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

## Webserver

The `project-switch-hotkey` tray app can manage a background webserver (the assist UI). Configure it under a `webserver` key in `~/.project-switch.yml`:

```yaml
webserver:
  enabled: true
  command: assist --no-open   # default
  distro: Ubuntu              # optional; Windows only, uses the default WSL distro if omitted
  port: 3100                  # optional; drives the "Open in browser" URL and the stop check
```

The `command` binds the port, so to move it you must set both: point the command at the port and match `port`. The bare `assist --no-open` is fixed to 3100 — use the subcommand form to choose a port, e.g. `command: assist sessions web --no-open --port 3101` with `port: 3101`. This lets two Windows accounts each run their own webserver without colliding on the shared host port (relevant under WSL mirrored networking, where a port bound in one account's WSL is visible machine-wide).

On Windows the webserver runs inside WSL via a login shell (`bash -lc`); on macOS it runs through your login shell (`$SHELL -ilc`). Because a login shell sources your `.profile`, any blocking setup there (for example an ssh-agent / SSL-key unlock step) can hang or fail at boot and take the webserver down.

When launching the webserver, the tray sets `PROJECT_SWITCH_WEBSERVER=1` in the shell's environment before the profile is sourced. Guard only the passphrase prompt, not the whole ssh-agent setup — the webserver must still attach to your persistent agent so git can reach the key once it is unlocked. With [keychain](https://www.funtoo.org/Keychain), use `--noask` for the webserver:

```sh
if [ -z "$PROJECT_SWITCH_WEBSERVER" ]; then
    eval `keychain --eval <your-key>`
else
    eval `keychain --eval --noask <your-key>`
fi
```

Keychain keeps one persistent agent per host, so unlocking the key once in any interactive shell also unlocks it for the running webserver.
