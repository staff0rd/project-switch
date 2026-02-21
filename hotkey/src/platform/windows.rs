use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn binary_name() -> &'static str {
    "project-switch.exe"
}

pub fn kill_existing_hotkey_instances() {
    let our_pid = std::process::id().to_string();
    let _ = Command::new("wmic")
        .args([
            "process",
            "where",
            &format!("Name='project-switch-hotkey.exe' and ProcessId!='{our_pid}'"),
            "call",
            "terminate",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

pub fn launch_project_switch(project_switch: &Path) {
    // Kill any existing project-switch.exe instances first
    let _ = Command::new("taskkill")
        .args(["/IM", "project-switch.exe", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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
