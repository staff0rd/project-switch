use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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

    eprintln!("Hotkey registered. Listening for ALT+SPACE...");

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();

    while running.load(Ordering::SeqCst) {
        let has_message = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) };

        if has_message.as_bool() && msg.message == WM_HOTKEY {
            eprintln!("ALT+SPACE pressed — launching project-switch list...");

            // Kill any existing project-switch.exe instances first
            let _ = Command::new("taskkill")
                .args(["/IM", "project-switch.exe", "/F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
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

        thread::sleep(Duration::from_millis(50));
    }

    let _ = unsafe { UnregisterHotKey(None, hotkey_id) };
    eprintln!("Hotkey unregistered. Goodbye.");
}
