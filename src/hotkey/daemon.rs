//! Daemon mode: global hotkey + system tray + GUI launcher in one process.

#[cfg(any(windows, target_os = "macos"))]
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
#[cfg(any(windows, target_os = "macos"))]
use muda::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
#[cfg(any(windows, target_os = "macos"))]
use std::sync::mpsc::{channel, Receiver};
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
    use crate::icon::create_icon_rgba;
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

#[cfg(any(windows, target_os = "macos"))]
enum DaemonEvent {
    Hotkey,
    Menu(MenuId),
}

#[cfg(windows)]
fn window_hwnd(cc: &eframe::CreationContext) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

#[cfg(windows)]
fn show_if_hidden(hwnd: Option<isize>) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        IsWindowVisible, SetForegroundWindow, ShowWindow, SW_SHOW,
    };
    let Some(hwnd) = hwnd else { return };
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(target_os = "macos")]
fn show_if_hidden(_hwnd: Option<isize>) {}

#[cfg(any(windows, target_os = "macos"))]
fn install_event_handlers(
    ctx: &egui::Context,
    hwnd: Option<isize>,
    open_id: MenuId,
) -> Receiver<DaemonEvent> {
    let (tx, rx) = channel();

    let hotkey_tx = tx.clone();
    let hotkey_ctx = ctx.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state != HotKeyState::Pressed {
            return;
        }
        let _ = hotkey_tx.send(DaemonEvent::Hotkey);
        show_if_hidden(hwnd);
        hotkey_ctx.request_repaint();
    }));

    let menu_ctx = ctx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let is_open = event.id() == &open_id;
        let _ = tx.send(DaemonEvent::Menu(event.id));
        if is_open {
            show_if_hidden(hwnd);
        }
        menu_ctx.request_repaint();
    }));

    rx
}

fn load_items() -> (Vec<ListItem>, String) {
    let config_manager = match ConfigManager::new() {
        Ok(cm) => cm,
        Err(_) => return (Vec::new(), "global".to_string()),
    };

    let display_name = crate::commands::list::selection_display_name(&config_manager);

    let (_, items) = crate::commands::list::load_items(&config_manager);
    (items, display_name)
}

struct DaemonApp {
    state: WindowState,
    client_name: String,
    prev_input: String,
    #[cfg(any(windows, target_os = "macos"))]
    _hotkey_manager: GlobalHotKeyManager,
    #[cfg(any(windows, target_os = "macos"))]
    _tray: TrayIcon,
    #[cfg(any(windows, target_os = "macos"))]
    menu_ids: MenuIds,
    #[cfg(any(windows, target_os = "macos"))]
    events: Receiver<DaemonEvent>,
}

#[cfg(any(windows, target_os = "macos"))]
impl DaemonApp {
    fn reload_items(&mut self) {
        let (items, name) = load_items();
        self.state.set_items(items);
        self.state.set_recent_keys(crate::history::load());
        self.client_name = name;
    }

    fn handle_event(&mut self, event: DaemonEvent) {
        match event {
            DaemonEvent::Hotkey => {
                self.state.toggle();
                if self.state.visibility == Visibility::Visible {
                    self.reload_items();
                }
            }
            DaemonEvent::Menu(id) if id == *self.menu_ids.open.id() => {
                self.state.show();
                self.reload_items();
            }
            DaemonEvent::Menu(id) if id == *self.menu_ids.exit.id() => std::process::exit(0),
            DaemonEvent::Menu(id) if id == *self.menu_ids.shortcuts.id() => {
                // Toggle shortcuts in config
                if let Ok(cm) = ConfigManager::new() {
                    let current = cm.get_shortcuts_config().enabled;
                    // Toggle by rewriting config — simplified for now
                    let _ = current; // TODO: implement toggle_shortcuts
                }
            }
            DaemonEvent::Menu(_) => {}
        }
    }
}

impl eframe::App for DaemonApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(any(windows, target_os = "macos"))]
        while let Ok(event) = self.events.try_recv() {
            self.handle_event(event);
        }

        // Surface failures from actions dispatched off the UI thread.
        self.state.poll_actions();
        if self.state.has_pending_actions() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        if crate::ui::window::sync_visibility(ctx, &mut self.state) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.state.visibility == Visibility::Hidden {
            return;
        }
        crate::ui::window::render_launcher(
            ui,
            &mut self.state,
            &self.client_name,
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

    let recent_keys = crate::history::load();
    let state = WindowState::new(items, recent_keys);

    eframe::run_native(
        "project-switch",
        crate::ui::launcher_options(false, None),
        Box::new(move |cc| {
            crate::ui::apply_launcher_style(&cc.egui_ctx);

            #[cfg(windows)]
            let hwnd = window_hwnd(cc);
            #[cfg(target_os = "macos")]
            let hwnd = None;
            #[cfg(any(windows, target_os = "macos"))]
            let events = install_event_handlers(&cc.egui_ctx, hwnd, menu_ids.open.id().clone());

            Ok(Box::new(DaemonApp {
                state,
                client_name: display_name,
                prev_input: String::new(),
                #[cfg(any(windows, target_os = "macos"))]
                _hotkey_manager: hotkey_manager,
                #[cfg(any(windows, target_os = "macos"))]
                _tray: tray,
                #[cfg(any(windows, target_os = "macos"))]
                menu_ids,
                #[cfg(any(windows, target_os = "macos"))]
                events,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Daemon error: {}", e))
}
