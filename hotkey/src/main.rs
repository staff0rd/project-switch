#![windows_subsystem = "windows"]

mod icon;

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::thread;
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x08000000;

use muda::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_NOREPEAT, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{PeekMessageW, PM_REMOVE, WM_HOTKEY};

fn main() {
    let exe_dir = std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf();
    let project_switch = exe_dir.join("project-switch.exe");

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

    // Build context menu with "Exit" item
    let menu = Menu::new();
    let exit_item = MenuItem::new("Exit", true, None);
    let exit_id = exit_item.id().clone();
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

        if has_message.as_bool() && msg.message == WM_HOTKEY {
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

        // Check tray menu events
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == &exit_id {
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
