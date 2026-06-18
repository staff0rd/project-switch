#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod icon;
mod platform;
mod sync;
mod webserver;

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, Submenu};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::TrayIconBuilder;

enum UserEvent {
    Hotkey(global_hotkey::GlobalHotKeyEvent),
    Menu(muda::MenuEvent),
}

fn main() {
    if platform::trampoline_if_needed() {
        return;
    }

    platform::kill_existing_hotkey_instances();

    let exe_dir = std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf();
    let project_switch = exe_dir.join(platform::binary_name());

    config::create_if_missing();
    sync::start_sync_thread();

    // Build event loop
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Register hotkey: CMD+Space on macOS, ALT+Space on Windows
    let manager = GlobalHotKeyManager::new().expect("Failed to create hotkey manager");
    let modifier = if cfg!(target_os = "macos") {
        Modifiers::SUPER
    } else {
        Modifiers::ALT
    };
    let hotkey = HotKey::new(Some(modifier), Code::Space);
    if let Err(e) = manager.register(hotkey) {
        eprintln!("Failed to register hotkey: {e}");
        std::process::exit(1);
    }

    // Forward hotkey events to the event loop
    let proxy = event_loop.create_proxy();
    GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Hotkey(event));
    }));

    // Forward menu events to the event loop
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));

    // Build monitor submenu from available monitors
    let monitor_count = event_loop.available_monitors().count().max(1);
    let saved_monitor = config::read_monitor_index();
    let monitor_submenu = Submenu::new("Monitor", true);
    let mut monitor_items: Vec<CheckMenuItem> = Vec::new();
    for i in 1..=monitor_count {
        let label = format!("Monitor {i}");
        let checked = i as u32 == saved_monitor;
        let item = CheckMenuItem::new(&label, true, checked, None);
        monitor_submenu
            .append(&item)
            .expect("Failed to add monitor item");
        monitor_items.push(item);
    }

    // Build context menu
    let menu = Menu::new();
    let open_item = MenuItem::new("Open", true, None);
    let open_id = open_item.id().clone();
    let shortcuts_enabled = config::read_shortcuts_enabled();
    let shortcuts_item = CheckMenuItem::new("Shortcuts", true, shortcuts_enabled, None);
    let shortcuts_id = shortcuts_item.id().clone();
    let webserver_enabled = config::read_webserver_enabled();
    let webserver_submenu = Submenu::new("Webserver", true);
    let webserver_item = CheckMenuItem::new("Enabled", true, webserver_enabled, None);
    let webserver_id = webserver_item.id().clone();
    let webserver_restart_item = MenuItem::new("Restart", true, None);
    let webserver_restart_id = webserver_restart_item.id().clone();
    let webserver_open_item = MenuItem::new("Open in browser", true, None);
    let webserver_open_id = webserver_open_item.id().clone();
    let webserver_logs_item = MenuItem::new("View logs", true, None);
    let webserver_logs_id = webserver_logs_item.id().clone();
    webserver_submenu
        .append(&webserver_item)
        .expect("Failed to add webserver item");
    webserver_submenu
        .append(&webserver_restart_item)
        .expect("Failed to add webserver item");
    webserver_submenu
        .append(&webserver_open_item)
        .expect("Failed to add webserver item");
    webserver_submenu
        .append(&webserver_logs_item)
        .expect("Failed to add webserver item");
    let exit_item = MenuItem::new("Exit", true, None);
    let exit_id = exit_item.id().clone();
    menu.append(&open_item).expect("Failed to add menu item");
    menu.append(&shortcuts_item).expect("Failed to add menu item");
    menu.append(&webserver_submenu)
        .expect("Failed to add menu item");
    menu.append(&monitor_submenu)
        .expect("Failed to add menu item");
    menu.append(&exit_item).expect("Failed to add menu item");

    let webserver_command = config::read_webserver_command();
    let webserver_distro = config::read_webserver_distro();

    // Spawn the WSL assist webserver at startup if enabled. A previous tray
    // instance killed without going through Exit (e.g. on rebuild/relaunch)
    // leaves its WSL webserver orphaned and holding port 3100, so clear any
    // existing one first to avoid an EADDRINUSE collision.
    let mut webserver_child = if webserver_enabled {
        webserver::stop_webserver(None, &webserver_command, webserver_distro.as_deref());
        match webserver::spawn_webserver(&webserver_command, webserver_distro.as_deref()) {
            Ok(child) => Some(child),
            Err(e) => {
                eprintln!("Failed to start webserver: {e}");
                None
            }
        }
    } else {
        None
    };

    let mut tray_icon = None;
    let mut current_monitor = saved_monitor;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                // Hide from Dock now that tao has initialized NSApplication
                #[cfg(target_os = "macos")]
                platform::hide_from_dock();

                // Create tray icon once the event loop is running (required on macOS)
                let tooltip = if cfg!(target_os = "macos") {
                    "Project Switch (\u{2318}+Space)"
                } else {
                    "Project Switch (ALT+SPACE)"
                };
                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_tooltip(tooltip)
                        .with_icon(icon::create_tray_icon())
                        .with_menu(Box::new(menu.clone()))
                        .build()
                        .expect("Failed to create tray icon"),
                );

                // Wake up the CFRunLoop so the icon appears immediately on macOS
                #[cfg(target_os = "macos")]
                {
                    use objc2_core_foundation::CFRunLoop;
                    let rl = CFRunLoop::main().unwrap();
                    rl.wake_up();
                }
            }

            Event::UserEvent(UserEvent::Hotkey(event)) => {
                if event.id() == hotkey.id() && event.state == HotKeyState::Pressed {
                    platform::launch_project_switch(&project_switch, current_monitor);
                }
            }

            Event::UserEvent(UserEvent::Menu(event)) => {
                if event.id() == &open_id {
                    platform::launch_project_switch(&project_switch, current_monitor);
                } else if event.id() == &shortcuts_id {
                    let new_value = config::toggle_shortcuts_enabled();
                    shortcuts_item.set_checked(new_value);
                } else if event.id() == &webserver_id {
                    let new_value = config::toggle_webserver_enabled();
                    webserver_item.set_checked(new_value);
                    if new_value {
                        match webserver::spawn_webserver(
                            &webserver_command,
                            webserver_distro.as_deref(),
                        ) {
                            Ok(child) => webserver_child = Some(child),
                            Err(e) => eprintln!("Failed to start webserver: {e}"),
                        }
                    } else {
                        webserver::stop_webserver(
                            webserver_child.take(),
                            &webserver_command,
                            webserver_distro.as_deref(),
                        );
                    }
                } else if event.id() == &webserver_restart_id {
                    webserver::stop_webserver(
                        webserver_child.take(),
                        &webserver_command,
                        webserver_distro.as_deref(),
                    );
                    match webserver::spawn_webserver(
                        &webserver_command,
                        webserver_distro.as_deref(),
                    ) {
                        Ok(child) => {
                            webserver_child = Some(child);
                            webserver_item.set_checked(true);
                        }
                        Err(e) => eprintln!("Failed to restart webserver: {e}"),
                    }
                } else if event.id() == &webserver_open_id {
                    webserver::open_webserver_url();
                } else if event.id() == &webserver_logs_id {
                    webserver::launch_log_tail();
                } else if event.id() == &exit_id {
                    webserver::stop_webserver(
                        webserver_child.take(),
                        &webserver_command,
                        webserver_distro.as_deref(),
                    );
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                } else {
                    for (i, item) in monitor_items.iter().enumerate() {
                        if event.id() == item.id() {
                            let index = (i + 1) as u32;
                            config::write_monitor_index(index);
                            current_monitor = index;
                            for (j, other) in monitor_items.iter().enumerate() {
                                other.set_checked(j == i);
                            }
                            platform::launch_project_switch(
                                &project_switch,
                                index,
                            );
                            break;
                        }
                    }
                }
            }

            _ => {}
        }
    });
}
