use std::path::Path;
use std::process::{Command, Stdio};

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

pub fn launch_project_switch(project_switch: &Path) {
    // Kill any existing project-switch instances
    let _ = Command::new("pkill")
        .args(["-f", "project-switch list"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Launch the windowed GUI launcher directly — no terminal needed
    let _ = Command::new(project_switch)
        .args(["list", "--gui"])
        .spawn();
}
