# Plan: Migrate Launcher from CLI to Native Windowed UI

## Phase 1: Research & Foundation [checkpoint: a333332]

- [x] Task: Evaluate GUI framework (iced vs egui/eframe) — build a minimal prototype with text input + scrollable list for both, compare startup time, keystroke latency, binary size, and cross-platform support. Document decision in tech-stack.md. [733ac4c]
- [x] Task: Define the crate structure for the merged binary — decide whether to merge hotkey + launcher into a single crate or use a workspace with shared library crate. Update Cargo.toml(s) accordingly. [0075dea]
- [x] Task: Conductor - User Manual Verification 'Research & Foundation' (Protocol in workflow.md) [a333332]

## Phase 2: Core Launcher Window

- [x] Task: Write tests for launcher data model — extract `ListItem`, `ListItemKind`, filtering logic, and `encode_url_args` into a shared module (`src/launcher/mod.rs`). Write unit tests for filtering, matching, calculator evaluation, and file path detection. [2ff2489]
- [x] Task: Implement the shared launcher data model — move the business logic out of `list.rs` into the shared module. Existing CLI `list` command should import from the shared module. All existing tests must still pass. [2ff2489]
- [x] Task: Write tests for the launcher window state machine [d064449] — test states: hidden, visible/empty, visible/filtering, visible/selected. Test transitions: show, hide, type, navigate, execute.
- [x] Task: Implement the launcher window [ab9ca70] — create the GUI window with text input and filtered list using the chosen framework. Wire up the shared data model for filtering and display. Keyboard navigation (up/down/enter/escape) must work.
- [x] Task: Conductor - User Manual Verification 'Core Launcher Window' (Protocol in workflow.md)

## Phase 3: Feature Parity

- [ ] Task: Write tests for calculator mode in the windowed launcher — input starting with `=` evaluates and displays the result inline.
- [ ] Task: Implement calculator mode in the windowed launcher — integrate `meval` evaluation into the window's input handling.
- [ ] Task: Write tests for file path mode in the windowed launcher — file path detection, directory listing, auto-expansion.
- [ ] Task: Implement file path mode in the windowed launcher — filesystem browsing with tab-completion and auto-expand for single directory matches.
- [ ] Task: Implement action execution from the windowed launcher — on Enter, dispatch to `browser::open_url_in_browser`, `browser::open_command_with_args`, `browser::launch_shortcut`, or file path opening, reusing the existing `src/utils/browser.rs` module.
- [ ] Task: Conductor - User Manual Verification 'Feature Parity' (Protocol in workflow.md)

## Phase 4: Hotkey & System Tray Integration

- [ ] Task: Write tests for hotkey toggle behavior — hotkey shows window when hidden, hides when visible. Verify focus is set and input is cleared on show.
- [ ] Task: Merge the hotkey service into the launcher binary — integrate `global-hotkey` registration and the `tray-icon`/`muda` system tray into the same event loop as the GUI window. Hotkey press shows/hides the window instead of spawning a terminal.
- [ ] Task: Implement system tray context menu — retain Open, Shortcuts toggle, and Exit menu items. Wire Open to show the launcher window.
- [ ] Task: Platform-specific adjustments — ensure Alt+Space (Windows) and Cmd+Space (macOS) work correctly. Handle platform-specific window focus behavior (e.g., macOS activation policy).
- [ ] Task: Conductor - User Manual Verification 'Hotkey & System Tray Integration' (Protocol in workflow.md)

## Phase 5: Migration Cleanup

- [ ] Task: Update the Docker build pipeline — produce a single merged binary per platform instead of two separate binaries. Update `docker-compose.yml`, `build.ps1`, `build.sh`, and `Dockerfile`.
- [ ] Task: Retain CLI fallback for `project-switch list` — if launched from a terminal without GUI (e.g., SSH session), fall back to the existing `inquire`-based terminal mode. Add a `--tui` flag to force terminal mode.
- [ ] Task: Update documentation — update README.md, CLAUDE.md, and example configs to reflect the new architecture (single binary, windowed launcher, updated build output).
- [ ] Task: Remove the standalone hotkey crate — delete `hotkey/` directory and update any references. The functionality now lives in the main binary.
- [ ] Task: Conductor - User Manual Verification 'Migration Cleanup' (Protocol in workflow.md)
