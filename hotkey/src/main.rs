#![windows_subsystem = "windows"]

mod config;
mod icon;
mod sync;

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub(crate) const CREATE_NO_WINDOW: u32 = 0x08000000;

use muda::{Menu, MenuEvent, MenuItem, CheckMenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_NOREPEAT, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, PM_REMOVE, WM_HOTKEY,
};

fn launch_project_switch(project_switch: &std::path::Path) {
    // Kill any existing project-switch.exe instances first
    let _ = Command::new("taskkill")
        .args(["/IM", "project-switch.exe", "/F"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();

    // Wrap in "cmd /c ... & exit /b 0" so the terminal tab closes
    // even when taskkill force-kills project-switch (exit code 1).
    let cmd_line = format!("{} list & exit /b 0", project_switch.to_string_lossy());
    match Command::new("wt.exe")
        .args(["--", "cmd", "/c", &cmd_line])
        .spawn()
    {
        Ok(_) => {}
        Err(e) => eprintln!("Failed to launch wt.exe: {e}"),
    }
}

fn main() {
    let exe_dir = std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf();
    let project_switch = exe_dir.join("project-switch.exe");

    config::create_if_missing();
    sync::start_sync_thread();

    // Register ALT+SPACE hotkey
    let hotkey_id = 1;
    let result = unsafe {
        RegisterHotKey(
            None,
            hotkey_id,
            MOD_ALT | MOD_NOREPEAT,
            VK_SPACE.0.into(),
        )
    };

    if let Err(e) = result {
        eprintln!(
            "Failed to register ALT+SPACE hotkey: {e}\n\
             Another program may have ALT+SPACE registered (check PowerToys, AutoHotkey)."
        );
        std::process::exit(1);
    }

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

    // Create system tray icon
    let _tray_icon = TrayIconBuilder::new()
        .with_tooltip("Project Switch (ALT+SPACE)")
        .with_icon(icon::create_tray_icon())
        .with_menu(Box::new(menu))
        .build()
        .expect("Failed to create tray icon");

    // Main message loop
    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
    let mut running = true;

    while running {
        // Check Win32 messages (hotkey)
        let has_message = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) };

        if has_message.as_bool() {
            if msg.message == WM_HOTKEY {
                launch_project_switch(&project_switch);
            } else {
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        // Check tray menu events
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == &open_id {
                launch_project_switch(&project_switch);
            } else if event.id() == &shortcuts_id {
                let new_value = config::toggle_shortcuts_enabled();
                shortcuts_item.set_checked(new_value);
            } else if event.id() == &exit_id {
                running = false;
            }
        }

        // Check tray icon events (double-click could also exit, but keeping it simple)
        if let Ok(_event) = TrayIconEvent::receiver().try_recv() {
            // No action on click — right-click menu handles everything
        }

        thread::sleep(Duration::from_millis(50));
    }

    let _ = unsafe { UnregisterHotKey(None, hotkey_id) };
}
