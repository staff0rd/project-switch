#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod icon;
mod platform;
mod sync;

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::TrayIconBuilder;

enum UserEvent {
    Hotkey(global_hotkey::GlobalHotKeyEvent),
    Menu(muda::MenuEvent),
}

fn main() {
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

    // Build context menu
    let menu = Menu::new();
    let open_item = MenuItem::new("Open", true, None);
    let open_id = open_item.id().clone();
    let shortcuts_enabled = config::read_shortcuts_enabled();
    let shortcuts_item = CheckMenuItem::new("Shortcuts", true, shortcuts_enabled, None);
    let shortcuts_id = shortcuts_item.id().clone();
    let exit_item = MenuItem::new("Exit", true, None);
    let exit_id = exit_item.id().clone();
    menu.append(&open_item).expect("Failed to add menu item");
    menu.append(&shortcuts_item).expect("Failed to add menu item");
    menu.append(&exit_item).expect("Failed to add menu item");

    let mut tray_icon = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
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
                if event.id() == hotkey.id() {
                    platform::launch_project_switch(&project_switch);
                }
            }

            Event::UserEvent(UserEvent::Menu(event)) => {
                if event.id() == &open_id {
                    platform::launch_project_switch(&project_switch);
                } else if event.id() == &shortcuts_id {
                    let new_value = config::toggle_shortcuts_enabled();
                    shortcuts_item.set_checked(new_value);
                } else if event.id() == &exit_id {
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => {}
        }
    });
}
