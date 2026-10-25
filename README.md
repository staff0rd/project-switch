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

# Manage the Paprika grocery list (see "Paprika grocery list" below)
project-switch grocery list
```

### Tab completion

In the launcher (terminal `list`, `list --gui` and the hotkey launcher), Tab completes the highlighted item's key. In the GUI it fills the full key plus a space, ready for arguments: type `fire`, press Tab, type `integrator`, press Enter to run `firefox-create-profile integrator`. Keys that contain spaces (e.g. a `Visual Studio Code` shortcut) are filled without the trailing space.

### Built-in switch item

The launcher list (terminal `list`, `list --gui` and the hotkey launcher) always contains a `switch` item — no config needed. Its detail column shows the active selection (e.g. `apm / web`), matching `project-switch current`. It ranks by recency like any other item. In the GUI, Enter opens the in-window client/project picker; in the terminal it runs the interactive `switch` prompt.

Delete any legacy config command that re-invokes the binary (e.g. `key: switch` with `command: project-switch.exe switch`). It is no longer intercepted: it spawns a nested process that does nothing useful from the GUI, and it duplicates the built-in row.

## Configuration

Uses `~/.project-switch.yml` for configuration. See `example-config.yml` for reference.

### Shortcuts

With `shortcuts.enabled`, `project-switch list` also indexes installed apps:

- **Windows** (`.lnk`/`.url`): user and public Desktop (flat), user and all-users Start Menu (recursive), plus packaged apps via `shell:AppsFolder`.
- **macOS** (`.app`): `/Applications` and `~/Applications` (flat), `/System/Applications` (recursive, so `Utilities/` apps like Activity Monitor and Terminal are found; never descends into `.app` internals).
- **Linux:** none.

`shortcuts.extraPaths` are scanned recursively. `shortcuts.exclude` patterns match the display name case-insensitively, as exact, `prefix*`, `*suffix` or `*contains*`:

```yaml
shortcuts:
  exclude:
    - "Migration Assistant"
    - "*Utility"
```

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
- **`paprika`**: merged field-by-field (local `image` / `token` win)
- Missing include file: warning printed, continues with local config only
- Only one level of include is supported (nested includes are ignored)
- The tool never writes to the included file

**Nested projects:**
Each client may contain a `projects:` array. When a project is selected, the effective command set is `project > client > global` (project commands override client commands; both override global). `project-switch switch` presents clients first; if the selected client has nested projects, a second prompt lets you pick the client itself (`<name> (client)`) or one of its projects.

**Schema migration:** Old configs using `projects:` / `currentProject:` are rewritten in place to `clients:` / `currentClient:` on first load. Included configs are migrated too.

See `example-include-config.yml` for a full shared config example.

## Browser choice

A command's `browser` can be a list of `{label, value}` entries instead of a single string, to open the same URL under different identities (e.g. one Firefox profile each):

```yaml
global:
  - key: member-dev
    url: https://member.dev.apm-bl.com/
    browser:
      - label: integrator
        value: firefox -P integrator
      - label: integrator+coach
        value: firefox -P coach
```

- With two or more entries, selecting the command in terminal `list` prompts with the labels; the chosen entry's `value` is launched exactly as a string `browser` would be, with the command's `args` and any typed URL args applied.
- A one-entry list opens immediately, like a string.
- The GUI launcher (hotkey / `list --gui`) shows the labels in the window; Enter opens with the chosen entry and Esc returns to the item list. The hidden `open` command uses the first entry.
- The last label picked for each command is remembered in `~/.project-switch-browser-choices.yml` and listed first next time; a remembered label no longer in the list is ignored.
- Only command-level `browser` takes a list. A list on a client, project or `defaultBrowser`, an empty list, or duplicate labels in one list fail config load.
- In an `include` merge, a local `browser` (string or list) replaces the base one entirely.

## Webview

A command with `webview: true` opens its `url` in a single reusable webview window (Windows and macOS) instead of a browser. Only one webview window exists: if it is already open, selecting any webview command just brings it to the front and leaves its page alone.

Add `navigate: true` to have the command also send its URL to an already-open window. A URL with the same origin as the current page is applied client-side (`history.pushState` plus a `popstate` event) so a single-page app reacts without reloading; any other URL is loaded normally. `navigate` requires `webview: true`.

```yaml
global:
  - key: assist
    url: http://localhost:3100
    webview: true            # only brings an open window to the front
  - key: assist - new
    url: http://localhost:3100/?new
    webview: true
    navigate: true           # also routes an open window to /?new
```

## Keyboard shortcuts

Add `hotkey` to a command to run it with one keystroke in the GUI launcher (hotkey / `list --gui`):

```yaml
global:
  - key: assist
    url: http://localhost:3100
    webview: true
    hotkey: alt+a
  - key: assist - new
    url: http://localhost:3100/?new
    webview: true
    navigate: true
    hotkey: alt+n
  - key: next
    command: ...
    hotkey: alt+x
```

- Format: one or more of `alt`/`ctrl`/`shift` plus a single letter or digit, joined by `+`, case-insensitive (`alt+a`, `ctrl+shift+1`). A key with no modifier is invalid.
- Pressing the combo in the main item list runs the command exactly as if it were selected and Enter pressed, whatever is typed; the letter is not inserted into the input. A multi-entry `browser` list still opens the browser picker.
- Hotkeys are ignored while the switch picker or browser picker is open, and in calculator (`=`) or file path input.
- Only commands in the active effective set (project > client > global) are bound. When levels share a combo, the more specific level wins.
- An invalid `hotkey`, or two commands in the same command list sharing a combo, fails config load. In an `include` merge, `hotkey` merges by command key like other command fields.
- Not supported in terminal `list`.

## Paprika grocery list

`project-switch grocery` manages a [Paprika](https://www.paprikaapp.com/) grocery list by running the [`ghcr.io/staff0rd/paprika`](https://github.com/staff0rd/paprika) Docker image, so Docker must be installed and running. It's opt-in via a `paprika:` section in `~/.project-switch.yml`:

```yaml
paprika:
  image: ghcr.io/staff0rd/paprika   # optional; this is the default
  token: ...                        # written by `project-switch grocery auth`
```

A bare `paprika:` is enough to enable it before you've logged in.

```bash
# Log in (prompts for your Paprika email and password inside the container)
project-switch grocery auth

# Show unpurchased items on every grocery list
project-switch grocery list

# Add to / remove from the default list, or a named one (case-insensitive)
project-switch grocery add "Milk"
project-switch grocery remove "Nails" --list "Hardware"
```

- `auth` runs the container with `-it` and saves the token as `paprika.token` in `~/.project-switch.yml`, never in an included file. Run it from a terminal.
- Every other command writes the token into a fresh temp dir, bind-mounts it into the container at `/home/paprika/.paprika` and deletes the dir afterwards. No Docker volume is created.
- A missing token, or one Paprika rejects, fails with a prompt to run `project-switch grocery auth`.
- Other container errors (Docker not running, an unknown list or item) are printed as-is.

With `paprika:` configured, the launcher (`project-switch list` and the GUI) gets a built-in `grocery` item. `grocery milk` adds Milk to the default list; in the GUI it runs in the background and failures show in the error banner. In `project-switch list`, Enter on `grocery` alone shows the default list's items and removes the one you pick.

In the GUI, Enter on `grocery` alone opens an in-window picker of the default list (it shows a loading state while the container runs):

- Type to filter the items; Enter removes the highlighted one.
- If the filter matches nothing, the picker offers `+ Add "<filter>"`; Enter adds it.
- The list refreshes after each change. Esc returns to the item list.
- Errors, including a missing or rejected token, show in the error banner.

Only the default list is available in the launcher; use `project-switch grocery ... --list <name>` for others.

## Webserver

The `project-switch-hotkey` tray app can manage background webservers (the assist UI). Configure them as a `webservers` list in `~/.project-switch.yml`; each gets its own tray submenu (Enabled, Restart, Open in browser, View logs):

```yaml
webservers:
  - name: pc-wsl
    enabled: true
    target: wsl                 # runs inside WSL (Windows only)
    distro: Ubuntu              # optional; uses the default WSL distro if omitted
    command: assist --no-open   # default
    port: 3100                  # default; drives "Open in browser" and the running check / stop
  - name: pc-windows
    enabled: true
    target: native              # runs on the host itself
    command: assist sessions web --no-open --port 3101
    port: 3101
```

`target` defaults to `wsl` on Windows and `native` elsewhere. A `wsl` server runs via a WSL login shell (`bash -lc`); a `native` server runs through `pwsh` on Windows and your login shell (`$SHELL -ilc`) on macOS.

Each server is checked and stopped by the port it listens on, so every server needs its own `port`. The `command` binds the port, so to move it you must set both: point the command at the port and match `port`. The bare `assist --no-open` is fixed to 3100 — use the subcommand form to choose a port. Separate ports also let two Windows accounts each run their own webserver without colliding on the shared host port (relevant under WSL mirrored networking, where a port bound in one account's WSL is visible machine-wide).

Each server logs to `assist-<name>.log` in `%LOCALAPPDATA%\project-switch` (Windows) or `~/Library/Logs/project-switch` (macOS).

The older single `webserver` key (same fields minus `name`, logging to `assist.log`) is still read when `webservers` is absent.

Because a login shell sources your `.profile`, any blocking setup there (for example an ssh-agent / SSL-key unlock step) can hang or fail at boot and take the webserver down.

When launching the webserver, the tray sets `PROJECT_SWITCH_WEBSERVER=1` in the shell's environment before the profile is sourced. Guard only the passphrase prompt, not the whole ssh-agent setup — the webserver must still attach to your persistent agent so git can reach the key once it is unlocked. With [keychain](https://www.funtoo.org/Keychain), use `--noask` for the webserver:

```sh
if [ -z "$PROJECT_SWITCH_WEBSERVER" ]; then
    eval `keychain --eval <your-key>`
else
    eval `keychain --eval --noask <your-key>`
fi
```

Keychain keeps one persistent agent per host, so unlocking the key once in any interactive shell also unlocks it for the running webserver.
