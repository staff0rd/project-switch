use std::path::Path;
use std::process::{Command, Stdio};

pub fn binary_name() -> &'static str {
    "project-switch"
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

    let cmd = format!("{} list", project_switch.display());

    if is_app_installed("iTerm") {
        let script = format!(
            r#"tell application "iTerm2"
    activate
    create window with default profile command "{cmd}"
end tell"#
        );
        let _ = Command::new("osascript").args(["-e", &script]).spawn();
    } else {
        let script = format!(
            r#"tell application "Terminal"
    activate
    do script "{cmd}"
end tell"#
        );
        let _ = Command::new("osascript").args(["-e", &script]).spawn();
    }
}

fn is_app_installed(name: &str) -> bool {
    Path::new(&format!("/Applications/{name}.app")).exists()
}
