use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Hide this app from the macOS Dock (set activation policy to Accessory).
pub fn hide_from_dock() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

pub fn binary_name() -> &'static str {
    "project-switch"
}

/// No-op on macOS — trampoline is only needed on Windows multi-profile setups.
pub fn trampoline_if_needed() -> bool {
    false
}

pub fn kill_existing_hotkey_instances() {
    let our_pid = std::process::id().to_string();
    let output = Command::new("pgrep")
        .args(["-f", "project-switch-hotkey"])
        .output();

    if let Ok(output) = output {
        let pids = String::from_utf8_lossy(&output.stdout);
        for pid in pids.lines() {
            let pid = pid.trim();
            if !pid.is_empty() && pid != our_pid {
                let _ = Command::new("kill").arg(pid).status();
            }
        }
    }
}

/// No-op stub on macOS — the tray-managed WSL webserver is Windows-only.
pub fn spawn_webserver(_command: &str, _distro: Option<&str>) -> std::io::Result<Child> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "webserver is only supported on Windows",
    ))
}

/// No-op stub on macOS — the tray-managed WSL webserver is Windows-only.
pub fn stop_webserver(_child: Option<Child>, _command: &str, _distro: Option<&str>) {}

/// No-op stub on macOS — the tray-managed WSL webserver is Windows-only.
pub fn open_webserver_url() {}

/// No-op stub on macOS — the tray-managed WSL webserver is Windows-only.
pub fn launch_log_tail() {}

pub fn launch_project_switch(project_switch: &Path, monitor: u32) {
    // Kill any existing project-switch instances
    let _ = Command::new("pkill")
        .args(["-f", "project-switch list"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let monitor_arg = monitor.to_string();

    // Launch the windowed GUI launcher directly — no terminal needed
    let _ = Command::new(project_switch)
        .args(["list", "--gui", "--monitor", &monitor_arg])
        .spawn();
}
