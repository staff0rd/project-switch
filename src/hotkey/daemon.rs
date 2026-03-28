//! Daemon mode: global hotkey + system tray + GUI launcher in one process.

#[cfg(any(windows, target_os = "macos"))]
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
#[cfg(any(windows, target_os = "macos"))]
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
#[cfg(any(windows, target_os = "macos"))]
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::config::ConfigManager;
use crate::hotkey::sync;
use crate::launcher::ListItem;
use crate::ui::state::Visibility;
use crate::ui::WindowState;
use anyhow::Result;
use eframe::egui;

#[cfg(any(windows, target_os = "macos"))]
fn register_hotkey() -> Result<GlobalHotKeyManager> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

    let manager = GlobalHotKeyManager::new().map_err(|e| anyhow::anyhow!("{}", e))?;

    let hotkey = if cfg!(target_os = "macos") {
        HotKey::new(Some(Modifiers::META), Code::Space)
    } else {
        HotKey::new(Some(Modifiers::ALT), Code::Space)
    };

    manager
        .register(hotkey)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(manager)
}

#[cfg(any(windows, target_os = "macos"))]
struct MenuIds {
    open: MenuItem,
    shortcuts: CheckMenuItem,
    exit: MenuItem,
}

#[cfg(any(windows, target_os = "macos"))]
fn create_tray(shortcuts_enabled: bool) -> Result<(TrayIcon, MenuIds)> {
    use crate::hotkey::icon::create_icon_rgba;
    let menu = Menu::new();
    let open = MenuItem::new("Open", true, None);
    let shortcuts = CheckMenuItem::new("Shortcuts", true, shortcuts_enabled, None);
    let separator = PredefinedMenuItem::separator();
    let exit = MenuItem::new("Exit", true, None);

    menu.append(&open).map_err(|e| anyhow::anyhow!("{}", e))?;
    menu.append(&shortcuts)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    menu.append(&separator)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    menu.append(&exit).map_err(|e| anyhow::anyhow!("{}", e))?;

    let (icon_rgba, w, h) = create_icon_rgba();
    let icon = tray_icon::Icon::from_rgba(icon_rgba, w, h).map_err(|e| anyhow::anyhow!("{}", e))?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("project-switch")
        .with_icon(icon)
        .build()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok((
        tray,
        MenuIds {
            open,
            shortcuts,
            exit,
        },
    ))
}

fn load_items() -> (Vec<ListItem>, String) {
    let config_manager = match ConfigManager::new() {
        Ok(cm) => cm,
        Err(_) => return (Vec::new(), "global".to_string()),
    };

    let display_name = config_manager
        .resolve_current_project()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "global".to_string());

    let (_, items) = crate::commands::list::load_items(&config_manager);
    (items, display_name)
}

struct DaemonApp {
    state: WindowState,
    project_name: String,
    prev_input: String,
    #[cfg(any(windows, target_os = "macos"))]
    _hotkey_manager: GlobalHotKeyManager,
    #[cfg(any(windows, target_os = "macos"))]
    _tray: TrayIcon,
    #[cfg(any(windows, target_os = "macos"))]
    menu_ids: MenuIds,
}

impl eframe::App for DaemonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll hotkey events
        #[cfg(any(windows, target_os = "macos"))]
        if let Ok(_event) = GlobalHotKeyEvent::receiver().try_recv() {
            self.state.toggle();
            if self.state.visibility == Visibility::Visible {
                let (items, name) = load_items();
                self.state.set_items(items);
                self.project_name = name;
            }
        }

        // Poll tray menu events
        #[cfg(any(windows, target_os = "macos"))]
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == self.menu_ids.open.id() {
                self.state.show();
                let (items, name) = load_items();
                self.state.set_items(items);
                self.project_name = name;
            } else if event.id() == self.menu_ids.exit.id() {
                std::process::exit(0);
            } else if event.id() == self.menu_ids.shortcuts.id() {
                // Toggle shortcuts in config
                if let Ok(cm) = ConfigManager::new() {
                    let current = cm.get_shortcuts_config().enabled;
                    // Toggle by rewriting config — simplified for now
                    let _ = current; // TODO: implement toggle_shortcuts
                }
            }
        }

        // Request repaint periodically to keep polling events
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        // Delegate to the launcher window rendering
        if self.state.visibility == Visibility::Hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        crate::ui::window::render_launcher(
            ctx,
            &mut self.state,
            &self.project_name,
            &mut self.prev_input,
        );
    }
}

/// Run the daemon: hotkey listener + system tray + GUI launcher.
pub fn run() -> Result<()> {
    // Start config sync
    if let Ok(cm) = ConfigManager::new() {
        sync::start(cm.get_include_path().map(|s| s.to_string()));
    }

    let (items, display_name) = load_items();
    #[cfg(any(windows, target_os = "macos"))]
    let shortcuts_enabled = ConfigManager::new()
        .map(|cm| cm.get_shortcuts_config().enabled)
        .unwrap_or(true);

    #[cfg(any(windows, target_os = "macos"))]
    let hotkey_manager = register_hotkey()?;
    #[cfg(any(windows, target_os = "macos"))]
    let (tray, menu_ids) = create_tray(shortcuts_enabled)?;

    let state = WindowState::new(items);

    eframe::run_native(
        "project-switch",
        crate::ui::launcher_options(false),
        Box::new(move |cc| {
            crate::ui::apply_launcher_style(&cc.egui_ctx);

            Ok(Box::new(DaemonApp {
                state,
                project_name: display_name,
                prev_input: String::new(),
                #[cfg(any(windows, target_os = "macos"))]
                _hotkey_manager: hotkey_manager,
                #[cfg(any(windows, target_os = "macos"))]
                _tray: tray,
                #[cfg(any(windows, target_os = "macos"))]
                menu_ids,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Daemon error: {}", e))
}
